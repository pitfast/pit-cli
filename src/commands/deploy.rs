use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use pit_artifact::ArtifactManifest;
use pit_deployment::{
    ArtifactAcquirer, DeployRequest, DeploymentStageDurations, DeploymentView, LocalArtifactStore,
    RollbackRequest, UndeployRequest,
};
use pit_lane_core::ServiceId;
use pit_paddock_core::{ArtifactDigest, PaddockRef, PaddockRefWire};
use pit_paddock_factory::PaddockConfig;
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
    /// Named Paddock from pit.toml or user configuration.
    #[arg(long)]
    pub paddock: Option<String>,
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
    /// Named Paddock used to reacquire a historical digest.
    #[arg(long)]
    pub paddock: Option<String>,
    #[arg(long)]
    pub paddock_dir: Option<PathBuf>,
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
    let project = std::env::current_dir()?;
    let paddock_config = PaddockConfig::load(&project)?;
    let paddock_name = args
        .paddock
        .clone()
        .unwrap_or_else(|| paddock_config.default_name().to_owned());
    let initial_generation = if args.force || args.expected_generation.is_some() {
        None
    } else {
        Some(current_generation(&args.control_endpoint, &service).await?)
    };
    let mut ref_resolution_ms = 0;
    let (digest, artifact_path, manifest_path, source_ref, source_paddock, acquisition) =
        if args.local {
            let manifest_path = project.join(".pit/artifact.json");
            let manifest = ArtifactManifest::load(&manifest_path)?;
            let artifact_path = manifest.verify_artifact(&project)?;
            let bytes = tokio::fs::read(&artifact_path).await?;
            let (artifact_path, manifest_path) = store.install(&manifest, &bytes)?;
            let digest: ArtifactDigest = format!("sha256:{}", manifest.artifact.sha256).parse()?;
            (digest, artifact_path, manifest_path, None, None, None)
        } else {
            let selector = args.selector.as_deref().unwrap();
            let (reference, digest) = parse_selector(selector)?;
            if let Some(digest) = &digest
                && let Ok((_manifest, artifact_path)) = store.open(digest)
            {
                (
                    digest.clone(),
                    artifact_path,
                    store.manifest_path(digest),
                    None,
                    args.paddock.clone(),
                    None,
                )
            } else {
                let paddock = paddock_config.open(&paddock_name, args.paddock_dir.clone())?;
                let resolve_started = std::time::Instant::now();
                let resolved = match (&reference, digest) {
                    (Some(reference), None) => paddock.resolve_ref(reference).await?,
                    (None, Some(digest)) => digest,
                    _ => bail!("provide exactly one artifact selector"),
                };
                ref_resolution_ms = resolve_started.elapsed().as_millis();
                let acquired = ArtifactAcquirer::default()
                    .ensure_local(&resolved, &paddock_name, &*paddock, &store)
                    .await?;
                (
                    resolved,
                    acquired.artifact_path.clone(),
                    acquired.manifest_path.clone(),
                    reference.map(|value| PaddockRefWire::from(&value)),
                    Some(paddock_name.clone()),
                    Some(acquired),
                )
            }
        };
    let acquisition_timings = acquisition.as_ref().map(|value| DeploymentStageDurations {
        ref_resolution_ms,
        artifact_acquisition_ms: value.duration.as_millis(),
        artifact_validation_ms: value.validation_duration.as_millis(),
        local_install_ms: value.install_duration.as_millis(),
        ..DeploymentStageDurations::default()
    });
    let response: DeploymentView = post_json(
        &args.control_endpoint,
        "/v1/deploy",
        &DeployRequest {
            service_id: service.to_string(),
            digest: digest.clone(),
            artifact_path: artifact_path.to_string_lossy().into_owned(),
            manifest_path: manifest_path.to_string_lossy().into_owned(),
            source_ref,
            source_paddock,
            expected_generation: args.expected_generation.or(initial_generation),
            force: args.force,
            stage_durations: acquisition_timings,
        },
    )
    .await?;
    println!("PitFast Deployment\n");
    println!("Service: {}", service);
    if let Some(acquisition) = acquisition {
        if acquisition.cache_hit {
            println!("✓ Artifact already available locally");
        } else {
            println!("✓ Artifact acquired from {}", paddock_name);
            println!("  {} bytes", acquisition.bytes);
        }
    }
    println!("✓ Artifact verified");
    println!("✓ Component prepared");
    println!("✓ Deployment state committed");
    println!("✓ Service activated");
    if let Some(timing) = response.timings {
        println!("\nTiming");
        println!("  Resolve:    {} ms", timing.ref_resolution_ms);
        println!("  Acquire:    {} ms", timing.artifact_acquisition_ms);
        println!("  Validate:   {} ms", timing.artifact_validation_ms);
        println!("  Install:    {} ms", timing.local_install_ms);
        println!("  Prepare:    {} ms", timing.artifact_preparation_ms);
        println!("  Commit:     {} ms", timing.state_commit_ms);
        println!("  Activate:   {} ms", timing.registry_activation_ms);
        println!("  Total:      {} ms", timing.total_ms);
    }
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
    let service: ServiceId = args.service.parse()?;
    let current: DeploymentView =
        get_json(&args.control_endpoint, &format!("/v1/services/{service}"))
            .await
            .context("failed to read deployment history")?;
    let target = if let Some(value) = args.to.as_deref() {
        let digest: ArtifactDigest = value.parse()?;
        current
            .state
            .history
            .iter()
            .find(|revision| revision.digest == digest)
            .cloned()
            .ok_or_else(|| anyhow!("digest is not in deployment history"))?
    } else if let Some(generation) = args.generation {
        current
            .state
            .history
            .iter()
            .find(|revision| revision.generation == generation)
            .cloned()
            .ok_or_else(|| anyhow!("deployment generation {generation} is not in history"))?
    } else {
        current
            .state
            .history
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("no previous deployment revision exists"))?
    };
    let store = LocalArtifactStore::new(match args.artifact_store {
        Some(path) => path,
        None => LocalArtifactStore::default_root()?,
    });
    if !store.contains(&target.digest) {
        let config = PaddockConfig::load(&std::env::current_dir()?)?;
        let name = args
            .paddock
            .clone()
            .or(target.source_paddock.clone())
            .unwrap_or_else(|| config.default_name().to_owned());
        let paddock = config.open(&name, args.paddock_dir.clone())?;
        ArtifactAcquirer::default()
            .ensure_local(&target.digest, &name, &*paddock, &store)
            .await
            .with_context(|| {
                format!("failed to reacquire historical artifact {}", target.digest)
            })?;
    } else {
        store.open(&target.digest)?;
    }
    let target_digest = Some(target.digest.clone());
    let view: DeploymentView = post_json(
        &args.control_endpoint,
        "/v1/rollback",
        &RollbackRequest {
            service_id: args.service,
            target_generation: args.generation,
            target_digest: target_digest.clone(),
            expected_generation: args.expected_generation.or(Some(current.state.generation)),
            force: args.force,
        },
    )
    .await
    .context("rollback failed")?;
    println!(
        "✓ Historical artifact {} available",
        target_digest.as_ref().unwrap()
    );
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

async fn current_generation(endpoint: &str, service: &ServiceId) -> Result<u64> {
    let response = Client::new()
        .get(format!(
            "{}/v1/services/{service}",
            endpoint.trim_end_matches('/')
        ))
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(0);
    }
    let view: DeploymentView = parse_response(response).await?;
    Ok(view.state.generation)
}
