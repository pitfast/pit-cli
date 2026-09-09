use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use pit_artifact::{BuildProfile, ComponentWorld};

use crate::commands::{build, deploy};

#[derive(Debug, Args)]
pub struct UpArgs {
    /// Select a Pit Manifest. Without this flag, the current directory's
    /// only *.pit file (or app.pit tie-break) is used.
    #[arg(short = 'f', long = "file")]
    pub file: Option<PathBuf>,
    /// Existing local PitLane deployment control endpoint.
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
    /// Host-local verified artifact store used by deployment.
    #[arg(long)]
    pub artifact_store: Option<PathBuf>,
    /// Re-activate even when the service generation has changed.
    #[arg(long)]
    pub force: bool,
    /// Use Cargo debug/source builder profiles where applicable.
    #[arg(long)]
    pub debug: bool,
    /// Print a control-plane timing breakdown for this activation.
    #[arg(long)]
    pub timings: bool,
}

pub async fn run(args: UpArgs) -> Result<()> {
    let lifecycle_started = Instant::now();
    let cwd = std::env::current_dir()?;
    let resolve_started = Instant::now();
    let resolved = crate::manifest::resolve(&cwd, args.file.as_deref())?;
    let plan = resolved.plan()?;
    let resolve_ms = resolve_started.elapsed().as_millis();
    let profile = if args.debug {
        BuildProfile::Debug
    } else {
        BuildProfile::Release
    };
    let crew = build::default_crew();

    println!(
        "PitFast\n\nManifest\n  {}\n  digest {}",
        plan.manifest.path.display(),
        plan.manifest.digest
    );
    println!("\nApplication\n  {}", plan.application_name);
    println!("\nServices");

    // Preflight every service before any build or deployment side effect.
    // This prevents an incompatible later service from activating an earlier
    // service merely because it happened to be listed first.
    let doctor_started = Instant::now();
    for service in &plan.services {
        let request = build::request_for_service(&plan, service, profile, args.force)?;
        if let Some(report) = crew.compatibility(&service.project_dir, &request)?
            && report.blocks_build()
        {
            return Err(anyhow::anyhow!(
                "service '{}' compatibility check failed before side effects:\n{}",
                service.id,
                report
                    .findings
                    .iter()
                    .filter(|finding| finding.blocks_build)
                    .map(|finding| format!("  [{}] {}", finding.category, finding.reason))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }
    let doctor_ms = doctor_started.elapsed().as_millis();

    let build_started = Instant::now();
    for service in &plan.services {
        let outcome = build::build_service(&plan, service, profile, args.force)
            .await
            .with_context(|| format!("failed to build service '{}'", service.id))?;
        let state = if outcome.reused { "cached" } else { "built" };
        let interface = outcome
            .artifact
            .manifest
            .build
            .application_interface
            .as_deref()
            .unwrap_or("native");
        println!(
            "  ✓ {:<12} {:<14} {:<8} {}",
            service.id, interface, state, outcome.artifact.manifest.artifact.sha256
        );
        if outcome.artifact.manifest.runtime.world != Some(ComponentWorld::WasiHttpProxy) {
            bail!(
                "service '{}' built as {}, but pit up deployment currently requires wasi:http/proxy",
                service.id,
                outcome.artifact.manifest.runtime.entrypoint.as_str()
            );
        }
    }
    let build_ms = build_started.elapsed().as_millis();

    if !plan.resources.is_empty() {
        println!("\nResources");
        for (id, resource) in &plan.resources {
            println!("  ✓ {}  {} <- {}", id, resource.kind, resource.source);
        }
    }
    if !plan.routes.is_empty() {
        println!("\nRoutes");
        for (path, service) in &plan.routes {
            println!("  {} → {}", path, service);
        }
    }

    println!("\nDeployment");
    ensure_bundled_pit_lane(&args.control_endpoint, args.artifact_store.as_deref()).await?;
    let prepare_started = Instant::now();
    let mut services = BTreeMap::new();
    for service in &plan.services {
        let bindings = service
            .spec
            .resources
            .iter()
            .map(|(variable, resource)| (variable.clone(), resource.clone()))
            .collect();
        let prepared = deploy::prepare_local_release_service(
            &service.id,
            &service.project_dir,
            args.artifact_store.clone(),
            bindings,
        )
        .await
        .with_context(|| {
            format!(
                "failed to prepare service '{}'; no application release was activated",
                service.id
            )
        })?;
        services.insert(service.id.to_string(), prepared);
        println!("  ✓ {} prepared", service.id);
    }
    let prepare_ms = prepare_started.elapsed().as_millis();
    let routes = plan
        .routes
        .iter()
        .map(|(path, service)| (path.clone(), service.to_string()))
        .collect();
    let resource_bindings = plan
        .services
        .iter()
        .map(|service| {
            (
                service.id.to_string(),
                service
                    .spec
                    .resources
                    .iter()
                    .map(|(variable, resource)| (variable.clone(), resource.clone()))
                    .collect(),
            )
        })
        .collect();
    let activation_started = Instant::now();
    let activation = deploy::activate_application_release(
        &args.control_endpoint,
        pit_deployment::ApplicationReleaseRequest {
            application_id: plan.application_name.clone(),
            manifest_digest: plan.manifest.digest.clone(),
            services,
            routes,
            resource_bindings,
            metadata: BTreeMap::new(),
        },
    )
    .await
    .context("failed to atomically activate application release")?;
    println!(
        "  ✓ application release {} active",
        activation.release.release_id
    );
    println!("\n✓ Pit application ready");
    if args.timings {
        println!("\nPit up timings");
        println!("  Resolve:        {resolve_ms} ms");
        println!("  Doctor:         {doctor_ms} ms");
        println!("  Build:          {build_ms} ms");
        println!("  Local prepare:  {prepare_ms} ms");
        println!(
            "  Activation HTTP: {} ms",
            activation_started.elapsed().as_millis()
        );
        println!("  Validation:     {} ms", activation.timings.validation_ms);
        println!(
            "  Artifact verify:{} ms",
            activation.timings.artifact_verification_ms
        );
        println!(
            "  PitBox prepare: {} ms",
            activation.timings.artifact_preparation_ms
        );
        println!(
            "  Release persist:{} ms",
            activation.timings.release_persistence_ms
        );
        println!(
            "  Snapshot publish: {} ms",
            activation.timings.snapshot_publication_ms
        );
        println!(
            "  Prepared hits:   {}",
            activation.timings.prepared_cache_hits
        );
        println!("  Cold compiles:   {}", activation.timings.cold_compiles);
        println!("  Warm restores:   {}", activation.timings.warm_restores);
        println!(
            "  Total:           {} ms",
            lifecycle_started.elapsed().as_millis()
        );
    }
    Ok(())
}

const DEFAULT_CONTROL_ENDPOINT: &str = "http://127.0.0.1:7081";

/// A distribution bundle contains the CLI and its local PitLane sibling. When
/// that topology is present, make `pit up` a one-command local experience.
/// Source-checkout users retain the explicit endpoint workflow used by the
/// existing tests and distributed deployments.
async fn ensure_bundled_pit_lane(
    control_endpoint: &str,
    artifact_store: Option<&std::path::Path>,
) -> Result<()> {
    if control_endpoint != DEFAULT_CONTROL_ENDPOINT
        || reqwest::get(format!("{control_endpoint}/v1/runtime"))
            .await
            .is_ok()
    {
        return Ok(());
    }
    let executable = std::env::current_exe()?;
    let bundled_candidate = executable.parent().map(|dir| dir.join("pit-lane"));
    let source_checkout_candidate =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pit-lane/target/release/pit-lane");
    let Some(pit_lane) = [bundled_candidate, Some(source_checkout_candidate)]
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
    else {
        return Ok(());
    };

    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".local/state"))
        })
        .ok_or_else(|| anyhow!("cannot locate a user state directory for local PitLane"))?
        .join("pitfast");
    std::fs::create_dir_all(&state_dir)?;
    let pid_path = state_dir.join("pit-lane.pid");
    if let Some(pid) = std::fs::read_to_string(&pid_path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
    {
        if std::path::Path::new(&format!("/proc/{pid}")).exists() {
            bail!(
                "local PitLane is unavailable at {control_endpoint}, but process {pid} is still recorded; inspect {}",
                state_dir.display()
            );
        }
        let _ = std::fs::remove_file(&pid_path);
    }
    let log_path = state_dir.join("pit-lane.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open local PitLane log {}", log_path.display()))?;
    // Detach the infrastructure daemon from the short-lived `pit up` process
    // group. This keeps the local runtime alive after the command returns.
    let mut command = std::process::Command::new("setsid");
    command
        .arg(&pit_lane)
        .args([
            "--listen",
            "127.0.0.1:7080",
            "--control-listen",
            "127.0.0.1:7081",
        ])
        .stdout(std::process::Stdio::from(log.try_clone()?))
        .stderr(std::process::Stdio::from(log));
    if let Some(artifact_store) = artifact_store {
        command.args(["--artifact-store", &artifact_store.to_string_lossy()]);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start bundled PitLane {}", pit_lane.display()))?;
    std::fs::write(&pid_path, child.id().to_string())?;
    for _ in 0..100 {
        if reqwest::get(format!("{control_endpoint}/v1/runtime"))
            .await
            .is_ok()
        {
            println!("\nLocal PitLane started");
            return Ok(());
        }
        if child.try_wait()?.is_some() {
            bail!(
                "bundled PitLane exited before becoming ready; inspect {}",
                log_path.display()
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    bail!(
        "bundled PitLane did not become ready at {control_endpoint}; inspect {}",
        log_path.display()
    )
}
