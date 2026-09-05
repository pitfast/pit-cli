use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Args;
use pit_crew::ArtifactManifest;
use pit_node::{
    ExecutionLimits, ExecutionRequest, ExecutionResult, ExecutionStatus, PitNode, WasmArtifact,
};
use pit_scheduler::ExecutionEvent;

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Path to the WASM module. Omit it to use .pit/artifact.json.
    pub wasm_file: Option<PathBuf>,
    /// Number of independent invocations.
    #[arg(long, default_value_t = 1)]
    pub concurrency: usize,
    /// Explicit guest environment variable in KEY=VALUE form.
    #[arg(long, value_parser = parse_env)]
    pub env: Vec<(String, String)>,
    /// Stop guest execution after this duration, for example 500ms or 2s.
    #[arg(long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
    /// Limit each guest linear memory, for example 64MiB.
    #[arg(long, value_parser = parse_memory)]
    pub memory: Option<usize>,
    /// Print queued, started, completed, and failed execution events.
    #[arg(short, long)]
    pub verbose: bool,
    /// Arguments passed to the guest after the conventional separator.
    #[arg(last = true)]
    pub guest_args: Vec<String>,
}

pub async fn resolve_artifact(project_dir: &Path, wasm_file: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = wasm_file {
        return Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            project_dir.join(path)
        });
    }
    let manifest = ArtifactManifest::read(project_dir).await?;
    manifest.resolve_path(project_dir)
}

pub async fn run(args: RunArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let wasm_file = resolve_artifact(&project_dir, args.wasm_file.as_deref()).await?;
    run_artifact(
        wasm_file,
        args.concurrency,
        args.env,
        args.timeout,
        args.memory,
        args.guest_args,
        args.verbose,
    )
}

fn run_artifact(
    wasm_file: PathBuf,
    concurrency: usize,
    env: Vec<(String, String)>,
    timeout: Option<Duration>,
    memory: Option<usize>,
    guest_args: Vec<String>,
    verbose: bool,
) -> Result<()> {
    if concurrency == 0 {
        bail!("concurrency must be greater than zero");
    }

    let artifact = WasmArtifact::from_path(&wasm_file);
    let request = ExecutionRequest::new(artifact.clone())
        .with_args(guest_args)
        .with_env(env)
        .with_limits(ExecutionLimits {
            timeout,
            memory_bytes: memory,
        });
    let node = PitNode::from_artifact(artifact)?;
    let report = node.execute_many(request, concurrency)?;

    if verbose {
        print_events(&report.events);
        print_selected_output(&report.executions);
        println!();
    } else if report.requested == 1 {
        print_single_output(&report.executions);
    }

    print_run_summary(&report);
    if report.completed != report.requested {
        print_failures(&report.executions);
        bail!("one or more WASM executions did not complete successfully");
    }
    Ok(())
}

fn print_single_output(executions: &[ExecutionResult]) {
    if let Some(execution) = executions.first() {
        println!("Execution: {}", execution.execution_id);
        println!("Lane: {}", execution.lane_id);
        println!("Status: {}", execution.status);
        println!(
            "Exit Code: {}",
            execution
                .exit_code
                .map_or_else(|| "-".to_owned(), |code| code.to_string())
        );
        println!("Duration: {}", format_duration(execution.duration));
        print_output_block("stdout", &execution.stdout);
        print_output_block("stderr", &execution.stderr);
        println!();
    }
}

fn print_selected_output(executions: &[ExecutionResult]) {
    for execution in executions
        .iter()
        .filter(|execution| !execution.stdout.is_empty() || !execution.stderr.is_empty())
        .take(5)
    {
        println!("BOX {} OUTPUT", execution.execution_id);
        print_output_block("stdout", &execution.stdout);
        print_output_block("stderr", &execution.stderr);
    }
}

fn print_output_block(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    println!("{label}:");
    let output = String::from_utf8_lossy(bytes);
    print!("{output}");
    if !output.ends_with('\n') {
        println!();
    }
}

fn print_events(events: &[ExecutionEvent]) {
    for event in events {
        match event {
            ExecutionEvent::Queued { execution_id } => println!("BOX {execution_id}  QUEUED"),
            ExecutionEvent::Started {
                execution_id,
                lane_id,
                queued_for,
            } => println!(
                "BOX {execution_id}  LANE {lane_id}  START  queue {}",
                format_duration(*queued_for)
            ),
            ExecutionEvent::Completed {
                execution_id,
                lane_id,
                duration,
            } => println!(
                "BOX {execution_id}  LANE {lane_id}  DONE   {}",
                format_duration(*duration)
            ),
            ExecutionEvent::Failed {
                execution_id,
                lane_id,
                duration,
                error,
            } => println!(
                "BOX {execution_id}  LANE {lane_id}  FAIL   {}  {error}",
                format_duration(*duration)
            ),
        }
    }
}

fn print_failures(executions: &[ExecutionResult]) {
    for execution in executions
        .iter()
        .filter(|execution| execution.status != ExecutionStatus::Completed)
        .take(5)
    {
        eprintln!(
            "execution {} on lane {}: status={}, exit_code={}, error={}",
            execution.execution_id,
            execution.lane_id,
            execution.status,
            execution
                .exit_code
                .map_or_else(|| "-".to_owned(), |code| code.to_string()),
            execution.error.as_deref().unwrap_or("none")
        );
    }
}

pub fn parse_env(value: &str) -> Result<(String, String), String> {
    let (key, value) = value
        .split_once('=')
        .ok_or_else(|| "environment entry must use KEY=VALUE".to_owned())?;
    if key.is_empty() || key.contains('\0') || value.contains('\0') {
        return Err("environment entry has an invalid name or NUL byte".to_owned());
    }
    Ok((key.to_owned(), value.to_owned()))
}

pub fn parse_duration(value: &str) -> Result<Duration, String> {
    let (number, unit) = if let Some(number) = value.strip_suffix("ms") {
        (number, "ms")
    } else if let Some(number) = value.strip_suffix('s') {
        (number, "s")
    } else if let Some(number) = value.strip_suffix('m') {
        (number, "m")
    } else {
        return Err("duration must use ms, s, or m (for example 500ms)".to_owned());
    };
    let number = number
        .parse::<u64>()
        .map_err(|_| "duration value must be a non-negative integer".to_owned())?;
    match unit {
        "ms" => Ok(Duration::from_millis(number)),
        "s" => Ok(Duration::from_secs(number)),
        "m" => number
            .checked_mul(60)
            .map(Duration::from_secs)
            .ok_or_else(|| "duration is too large".to_owned()),
        _ => unreachable!(),
    }
}

pub fn parse_memory(value: &str) -> Result<usize, String> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("KiB") {
        (number, 1024_usize)
    } else if let Some(number) = value.strip_suffix("MiB") {
        (number, 1024_usize.pow(2))
    } else if let Some(number) = value.strip_suffix("GiB") {
        (number, 1024_usize.pow(3))
    } else {
        return Err("memory must use KiB, MiB, or GiB (for example 64MiB)".to_owned());
    };
    let number = number
        .parse::<usize>()
        .map_err(|_| "memory value must be a non-negative integer".to_owned())?;
    number
        .checked_mul(multiplier)
        .ok_or_else(|| "memory limit is too large".to_owned())
}

fn print_run_summary(report: &pit_node::NodeRunReport) {
    let timing = TimingSummary::from_reports(&report.executions);
    println!("PitFast Pit Box");
    println!();
    println!("Component: {}", report.artifact_path.display());
    println!();
    println!("Execution Grid");
    println!("  Lanes: {}", report.execution_lanes);
    println!();
    println!("Executions");
    println!("  Requested: {}", report.requested);
    println!("  Completed: {}", report.completed);
    println!("  Failed: {}", report.failed);
    println!("  Timed Out: {}", report.timed_out);
    println!("  Cancelled: {}", report.cancelled);
    println!("  Memory Limit: {}", report.memory_limit_exceeded);
    println!("  Peak Active: {}", report.peak_active);
    println!();
    println!("Timing");
    println!("  Total: {}", format_duration(report.total_duration));
    println!("  Mean: {}", format_duration(timing.mean));
    println!("  p50: {}", format_duration(timing.p50));
    println!("  p95: {}", format_duration(timing.p95));
    println!("  p99: {}", format_duration(timing.p99));
}

#[derive(Debug, Clone, Copy)]
struct TimingSummary {
    mean: Duration,
    p50: Duration,
    p95: Duration,
    p99: Duration,
}

impl TimingSummary {
    fn from_reports(reports: &[ExecutionResult]) -> Self {
        let mut durations = reports
            .iter()
            .map(|report| report.duration)
            .collect::<Vec<_>>();
        durations.sort_unstable();
        if durations.is_empty() {
            return Self {
                mean: Duration::ZERO,
                p50: Duration::ZERO,
                p95: Duration::ZERO,
                p99: Duration::ZERO,
            };
        }
        let total_nanos = durations.iter().map(Duration::as_nanos).sum::<u128>();
        let mean = Duration::from_nanos(
            u64::try_from(total_nanos / durations.len() as u128).unwrap_or(u64::MAX),
        );
        Self {
            mean,
            p50: percentile(&durations, 50),
            p95: percentile(&durations, 95),
            p99: percentile(&durations, 99),
        }
    }
}

fn percentile(sorted: &[Duration], percentile: usize) -> Duration {
    let rank = percentile.saturating_mul(sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

pub(crate) fn format_duration(duration: Duration) -> String {
    let millis = duration.as_secs_f64() * 1000.0;
    if millis < 1000.0 {
        format!("{millis:.1}ms")
    } else {
        format!("{:.3}s", duration.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_duration, parse_env, parse_memory, resolve_artifact};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn parses_existing_execution_flags() {
        assert_eq!(parse_env("MODE=production").unwrap().1, "production");
        assert_eq!(parse_duration("2s").unwrap().as_secs(), 2);
        assert_eq!(parse_memory("64MiB").unwrap(), 64 * 1024 * 1024);
        assert!(parse_env("missing-equals").is_err());
    }

    #[tokio::test]
    async fn missing_and_malformed_manifests_are_reported() {
        let project = std::env::temp_dir().join(format!("pit-cli-manifest-{}", std::process::id()));
        fs::create_dir_all(project.join(".pit")).unwrap();
        assert!(resolve_artifact(&project, None).await.is_err());
        fs::write(project.join(".pit/artifact.json"), b"not json").unwrap();
        let error = resolve_artifact(&project, None)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("malformed"));
        let _ = fs::remove_dir_all(project);
    }

    #[tokio::test]
    async fn explicit_artifact_paths_resolve_from_project() {
        let project = PathBuf::from("/tmp/pit-cli-project");
        assert_eq!(
            resolve_artifact(&project, Some(PathBuf::from("foo.wasm").as_path()))
                .await
                .unwrap(),
            project.join("foo.wasm")
        );
    }
}
