use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Resolve, validate, and print the normalized Pit Manifest.
    Show(ConfigShowArgs),
}

#[derive(Debug, Args)]
pub struct ConfigShowArgs {
    #[arg(short = 'f', long = "file")]
    pub file: Option<PathBuf>,
}

pub fn show(args: ConfigShowArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let resolved = crate::manifest::resolve(&cwd, args.file.as_deref())?;
    let plan = resolved.plan()?;
    println!("Manifest: {}", plan.manifest.path.display());
    println!("Digest: {}", plan.manifest.digest);
    println!("\n{}", toml::to_string_pretty(&plan.manifest.manifest)?);
    println!("Plan\n  application: {}", plan.application_name);
    for service in plan.services {
        println!(
            "  service {} → {}",
            service.id,
            service.project_dir.display()
        );
    }
    Ok(())
}
