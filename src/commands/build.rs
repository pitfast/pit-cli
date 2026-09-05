use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use pit_builder_rust::RustBuilder;
use pit_crew::{BuildProfile, BuildRequest, PitCrew};

use crate::project;

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Select a binary target when the project has more than one.
    #[arg(long)]
    pub bin: Option<String>,
    /// Use Cargo's debug profile instead of the default release profile.
    #[arg(long)]
    pub debug: bool,
    /// Rebuild even when the cached build inputs and artifact are valid.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: BuildArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let profile = if args.debug {
        BuildProfile::Debug
    } else {
        BuildProfile::Release
    };
    let config = project::load(&project_dir)?;
    let defaults = project::execution_defaults(&config)?;
    let request = BuildRequest {
        project_dir: project_dir.clone(),
        bin: args.bin.or(config.build.bin),
        profile,
        execution_defaults: defaults,
        force: args.force,
    };
    let crew = PitCrew::with_adapter(RustBuilder::new());
    let outcome = crew.build_with_status(request).await?;
    let artifact = outcome.artifact;
    let display_path = artifact
        .artifact_path
        .strip_prefix(&project_dir)
        .map(PathBuf::from)
        .unwrap_or(artifact.artifact_path.clone());

    println!("PitCrew");
    println!();
    println!("Language: Rust");
    println!("Target: {}", artifact.manifest.build.target);
    println!("Profile: {}", artifact.manifest.build.profile.as_str());
    println!();
    if outcome.reused {
        println!("✓ Build inputs unchanged");
        println!("✓ Reusing cached artifact");
        println!();
    } else {
        println!("✓ Build completed");
        println!("✓ WASM validated");
        println!("✓ Artifact written");
        println!();
    }
    println!("{}", display_path.display());
    Ok(())
}
