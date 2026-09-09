use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

#[derive(Debug, Subcommand)]
pub enum RouteCommand {
    /// Show the persisted immutable route snapshot.
    List(ListArgs),
    /// Route all traffic to one stable ApplicationRelease.
    Stable(ReleaseArgs),
    /// Switch atomically between stable and candidate releases.
    BlueGreen(BlueGreenArgs),
    /// Set weighted stable/candidate canary traffic.
    Canary(CanaryArgs),
    /// Set deterministic A/B traffic using a stable header key.
    Ab(AbArgs),
    /// Select candidate only when an explicit header says candidate.
    Header(HeaderArgs),
    /// Run safe GET/HEAD shadow traffic to a candidate release.
    Shadow(ShadowArgs),
    /// Remove a custom policy and return this route to active-release routing.
    Clear(CommonArgs),
}

#[derive(Debug, Args, Clone)]
pub struct CommonArgs {
    pub application: String,
    pub path: String,
    #[arg(long)]
    pub service: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
}

#[derive(Debug, Args)]
pub struct ReleaseArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub release: String,
}

#[derive(Debug, Args)]
pub struct BlueGreenArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub stable: String,
    #[arg(long)]
    pub candidate: String,
    #[arg(long, default_value = "stable")]
    pub active: String,
}

#[derive(Debug, Args)]
pub struct CanaryArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub stable: String,
    #[arg(long)]
    pub candidate: String,
    #[arg(long)]
    pub stable_weight: u8,
    #[arg(long)]
    pub candidate_weight: u8,
}

#[derive(Debug, Args)]
pub struct AbArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub a: String,
    #[arg(long)]
    pub b: String,
    #[arg(long, default_value_t = 50)]
    pub a_weight: u8,
    #[arg(long, default_value_t = 50)]
    pub b_weight: u8,
    #[arg(long, default_value = "X-Pit-User")]
    pub key_header: String,
    #[arg(long, default_value = "experiment")]
    pub experiment_id: String,
}

#[derive(Debug, Args)]
pub struct HeaderArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub stable: String,
    #[arg(long)]
    pub candidate: String,
    #[arg(long, default_value = "X-Pit-Variant")]
    pub header: String,
}

#[derive(Debug, Args)]
pub struct ShadowArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    #[arg(long)]
    pub stable: String,
    #[arg(long)]
    pub candidate: String,
}

pub async fn run(command: RouteCommand) -> Result<()> {
    match command {
        RouteCommand::List(args) => {
            let response = reqwest::get(format!(
                "{}/v1/routing/snapshot",
                args.control_endpoint.trim_end_matches('/')
            ))
            .await?;
            response.error_for_status_ref()?;
            println!("{}", serde_json::to_string_pretty::<Value>(&response.json().await?)?);
        }
        RouteCommand::Clear(args) => {
            post(
                &args.control_endpoint,
                "/v1/routing/clear",
                json!({
                    "application_id": args.application,
                    "host": args.host,
                    "path": args.path,
                }),
            )
            .await?
        }
        RouteCommand::Stable(args) => {
            post_policy(&args.common, json!({
                "stable_release": args.release,
            }))
            .await?
        }
        RouteCommand::BlueGreen(args) => {
            post_policy(&args.common, json!({
                "stable_release": args.stable,
                "candidate_release": args.candidate,
                "policy": {"type": "blue-green", "active": args.active},
            }))
            .await?
        }
        RouteCommand::Canary(args) => {
            post_policy(&args.common, json!({
                "stable_release": args.stable,
                "candidate_release": args.candidate,
                "policy": {"type": "weighted", "stable_weight": args.stable_weight, "candidate_weight": args.candidate_weight},
            }))
            .await?
        }
        RouteCommand::Ab(args) => {
            post_policy(&args.common, json!({
                "stable_release": args.a,
                "candidate_release": args.b,
                "a_release": args.a,
                "b_release": args.b,
                "policy": {"type": "deterministic", "a_weight": args.a_weight, "b_weight": args.b_weight, "key_header": args.key_header, "experiment_id": args.experiment_id},
            }))
            .await?
        }
        RouteCommand::Header(args) => {
            post_policy(&args.common, json!({
                "stable_release": args.stable,
                "candidate_release": args.candidate,
                "policy": {"type": "header-override", "header": args.header},
            }))
            .await?
        }
        RouteCommand::Shadow(args) => {
            post_policy(&args.common, json!({
                "stable_release": args.stable,
                "candidate_release": args.candidate,
                "policy": {"type": "shadow"},
                "shadow": args.candidate,
            }))
            .await?
        }
    }
    Ok(())
}

async fn post_policy(common: &CommonArgs, extra: Value) -> Result<()> {
    let inferred_service = common.service.clone().unwrap_or_else(|| {
        common
            .path
            .trim_matches('/')
            .split('/')
            .next()
            .unwrap_or("web")
            .to_owned()
    });
    let mut body = json!({
        "host": common.host,
        "path": common.path,
        "application_id": common.application,
        "service_id": inferred_service,
        "stable_release": "",
        "candidate_release": null,
        "a_release": null,
        "b_release": null,
        "policy": {"type": "stable"},
        "shadow": null,
    });
    if let (Some(target), Some(object)) = (body.as_object_mut(), extra.as_object()) {
        for (key, value) in object {
            target.insert(key.clone(), value.clone());
        }
    }
    if body["stable_release"] == "" {
        bail!("stable release is required");
    }
    post(&common.control_endpoint, "/v1/routing/policy", body).await
}

async fn post(endpoint: &str, path: &str, body: Value) -> Result<()> {
    let response = reqwest::Client::new()
        .post(format!("{}{path}", endpoint.trim_end_matches('/')))
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to reach PitLane control endpoint {endpoint}"))?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        bail!(
            "PitLane routing request failed ({status}): {}",
            String::from_utf8_lossy(&bytes)
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty::<Value>(&serde_json::from_slice(&bytes)?)?
    );
    Ok(())
}
