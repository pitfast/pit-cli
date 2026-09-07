use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct PlanArgs {
    #[arg(short = 'f', long = "file")]
    pub file: Option<PathBuf>,
}

/// Resolve and validate an application plan without building, preparing, or
/// mutating any deployment state.
pub fn run(args: PlanArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let plan = crate::manifest::resolve(&cwd, args.file.as_deref())?.plan()?;
    println!("Application: {}", plan.application_name);
    println!("Manifest: {}", plan.manifest.path.display());
    println!("Manifest digest: {}", plan.manifest.digest);
    println!("\nServices");
    for service in &plan.services {
        println!("  {}\t{}", service.id, service.project_dir.display());
    }
    if !plan.routes.is_empty() {
        println!("\nRoutes");
        for (path, service) in &plan.routes {
            println!("  {} → {}", path, service);
        }
    }
    if !plan.resources.is_empty() {
        println!("\nResources");
        for (id, resource) in &plan.resources {
            println!("  {}\t{}", id, resource.kind);
        }
    }
    Ok(())
}
