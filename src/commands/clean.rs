use std::fs;

use anyhow::Result;

pub fn run() -> Result<()> {
    let path = std::env::current_dir()?.join(".pit");
    if path.exists() {
        fs::remove_dir_all(&path)?;
        println!("✓ Removed PitFast build artifacts");
    } else {
        println!("nothing to clean");
    }
    Ok(())
}
