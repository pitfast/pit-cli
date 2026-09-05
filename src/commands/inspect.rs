use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::Args;
use pit_artifact::{ArtifactManifest, manifest_path};

#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Manifest path. Defaults to the current project's .pit/artifact.json.
    pub manifest: Option<PathBuf>,
}

pub fn run(args: InspectArgs) -> Result<()> {
    let current = std::env::current_dir()?;
    let manifest_file = args
        .manifest
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                current.join(path)
            }
        })
        .unwrap_or_else(|| manifest_path(&current));
    let project_dir = project_dir_for_manifest(&current, &manifest_file);
    let manifest = ArtifactManifest::load(&manifest_file)?;

    println!("PitFast Artifact");
    println!();
    println!("Name: {}", manifest.artifact.name);
    println!("Schema: {}", manifest.schema_version);
    println!();
    println!("Build");
    println!("  Language: {}", manifest.build.language);
    println!("  Target: {}", manifest.build.target);
    println!("  Profile: {}", manifest.build.profile.as_str());
    println!("  Fingerprint: {}", manifest.build.fingerprint);
    println!();
    println!("Runtime");
    println!("  ABI: {}", manifest.runtime.abi.as_str());
    println!("  Entrypoint: {}", manifest.runtime.entrypoint);
    println!();
    println!("Artifact");
    println!(
        "  Path: {}",
        project_dir
            .join(".pit")
            .join(&manifest.artifact.path)
            .strip_prefix(&project_dir)
            .unwrap_or(Path::new("."))
            .display()
    );
    println!("  Size: {}", format_size(manifest.artifact.size_bytes));
    println!("  SHA-256: {}", manifest.artifact.sha256);
    println!();
    println!("Execution Defaults");
    println!(
        "  Timeout: {}",
        manifest
            .execution
            .timeout_ms
            .map_or_else(|| "none".into(), |value| format!("{value}ms"))
    );
    println!(
        "  Memory: {}",
        manifest
            .execution
            .memory_bytes
            .map_or_else(|| "none".into(), format_bytes)
    );
    println!();
    println!("Capabilities");
    for capability in &manifest.capabilities {
        println!("  {}", capability.as_str());
    }
    println!();
    println!("Integrity");
    let mut failed = false;
    match manifest.verify_artifact(&project_dir) {
        Ok(_) => println!("  ✓ artifact exists"),
        Err(error) => {
            println!("  ✗ artifact integrity: {error}");
            failed = true;
        }
    }
    match manifest.validate_runtime_compatibility() {
        Ok(()) => println!("  ✓ runtime ABI supported"),
        Err(error) => {
            println!("  ✗ runtime compatibility: {error}");
            failed = true;
        }
    }
    if failed {
        bail!("PitFast artifact inspection failed");
    }
    Ok(())
}

fn project_dir_for_manifest(current: &Path, manifest: &Path) -> PathBuf {
    if manifest.file_name().and_then(|name| name.to_str()) == Some("artifact.json")
        && manifest
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(".pit")
    {
        return manifest
            .parent()
            .and_then(Path::parent)
            .unwrap_or(current)
            .to_path_buf();
    }
    current.to_path_buf()
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes.is_multiple_of(1024 * 1024) {
        format!("{} MiB", bytes / (1024 * 1024))
    } else {
        format!("{bytes} bytes")
    }
}
