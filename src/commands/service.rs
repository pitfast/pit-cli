use anyhow::Result;
use clap::Args;

use crate::project;

#[derive(Debug, Args)]
pub struct InspectArgs {
    pub id: String,
}

pub fn inspect(args: InspectArgs) -> Result<()> {
    let config = project::load(&std::env::current_dir()?)?;
    let name = config
        .service
        .name
        .as_deref()
        .or(config.project.name.as_deref())
        .unwrap_or("current-project");
    if name != args.id {
        anyhow::bail!("service '{}' is not configured in this project", args.id)
    }
    println!("Service: {name}");
    if let Some(artifact) = config.service.artifact {
        println!("Artifact: {artifact}");
    }
    if config.service.resources.is_empty() {
        println!("Resources: none");
    } else {
        println!("Resources");
        for (binding, resource) in config.service.resources {
            println!("{binding} → {resource}");
        }
    }
    Ok(())
}
