use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use pit_builder_rust::RustBuilder;
use pit_crew::{BuildProfile, BuildRequest, PitCrew};

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Select a binary target when the project has more than one.
    #[arg(long)]
    pub bin: Option<String>,
    /// Use Cargo's debug profile instead of the default release profile.
    #[arg(long)]
    pub debug: bool,
}

pub async fn run(args: BuildArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let profile = if args.debug {
        BuildProfile::Debug
    } else {
        BuildProfile::Release
    };
    let request = BuildRequest {
        project_dir: project_dir.clone(),
        bin: args.bin,
        profile,
    };
    let crew = PitCrew::with_adapter(RustBuilder::new());
    let artifact = crew.build(request).await?;
    let display_path = artifact
        .artifact_path
        .strip_prefix(&project_dir)
        .map(PathBuf::from)
        .unwrap_or(artifact.artifact_path.clone());

    println!("PitCrew");
    println!();
    println!("Language: Rust");
    println!("Target: {}", artifact.target);
    println!("Profile: {}", artifact.profile.as_str());
    println!();
    println!("✓ Build completed");
    println!("✓ WASM validated");
    println!("✓ Artifact written");
    println!();
    println!("{}", display_path.display());
    Ok(())
}
