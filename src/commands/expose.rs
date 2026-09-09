use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Debug, Subcommand)]
pub enum ExposeCommand {
    /// Print the loopback listener and current logical routes.
    Local(EndpointArgs),
    /// Check for optional cloudflared exposure without changing PitLane.
    Cloudflare(CloudflareArgs),
}

#[derive(Debug, Args)]
pub struct EndpointArgs {
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
}

#[derive(Debug, Args)]
pub struct CloudflareArgs {
    #[arg(long)]
    pub hostname: Option<String>,
    #[arg(long, default_value = "http://127.0.0.1:7080")]
    pub origin: String,
}

pub async fn run(command: ExposeCommand) -> Result<()> {
    match command {
        ExposeCommand::Local(args) => {
            let endpoint = args.control_endpoint.trim_end_matches('/');
            let routing_response = reqwest::get(format!("{endpoint}/v1/routing/snapshot"))
                .await
                .context("failed to reach the local PitLane control endpoint")?;
            let snapshot: Value = routing_response.error_for_status()?.json().await?;
            let management_response = reqwest::get(format!("{endpoint}/v1/management/snapshot"))
                .await
                .context("failed to read the local PitLane management snapshot")?;
            let management: Value = management_response.error_for_status()?.json().await?;
            println!("PitLane Local\n\nListener:\n  http://127.0.0.1:7080\n");
            println!("RouteSnapshot generation: {}", snapshot["generation"]);
            println!("Routes:");
            let routes = management["routes"].as_array();
            for route in routes.into_iter().flatten() {
                println!(
                    "  {} {}{} → {}",
                    route["host"].as_str().unwrap_or("*"),
                    route["path"].as_str().unwrap_or("/"),
                    route["strategy"]
                        .as_str()
                        .filter(|strategy| *strategy != "stable")
                        .map_or(String::new(), |strategy| format!(" [{strategy}]")),
                    route["service_id"].as_str().unwrap_or("unknown"),
                );
            }
            if routes.is_none_or(|routes| routes.is_empty()) {
                println!("  (no logical routes configured)");
            }
        }
        ExposeCommand::Cloudflare(args) => {
            if which("cloudflared").is_none() {
                bail!(
                    "cloudflared is not installed; install it separately, then run 'pit expose cloudflare --hostname <name>'; PitLane remains available at {}",
                    args.origin
                );
            }
            println!("cloudflared detected. Origin remains {}.", args.origin);
            if let Some(hostname) = args.hostname {
                println!("Configured hostname: {hostname}");
            } else {
                println!("No hostname supplied; use an existing named tunnel/configuration.");
            }
            println!("PitLane keeps owning local routing; no credentials are stored by PitFast.");
        }
    }
    Ok(())
}

fn which(program: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join(program))
            .find(|path| path.is_file())
    })
}
