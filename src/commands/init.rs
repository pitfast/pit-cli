use anyhow::Result;
use clap::Args;
use pit_crew::Language;

use crate::project;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Source language for the project. Defaults to Rust for existing projects.
    #[arg(long, default_value = "rust")]
    pub language: Language,
}

pub fn run(args: InitArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let created = project::init(&project_dir, args.language)?;
    println!("PitFast project initialized");
    if created {
        println!("✓ Created pit.toml");
    } else {
        println!("✓ Kept existing pit.toml");
    }
    println!("✓ .pit/ is ignored as generated output");
    println!();
    println!("Next steps:");
    println!("  pit build");
    println!("  pit inspect");
    println!("  pit run");
    Ok(())
}
