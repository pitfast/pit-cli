use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Debug, Parser)]
#[command(name = "pit", about = "PitFast developer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build the current Rust project into a PitFast WASM artifact.
    Build(commands::build::BuildArgs),
    /// Run a WASI Preview 1 artifact through PitBox.
    Run(commands::run::RunArgs),
    /// Run a local concurrency benchmark through PitBox.
    Bench(commands::bench::BenchArgs),
    /// Display local PitBox hardware and scheduler information.
    System,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .without_time()
        .init();

    match Cli::parse().command {
        Command::Build(args) => commands::build::run(args).await,
        Command::Run(args) => commands::run::run(args).await,
        Command::Bench(args) => commands::bench::run(args).await,
        Command::System => commands::system::run(),
    }
}
