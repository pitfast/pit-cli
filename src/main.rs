use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod project;

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
    /// Initialize PitFast project configuration and ignore generated output.
    Init,
    /// Run a WASI Preview 1 or Preview 2 artifact through PitBox.
    Run(commands::run::RunArgs),
    /// Run a local concurrency benchmark through PitBox.
    Bench(commands::bench::BenchArgs),
    /// Inspect a project-managed artifact manifest and verify its integrity.
    Inspect(commands::inspect::InspectArgs),
    /// Remove only the current project's generated .pit directory.
    Clean,
    /// Display local PitBox hardware and scheduler information.
    System,
    /// Call a logical PitFast HTTP service through the local PitLane.
    Call(commands::call::CallArgs),
    /// Inspect configured PostgreSQL resources.
    Resource {
        #[command(subcommand)]
        command: ResourceCommand,
    },
    /// Inspect configured logical services.
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ResourceCommand {
    List,
    Inspect(commands::resource::ResourceInspectArgs),
    Check(commands::resource::ResourceInspectArgs),
}

#[derive(Debug, Subcommand)]
enum ServiceCommand {
    Inspect(commands::service::InspectArgs),
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
        Command::Init => commands::init::run(),
        Command::Run(args) => commands::run::run(args).await,
        Command::Bench(args) => commands::bench::run(args).await,
        Command::Inspect(args) => commands::inspect::run(args),
        Command::Clean => commands::clean::run(),
        Command::System => commands::system::run(),
        Command::Call(args) => commands::call::run(args).await,
        Command::Resource { command } => match command {
            ResourceCommand::List => commands::resource::list(),
            ResourceCommand::Inspect(args) => commands::resource::inspect(args),
            ResourceCommand::Check(args) => commands::resource::check(args),
        },
        Command::Service { command } => match command {
            ServiceCommand::Inspect(args) => commands::service::inspect(args),
        },
    }
}
