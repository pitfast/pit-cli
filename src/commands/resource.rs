use anyhow::{Result, bail};
use clap::Args;

use crate::project;

#[derive(Debug, Args)]
pub struct ResourceInspectArgs {
    pub id: String,
}

pub fn list() -> Result<()> {
    let config = project::load(&std::env::current_dir()?)?;
    if config.resources.is_empty() {
        println!("No PitFast resources configured.");
        return Ok(());
    }
    println!("RESOURCE     KIND       PROVIDER       STATUS");
    for resource in config.resources {
        println!(
            "{:<12} {:<10} {:<14} configured",
            resource.id, resource.kind, resource.provider
        );
    }
    Ok(())
}

pub fn inspect(args: ResourceInspectArgs) -> Result<()> {
    let config = project::load(&std::env::current_dir()?)?;
    let resource = config
        .resources
        .iter()
        .find(|resource| resource.id == args.id)
        .ok_or_else(|| anyhow::anyhow!("unknown resource '{}'", args.id))?;
    println!("Resource: {}", resource.id);
    println!("Kind: {}", resource.kind);
    println!("Provider: {}", resource.provider);
    println!("URL environment variable: {}", resource.url_env);
    println!("Secret value: redacted");
    Ok(())
}

pub fn check(args: ResourceInspectArgs) -> Result<()> {
    let config = project::load(&std::env::current_dir()?)?;
    let resource = config
        .resources
        .iter()
        .find(|resource| resource.id == args.id)
        .ok_or_else(|| anyhow::anyhow!("unknown resource '{}'", args.id))?;
    if std::env::var(&resource.url_env).is_err() {
        bail!(
            "resource URL environment variable {} is not set",
            resource.url_env
        )
    }
    println!(
        "{}: configured (connectivity requires the PitFast gateway)",
        resource.id
    );
    Ok(())
}
