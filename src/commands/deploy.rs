use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use pit_artifact::ArtifactManifest;
use pit_deployment::{
    DeployRequest, DeploymentView, LocalArtifactStore, RollbackRequest, UndeployRequest,
};
use pit_lane_core::ServiceId;
use pit_paddock_core::{ArtifactDigest, PaddockBackend, PaddockRef, PaddockRefWire, pull};
use pit_paddock_fs::FilesystemPaddock;
use reqwest::Client;

#[derive(Debug, Args)]
pub struct DeployArgs {
    pub service: String,
    /// Paddock ref or sha256 digest. Omit with --local.
    pub selector: Option<String>,
    /// Deploy the current project's verified .pit/artifact.json.
    #[arg(long)]
    pub local: bool,
    #[arg(long)]
    pub paddock_dir: Option<PathBuf>,
    #[arg(long)]
    pub artifact_store: Option<PathBuf>,
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
    #[arg(long)]
    pub expected_generation: Option<u64>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct RollbackArgs {
    pub service: String,
    #[arg(long)]
    pub generation: Option<u64>,
    #[arg(long)]
    pub to: Option<String>,
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
    #[arg(long)]
    pub artifact_store: Option<PathBuf>,
    #[arg(long)]
    pub expected_generation: Option<u64>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct UndeployArgs {
    pub service: String,
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
    #[arg(long)]
    pub expected_generation: Option<u64>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    pub id: String,
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
}

#[derive(Debug, Args)]
pub struct ServiceListArgs {
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
}

#[derive(Debug, Args)]
pub struct ServiceInspectArgs {
    pub id: String,
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
}

pub async fn deploy(args: DeployArgs) -> Result<()> {
    let service: ServiceId = args.service.parse()?;
    if args.local == args.selector.is_some() {
        bail!("provide exactly one of an artifact selector or --local")
    }
    let store = LocalArtifactStore::new(match args.artifact_store {
        Some(path) => path,
        None => LocalArtifactStore::default_root()?,
    });
    let (digest, artifact_path, manifest_path, source_ref) = if args.local {
        let project = std::env::current_dir()?;
        let manifest_path = project.join(".pit/artifact.json");
        let manifest = ArtifactManifest::load(&manifest_path)?;
        let artifact_path = manifest.verify_artifact(&project)?;
        let bytes = tokio::fs::read(&artifact_path).await?;
        let (artifact_path, manifest_path) = store.install(&manifest, &bytes)?;
        let digest: ArtifactDigest = format!("sha256:{}", manifest.artifact.sha256).parse()?;
        (digest, artifact_path, manifest_path, None)
    } else {
        let selector = args.selector.as_deref().unwrap();
        let (reference, digest) = parse_selector(selector)?;
        let paddock = FilesystemPaddock::new(paddock_root(args.paddock_dir.clone())?);
        let stored = pull(&paddock, reference.as_ref(), digest.as_ref()).await?;
        let bytes = paddock.get_blob(&stored.digest).await?;
        let (artifact_path, manifest_path) = store.install(&stored.manifest, &bytes)?;
        (
            stored.digest,
            artifact_path,
            manifest_path,
            reference.map(|value| PaddockRefWire::from(&value)),
        )
    };
    let response: DeploymentView = post_json(
        &args.control_endpoint,
        "/v1/deploy",
        &DeployRequest {
            service_id: service.to_string(),
            digest: digest.clone(),
            artifact_path: artifact_path.to_string_lossy().into_owned(),
            manifest_path: manifest_path.to_string_lossy().into_owned(),
            source_ref,
            source_paddock: Some("local".into()),
            expected_generation: args.expected_generation,
            force: args.force,
        },
    )
    .await?;
    println!("PitFast Deployment\n");
    println!("Service: {}", service);
    println!("✓ Artifact verified");
    println!("✓ Component prepared");
    println!("✓ Deployment state committed");
    println!("✓ Service activated");
    println!(
        "\n{}\ngeneration {}\n{}",
        service, response.state.generation, digest
    );
    Ok(())
}

pub async fn rollback(args: RollbackArgs) -> Result<()> {
    if args.generation.is_some() && args.to.is_some() {
        bail!("use only one of --generation or --to")
    }
    let target_digest = args.to.map(|value| value.parse()).transpose()?;
    let view: DeploymentView = post_json(
        &args.control_endpoint,
        "/v1/rollback",
        &RollbackRequest {
            service_id: args.service,
            target_generation: args.generation,
            target_digest,
            expected_generation: args.expected_generation,
            force: args.force,
        },
    )
    .await
    .context("rollback failed")?;
    println!("✓ Rollback activated generation {}", view.state.generation);
    if let Some(current) = view.state.current {
        println!("{} → {}", view.state.service_id, current.digest);
    }
    Ok(())
}

pub async fn undeploy(args: UndeployArgs) -> Result<()> {
    let service = args.service.clone();
    let view: DeploymentView = post_json(
        &args.control_endpoint,
        "/v1/undeploy",
        &UndeployRequest {
            service_id: args.service,
            expected_generation: args.expected_generation,
            force: args.force,
        },
    )
    .await?;
    println!(
        "✓ Undeployed {service} at generation {}",
        view.state.generation
    );
    Ok(())
}

pub async fn service_list(args: ServiceListArgs) -> Result<()> {
    let views: Vec<DeploymentView> = get_json(&args.control_endpoint, "/v1/services").await?;
    println!("SERVICE\tSTATUS\tGENERATION\tDIGEST\tSOURCE");
    for view in views {
        let digest = view
            .state
            .current
            .as_ref()
            .map_or("-".into(), |r| r.digest.to_string());
        let source = view
            .state
            .current
            .as_ref()
            .and_then(|r| r.source_ref.as_ref())
            .map_or("-".into(), |r| format!("{}:{}", r.name, r.tag));
        println!(
            "{}\t{:?}\t{}\t{}\t{}",
            view.state.service_id, view.status, view.state.generation, digest, source
        );
    }
    Ok(())
}

pub async fn service_history(args: HistoryArgs) -> Result<()> {
    let view: DeploymentView =
        get_json(&args.control_endpoint, &format!("/v1/services/{}", args.id)).await?;
    println!("GEN\tSTATUS\tDIGEST\tSOURCE");
    if let Some(current) = &view.state.current {
        println!(
            "{}\tcurrent\t{}\t{}",
            current.generation,
            current.digest,
            source(current)
        );
    }
    for revision in &view.state.history {
        println!(
            "{}\t\t{}\t{}",
            revision.generation,
            revision.digest,
            source(revision)
        );
    }
    Ok(())
}

pub async fn service_inspect(args: ServiceInspectArgs) -> Result<()> {
    let view: DeploymentView =
        get_json(&args.control_endpoint, &format!("/v1/services/{}", args.id)).await?;
    println!(
        "Service\n  ID: {}\n  Status: {:?}\n  Generation: {}",
        view.state.service_id, view.status, view.state.generation
    );
    if let Some(current) = view.state.current {
        println!(
            "\nDeployment\n  Digest: {}\n  Source ref: {}",
            current.digest,
            source(&current)
        );
    }
    println!(
        "\nArtifact\n  Local: {}\n  Prepared: {}",
        view.artifact_local, view.prepared
    );
    if let Some(reason) = view.unavailable_reason {
        println!("  Reason: {reason}");
    }
    if view.resource_bindings.is_empty() {
        println!("\nResources\n  none");
    } else {
        println!("\nResources");
        for (variable, resource) in view.resource_bindings {
            println!("  {variable} → {resource}");
        }
    }
    Ok(())
}

fn source(revision: &pit_deployment::DeploymentRevision) -> String {
    revision
        .source_ref
        .as_ref()
        .map_or("-".into(), |r| format!("{}:{}", r.name, r.tag))
}

async fn post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
    endpoint: &str,
    path: &str,
    body: &T,
) -> Result<R> {
    let response = Client::new()
        .post(format!("{}{}", endpoint.trim_end_matches('/'), path))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(body)?)
        .send()
        .await?;
    parse_response(response).await
}

async fn get_json<R: serde::de::DeserializeOwned>(endpoint: &str, path: &str) -> Result<R> {
    parse_response(
        Client::new()
            .get(format!("{}{}", endpoint.trim_end_matches('/'), path))
            .send()
            .await?,
    )
    .await
}

async fn parse_response<R: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<R> {
    let status = response.status();
    let body = response.bytes().await?;
    if !status.is_success() {
        bail!(
            "PitLane control returned HTTP {}: {}",
            status,
            String::from_utf8_lossy(&body)
        );
    }
    serde_json::from_slice(&body).context("malformed PitLane control response")
}

fn parse_selector(value: &str) -> Result<(Option<PaddockRef>, Option<ArtifactDigest>)> {
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
    Err(anyhow!("unable to determine Paddock root"))
}
