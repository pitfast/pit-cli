use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use pit_artifact::ArtifactManifest;
use pit_paddock_core::{
    ArtifactDigest, PaddockBackend, PaddockRef, pull as pull_artifact, push as push_artifact,
};
use pit_paddock_fs::FilesystemPaddock;

#[derive(Debug, Args)]
pub struct PushArgs {
    /// Mutable human-facing ref, for example service-a:v1.
    pub reference: String,
    /// Filesystem Paddock root. Defaults to the platform data directory.
    #[arg(long)]
    pub paddock_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct PullArgs {
    /// A name:tag ref or sha256:<64 lowercase hex> digest.
    pub artifact: String,
    /// Filesystem Paddock root. Defaults to the platform data directory.
    #[arg(long)]
    pub paddock_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub paddock_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct InspectArgs {
    pub reference: String,
    #[arg(long)]
    pub paddock_dir: Option<PathBuf>,
}

pub async fn push(args: PushArgs) -> Result<()> {
    let reference: PaddockRef = args.reference.parse()?;
    let project = std::env::current_dir()?;
    let manifest_path = project.join(".pit/artifact.json");
    let manifest = ArtifactManifest::load(&manifest_path)?;
    let artifact_path = manifest.verify_artifact(&project)?;
    let bytes = tokio::fs::read(&artifact_path).await?;
    let paddock = FilesystemPaddock::new(paddock_root(args.paddock_dir)?);
    let (stored, blob) = push_artifact(&paddock, &reference, manifest, &bytes).await?;
    println!("Paddock");
    println!();
    println!("Artifact: {}", stored.manifest.artifact.name);
    println!("Digest: {}", stored.digest);
    println!("Size: {} bytes", stored.size_bytes);
    println!();
    println!("✓ Artifact verified");
    println!(
        "✓ Blob {}",
        match blob {
            pit_paddock_core::BlobPut::Uploaded => "uploaded",
            pit_paddock_core::BlobPut::AlreadyPresent => "already present",
        }
    );
    println!("✓ Manifest uploaded");
    println!("✓ Ref published");
    println!();
    println!("{}", reference);
    println!("→ {}", stored.digest);
    Ok(())
}

pub async fn pull(args: PullArgs) -> Result<()> {
    let paddock = FilesystemPaddock::new(paddock_root(args.paddock_dir)?);
    let (reference, digest) = parse_locator(&args.artifact)?;
    let stored = pull_artifact(&paddock, reference.as_ref(), digest.as_ref()).await?;
    let project = std::env::current_dir()?;
    let cache = project
        .join(".pit/cache/sha256")
        .join(format!("{}.wasm", stored.digest.hex()));
    write_atomic(&cache, &paddock.get_blob(&stored.digest).await?).await?;
    println!("Resolved:");
    if let Some(reference) = reference {
        println!("{}", reference);
    }
    println!("→ {}", stored.digest);
    println!();
    println!("✓ Manifest verified");
    println!("✓ Artifact downloaded and cached");
    println!("✓ SHA-256 verified");
    println!("{}", cache.display());
    Ok(())
}

pub async fn list(args: ListArgs) -> Result<()> {
    let paddock = FilesystemPaddock::new(paddock_root(args.paddock_dir)?);
    let refs = paddock.list_refs(None).await?;
    if refs.is_empty() {
        println!("No Paddock refs found.");
        return Ok(());
    }
    println!("REFERENCE                 DIGEST");
    for value in refs {
        let reference = PaddockRef::try_from(value.reference)?;
        println!("{:<25} {}", reference, value.digest);
    }
    Ok(())
}

pub async fn inspect(args: InspectArgs) -> Result<()> {
    let reference: PaddockRef = args.reference.parse()?;
    let paddock = FilesystemPaddock::new(paddock_root(args.paddock_dir)?);
    let stored = pull_artifact(&paddock, Some(&reference), None).await?;
    println!("Reference");
    println!("  {}", reference);
    println!();
    println!("Digest");
    println!("  {}", stored.digest);
    println!();
    println!("Artifact");
    println!("  Size: {} bytes", stored.size_bytes);
    println!("  ABI: {}", stored.manifest.runtime.abi);
    println!(
        "  World: {}",
        stored
            .manifest
            .runtime
            .world
            .map_or("none", |world| world.as_str())
    );
    println!("  Format: {}", stored.manifest.runtime.format);
    println!();
    println!("Integrity");
    println!("  ✓ manifest valid");
    println!("  ✓ digest valid");
    Ok(())
}

fn parse_locator(value: &str) -> Result<(Option<PaddockRef>, Option<ArtifactDigest>)> {
    if value.starts_with("sha256:") {
        Ok((None, Some(value.parse()?)))
    } else {
        Ok((Some(value.parse()?), None))
    }
}

fn paddock_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Some(path) = std::env::var_os("PIT_PADDOCK_ROOT") {
        return Ok(path.into());
    }
    if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path).join("pit/paddock"));
    }
    if let Some(path) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(path).join(".local/share/pit/paddock"));
    }
    bail!("unable to determine Paddock root; pass --paddock-dir")
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("cache path has no parent"))?;
    tokio::fs::create_dir_all(parent).await?;
    let temp = path.with_extension("tmp");
    tokio::fs::write(&temp, bytes).await?;
    tokio::fs::rename(&temp, path)
        .await
        .with_context(|| format!("failed to promote cache file {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::parse_locator;

    #[test]
    fn locator_distinguishes_refs_and_digests() {
        let (reference, digest) = parse_locator("service-a:v1").unwrap();
        assert_eq!(reference.unwrap().to_string(), "service-a:v1");
        assert!(digest.is_none());
        let (reference, digest) = parse_locator(
            "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        )
        .unwrap();
        assert!(reference.is_none());
        assert!(digest.is_some());
        assert!(parse_locator("../../etc:latest").is_err());
    }
}
