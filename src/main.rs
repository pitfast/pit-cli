use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod manifest;
mod project;

#[derive(Debug, Parser)]
#[command(name = "pit", version, about = "PitFast developer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build the current project into a language-neutral PitFast WASM artifact.
    Build(commands::build::BuildArgs),
    /// Initialize PitFast project configuration and ignore generated output.
    Init(commands::init::InitArgs),
    /// Build, deploy, and prepare a Pit Manifest application.
    Up(commands::up::UpArgs),
    /// Resolve and validate an application plan without side effects.
    Plan(commands::plan::PlanArgs),
    /// Inspect the effective Pit Manifest/application plan.
    Config {
        #[command(subcommand)]
        command: commands::config::ConfigCommand,
    },
    /// Probe installed source-language toolchains and component support.
    Doctor {
        /// Emit a versioned machine-readable compatibility report.
        #[arg(long)]
        json: bool,
        /// Include dependency paths and low-level evidence.
        #[arg(long)]
        verbose: bool,
        /// Select a Pit Manifest explicitly.
        #[arg(short = 'f', long = "file")]
        file: Option<std::path::PathBuf>,
        #[command(subcommand)]
        command: Option<DoctorCommand>,
    },
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
    /// Store the current project artifact in local Paddock.
    Push(commands::paddock::PushArgs),
    /// Resolve and verify an artifact from local Paddock.
    Pull(commands::paddock::PullArgs),
    /// Inspect local Paddock refs and artifacts.
    Paddock {
        #[command(subcommand)]
        command: PaddockCommand,
    },
    /// Activate an immutable artifact for a logical service.
    Deploy(commands::deploy::DeployArgs),
    /// Roll a service back to a previous immutable revision.
    Rollback(commands::deploy::RollbackArgs),
    /// List coherent application releases, newest first.
    Releases {
        application: String,
        #[arg(long, default_value = "http://127.0.0.1:7081")]
        control_endpoint: String,
    },
    /// Stop routing new requests to a service without deleting history.
    Undeploy(commands::deploy::UndeployArgs),
    /// Inspect Circuit membership and Garage capacity.
    Circuit {
        #[command(subcommand)]
        command: CircuitCommand,
    },
    /// Inspect Garage membership and runtime capacity.
    Garage {
        #[command(subcommand)]
        command: GarageCommand,
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
    Inspect(commands::deploy::ServiceInspectArgs),
    List(commands::deploy::ServiceListArgs),
    History(commands::deploy::HistoryArgs),
}

#[derive(Debug, Subcommand)]
enum PaddockCommand {
    List(commands::paddock::ListArgs),
    Inspect(commands::paddock::InspectArgs),
}

#[derive(Debug, Subcommand)]
enum CircuitCommand {
    Status(commands::circuit::EndpointArgs),
    Snapshot(commands::circuit::EndpointArgs),
}

#[derive(Debug, Subcommand)]
enum GarageCommand {
    List(commands::circuit::EndpointArgs),
    Inspect(commands::circuit::GarageInspectArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum DoctorCommand {
    Languages,
    /// Inspect the local PitFast runtime and optional build toolchains.
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
        Command::Init(args) => commands::init::run(args),
        Command::Up(args) => commands::up::run(args).await,
        Command::Plan(args) => commands::plan::run(args),
        Command::Config { command } => match command {
            commands::config::ConfigCommand::Show(args) => commands::config::show(args),
        },
        Command::Doctor {
            json,
            verbose,
            file,
            command,
        } => commands::doctor::run(command, json, verbose, file).await,
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
            ServiceCommand::Inspect(args) => commands::deploy::service_inspect(args).await,
            ServiceCommand::List(args) => commands::deploy::service_list(args).await,
            ServiceCommand::History(args) => commands::deploy::service_history(args).await,
        },
        Command::Push(args) => commands::paddock::push(args).await,
        Command::Pull(args) => commands::paddock::pull(args).await,
        Command::Paddock { command } => match command {
            PaddockCommand::List(args) => commands::paddock::list(args).await,
            PaddockCommand::Inspect(args) => commands::paddock::inspect(args).await,
        },
        Command::Deploy(args) => commands::deploy::deploy(args).await,
        Command::Rollback(args) => commands::deploy::rollback(args).await,
        Command::Releases {
            application,
            control_endpoint,
        } => commands::deploy::application_releases_command(application, control_endpoint).await,
        Command::Undeploy(args) => commands::deploy::undeploy(args).await,
        Command::Circuit { command } => match command {
            CircuitCommand::Status(args) => commands::circuit::status(args).await,
            CircuitCommand::Snapshot(args) => commands::circuit::snapshot(args).await,
        },
        Command::Garage { command } => match command {
            GarageCommand::List(args) => commands::circuit::list(args).await,
            GarageCommand::Inspect(args) => commands::circuit::inspect(args).await,
        },
    }
}
