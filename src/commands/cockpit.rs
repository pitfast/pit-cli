use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use serde::Deserialize;
use tokio::time::sleep;

const MAX_EXIT_TOKENS: usize = 8;

#[derive(Debug, Args)]
pub struct CockpitArgs {
    /// PitLane's loopback read-only control endpoint.
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub endpoint: String,
    /// Snapshot refresh interval in milliseconds.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u64).range(50..=2000))]
    pub refresh_ms: u64,
    /// Save the last received snapshot when the Cockpit exits.
    #[arg(long)]
    pub json_on_exit: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
struct Snapshot {
    grid: Grid,
    services: Vec<Service>,
    recent_executions: Vec<Execution>,
    system: SystemInfo,
}

#[derive(Debug, Clone, Deserialize)]
struct Grid {
    lane_count: usize,
    running: usize,
    queue_depth: usize,
    completed: u64,
    failed: u64,
    peak_active_lanes: usize,
    utilization: f64,
    lanes: Vec<Lane>,
}

#[derive(Debug, Clone, Deserialize)]
struct Lane {
    lane_id: usize,
    state: String,
    execution_id: Option<String>,
    service_id: Option<String>,
    release_id: Option<String>,
    variant: Option<String>,
    #[serde(default)]
    shadow: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Service {
    service_id: String,
    deployed: bool,
    readiness: String,
    active_executions: usize,
    total_requests: u64,
    failures: u64,
    last_activity_unix_ms: Option<u128>,
}

#[derive(Debug, Clone, Deserialize)]
struct Execution {
    request_id: String,
    execution_id: String,
    service_id: String,
    release_id: Option<String>,
    variant: Option<String>,
    #[serde(default)]
    shadow: bool,
    lane_id: usize,
    status: String,
    queue_wait_us: u128,
    dispatch_gap_us: u128,
    guest_execution_us: u128,
    total_us: u128,
}

#[derive(Debug, Clone, Deserialize)]
struct SystemInfo {
    process_cpu_percent: Option<f64>,
    host_logical_cpus: Option<usize>,
    cpu_equivalent_cores: Option<f64>,
    process_rss_bytes: Option<u64>,
    active_guest_executions: usize,
    active_stores: usize,
    hot_artifacts: usize,
    warm_artifacts: usize,
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode().context("failed to enable terminal raw mode")?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)
            .context("failed to enter Cockpit terminal")?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
        let _ = terminal::disable_raw_mode();
    }
}

pub async fn run(args: CockpitArgs) -> Result<()> {
    let client = reqwest::Client::new();
    let endpoint = format!(
        "{}/v1/cockpit/snapshot",
        args.endpoint.trim_end_matches('/')
    );
    let _terminal = TerminalGuard::enter()?;
    let mut paused = false;
    let mut last_snapshot: Option<Snapshot> = None;
    let mut last_json: Option<String> = None;
    let mut error = None;

    loop {
        if !paused {
            match client.get(&endpoint).send().await {
                Ok(response) => match response.error_for_status() {
                    Ok(response) => match response.text().await {
                        Ok(body) => match serde_json::from_str::<Snapshot>(&body) {
                            Ok(snapshot) => {
                                last_json = Some(body);
                                last_snapshot = Some(snapshot);
                                error = None;
                            }
                            Err(parse_error) => {
                                error = Some(format!("invalid snapshot: {parse_error}"))
                            }
                        },
                        Err(request_error) => error = Some(request_error.to_string()),
                    },
                    Err(request_error) => error = Some(request_error.to_string()),
                },
                Err(request_error) => error = Some(format!("PitLane unavailable: {request_error}")),
            }
        }
        render(last_snapshot.as_ref(), error.as_deref(), paused)?;

        let refresh = Duration::from_millis(args.refresh_ms);
        let mut elapsed = Duration::ZERO;
        while elapsed < refresh {
            if event::poll(Duration::from_millis(10)).unwrap_or(false)
                && let Ok(Event::Key(key)) = event::read()
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if let Some(path) = &args.json_on_exit
                            && let Some(json) = &last_json
                        {
                            std::fs::write(path, json).with_context(|| {
                                format!("failed to write Cockpit snapshot {}", path.display())
                            })?;
                        }
                        return Ok(());
                    }
                    KeyCode::Char('p') | KeyCode::Char(' ') => paused = !paused,
                    _ => {}
                }
            }
            sleep(Duration::from_millis(10)).await;
            elapsed += Duration::from_millis(10);
        }
    }
}

fn render(snapshot: Option<&Snapshot>, error: Option<&str>, paused: bool) -> Result<()> {
    let (width, height) = terminal::size().unwrap_or((120, 36)).max((120, 36));
    let mut stdout = io::stdout();
    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    execute!(
        stdout,
        SetForegroundColor(Color::Red),
        SetAttribute(Attribute::Bold),
        Print(" PITFAST COCKPIT "),
        ResetColor,
        SetAttribute(Attribute::Reset),
        Print(if paused { " [PAUSED]" } else { " [LIVE]" }),
        Print(format!(
            "   {}",
            "─".repeat(width.saturating_sub(22) as usize)
        )),
        MoveTo(0, 1),
        Print(" Read-only operational view  |  q quit  p pause  refresh 50–2000ms\n")
    )?;

    let Some(snapshot) = snapshot else {
        execute!(
            stdout,
            MoveTo(0, 3),
            SetForegroundColor(Color::Yellow),
            Print("Waiting for PitLane snapshot..."),
            ResetColor
        )?;
        if let Some(error) = error {
            execute!(
                stdout,
                MoveTo(0, 4),
                SetForegroundColor(Color::Red),
                Print(error),
                ResetColor
            )?;
        }
        stdout.flush()?;
        return Ok(());
    };

    let mut row = 3u16;
    let queue_count = snapshot.grid.queue_depth;
    print_box_header(
        &mut stdout,
        row,
        width,
        "PIT ENTRY QUEUE",
        &format!("[WAITING {queue_count:02}]"),
        Color::Yellow,
    )?;
    row += 1;
    let queue_text = if queue_count == 0 {
        "EMPTY - no work waiting".to_owned()
    } else {
        (0..queue_count.min(8))
            .map(|index| format!("[WAITING #{:02}]", index + 1))
            .collect::<Vec<_>>()
            .join("  ")
    };
    print_box_line(&mut stdout, row, width, &queue_text, Color::Yellow)?;
    row += 1;
    print_flow_arrow(&mut stdout, row, width, "PIT ENTRY", Color::Yellow)?;
    row += 1;

    print_box_header(
        &mut stdout,
        row,
        width,
        "ACTIVE EXECUTIONS",
        &format!(
            "[RUNNING {}/{}]",
            snapshot.grid.running, snapshot.grid.lane_count
        ),
        Color::Red,
    )?;
    row += 1;
    for lane in &snapshot.grid.lanes {
        let service = lane.service_id.as_deref().unwrap_or("service?");
        let execution = lane
            .execution_id
            .as_deref()
            .map(short_id)
            .unwrap_or_else(|| "-".to_owned());
        let release = lane
            .release_id
            .as_deref()
            .map(short_id)
            .unwrap_or_else(|| "-".to_owned());
        let variant = lane.variant.as_deref().unwrap_or("-");
        let text = if lane.state == "running" {
            format!(
                "LANE {:02}  [RUNNING]  {service}  exec={execution}  release={release}  {variant}{}",
                lane.lane_id + 1,
                if lane.shadow { "  SHADOW" } else { "" }
            )
        } else {
            format!("LANE {:02}  [FREE]     no execution", lane.lane_id + 1)
        };
        row += 1;
        print_box_line(
            &mut stdout,
            row,
            width,
            &text,
            if lane.state == "running" {
                Color::Red
            } else {
                Color::DarkGrey
            },
        )?;
    }
    row += 1;
    print_flow_arrow(&mut stdout, row, width, "LANES", Color::Red)?;
    row += 1;

    print_box_header(
        &mut stdout,
        row,
        width,
        "EXIT / COMPLETED",
        "[RECENT]",
        Color::Green,
    )?;
    row += 1;
    let exit_text = if snapshot.recent_executions.is_empty() {
        "EMPTY - no completed executions yet".to_owned()
    } else {
        snapshot
            .recent_executions
            .iter()
            .rev()
            .take(MAX_EXIT_TOKENS)
            .map(|execution| {
                let marker = if execution.status == "completed" {
                    "EXIT"
                } else {
                    "FAILED"
                };
                format!(
                    "[{marker}] {} · exec={}",
                    execution.service_id,
                    short_id(&execution.execution_id)
                )
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    print_box_line(&mut stdout, row, width, &exit_text, Color::Green)?;

    let panel_row = height.saturating_sub(12).max(row + 2);
    execute!(
        stdout,
        MoveTo(0, panel_row),
        SetForegroundColor(Color::Cyan),
        Print("SUMMARY"),
        ResetColor
    )?;
    execute!(
        stdout,
        MoveTo(0, panel_row + 1),
        Print(format!(
            "  services={}  queue={}  running={}/{}  completed={}  failed={}  peak-lanes={}  grid={:.0}%",
            snapshot
                .services
                .iter()
                .filter(|service| service.deployed)
                .count(),
            snapshot.grid.queue_depth,
            snapshot.grid.running,
            snapshot.grid.lane_count,
            snapshot.grid.completed,
            snapshot.grid.failed,
            snapshot.grid.peak_active_lanes,
            snapshot.grid.utilization * 100.0
        )),
        MoveTo(0, panel_row + 2),
        Print(format!(
            "  Process CPU={}  host-cpus={}  RSS={}  guest={}  stores={}  HOT={}  WARM={}",
            format_percent(
                snapshot.system.process_cpu_percent,
                snapshot.system.cpu_equivalent_cores,
            ),
            snapshot
                .system
                .host_logical_cpus
                .map_or_else(|| "n/a".to_owned(), |value| value.to_string()),
            format_bytes(snapshot.system.process_rss_bytes),
            snapshot.system.active_guest_executions,
            snapshot.system.active_stores,
            snapshot.system.hot_artifacts,
            snapshot.system.warm_artifacts
        )),
        MoveTo(0, panel_row + 4),
        SetForegroundColor(Color::Cyan),
        Print("SERVICES"),
        ResetColor
    )?;
    for (index, service) in snapshot.services.iter().enumerate() {
        let service_row = panel_row + 5 + index as u16;
        if service_row >= height {
            break;
        }
        execute!(
            stdout,
            MoveTo(0, service_row),
            SetForegroundColor(readiness_color(&service.readiness)),
            Print(format!(
                "  {:<14} {:<5} active={} requests={} failures={} last={}",
                service.service_id,
                service.readiness.to_uppercase(),
                service.active_executions,
                service.total_requests,
                service.failures,
                service
                    .last_activity_unix_ms
                    .map_or("-".to_owned(), |_| "seen".to_owned())
            )),
            ResetColor
        )?;
    }
    if let Some(latest) = snapshot.recent_executions.last() {
        let detail_row = height.saturating_sub(3);
        execute!(
            stdout,
            MoveTo(0, detail_row),
            SetForegroundColor(Color::DarkGrey),
            Print(format!(
                "DETAIL latest={} req={} service={} release={} variant={}{} lane={} queue-wait={}us dispatch-gap={}us guest={}us total={}us",
                latest.execution_id,
                latest.request_id,
                latest.service_id,
                latest.release_id.as_deref().unwrap_or("n/a"),
                latest.variant.as_deref().unwrap_or("stable"),
                if latest.shadow { " shadow" } else { "" },
                latest.lane_id + 1,
                latest.queue_wait_us,
                latest.dispatch_gap_us,
                latest.guest_execution_us,
                latest.total_us
            )),
            ResetColor
        )?;
    }
    if let Some(error) = error {
        execute!(
            stdout,
            MoveTo(0, height.saturating_sub(1)),
            SetForegroundColor(Color::Red),
            Print(error),
            ResetColor
        )?;
    }
    stdout.flush()?;
    Ok(())
}

fn print_box_header(
    stdout: &mut io::Stdout,
    row: u16,
    width: u16,
    title: &str,
    detail: &str,
    color: Color,
) -> Result<()> {
    let prefix = format!("+-- {title} {detail} ");
    let dashes = (width as usize)
        .saturating_sub(prefix.chars().count())
        .saturating_sub(1);
    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(color),
        SetAttribute(Attribute::Bold),
        Print(prefix),
        Print("-".repeat(dashes)),
        Print("+"),
        SetAttribute(Attribute::Reset),
        ResetColor
    )?;
    Ok(())
}

fn print_box_line(
    stdout: &mut io::Stdout,
    row: u16,
    width: u16,
    text: &str,
    color: Color,
) -> Result<()> {
    let inner_width = (width as usize).saturating_sub(4);
    let text = fit_text(text, inner_width);
    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(color),
        Print(format!("| {:<inner_width$} |", text)),
        ResetColor
    )?;
    Ok(())
}

fn print_flow_arrow(
    stdout: &mut io::Stdout,
    row: u16,
    width: u16,
    label: &str,
    color: Color,
) -> Result<()> {
    let text = format!("                         |  {label}  v");
    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(color),
        Print(fit_text(&text, width as usize)),
        ResetColor
    )?;
    Ok(())
}

fn fit_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_owned();
    }
    let mut result = text.chars().take(max_chars - 1).collect::<String>();
    result.push('…');
    result
}

fn short_id(value: &str) -> String {
    value
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn readiness_color(readiness: &str) -> Color {
    match readiness {
        "hot" => Color::Green,
        "warm" => Color::Yellow,
        _ => Color::DarkGrey,
    }
}

fn format_percent(value: Option<f64>, equivalent_cores: Option<f64>) -> String {
    match (value, equivalent_cores) {
        (Some(value), Some(cores)) => format!("{value:.1}% (~{cores:.1} logical CPUs)"),
        (Some(value), None) => format!("{value:.1}% (equivalent cores n/a)"),
        _ => "n/a".to_owned(),
    }
}

fn format_bytes(value: Option<u64>) -> String {
    value.map_or_else(
        || "n/a".to_owned(),
        |value| format!("{}MiB", value / 1024 / 1024),
    )
}

#[cfg(test)]
mod tests {
    use super::{fit_text, format_bytes, format_percent, short_id};

    #[test]
    fn execution_ids_are_compact_for_the_flow() {
        assert_eq!(short_id("execution-123456"), "123456");
    }

    #[test]
    fn missing_memory_is_explicit() {
        assert_eq!(format_bytes(None), "n/a");
    }

    #[test]
    fn cpu_is_not_capped_at_one_hundred_percent() {
        assert_eq!(
            format_percent(Some(801.8), Some(8.018)),
            "801.8% (~8.0 logical CPUs)"
        );
        assert_eq!(format_percent(None, Some(8.0)), "n/a");
    }

    #[test]
    fn flow_labels_are_bounded_for_small_terminals() {
        assert_eq!(fit_text("LANE 01 [RUNNING] orders", 10), "LANE 01 […");
        assert_eq!(fit_text("short", 10), "short");
        assert_eq!(fit_text("anything", 0), "");
    }
}
