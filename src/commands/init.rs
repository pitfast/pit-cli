use anyhow::Result;

use crate::project;

pub fn run() -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let created = project::init(&project_dir)?;
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
