use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use pit_artifact::{ComponentWorld, RuntimeAbi};
use pit_builder_experimental::ExperimentalBuilder;
use pit_builder_go::GoBuilder;
use pit_builder_js::JsBuilder;
use pit_builder_native::NativeBuilder;
use pit_builder_python::PythonBuilder;
use pit_builder_rust::RustBuilder;
use pit_crew::{BuildOutcome, BuildProfile, BuildRequest, Language, LanguageBuilder, PitCrew};

use crate::manifest::{ApplicationPlan, ResolvedService};
use crate::project;

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Optional service identity from a resolved Pit Manifest.
    pub service: Option<String>,
    /// Select a Pit Manifest. Without this flag, *.pit discovery is used when present.
    #[arg(short = 'f', long = "file")]
    pub file: Option<PathBuf>,
    /// Select the source language, overriding pit.toml and detection.
    #[arg(long)]
    pub language: Option<Language>,
    /// Select the application interface, for example asgi or wasi-http.
    #[arg(long)]
    pub interface: Option<pit_crew::ApplicationInterface>,
    /// Application entrypoint, for example main:app.
    #[arg(long)]
    pub entry: Option<String>,
    /// Select a built-in adapter id or a project-local adapter directory.
    #[arg(long)]
    pub adapter: Option<String>,
    /// Validate and package an already-built compatible WASM Component.
    #[arg(long)]
    pub artifact: Option<PathBuf>,
    /// Select a binary target when the project has more than one.
    #[arg(long)]
    pub bin: Option<String>,
    /// Use Cargo's debug profile instead of the default release profile.
    #[arg(long)]
    pub debug: bool,
    /// Rebuild even when the cached build inputs and artifact are valid.
    #[arg(long)]
    pub force: bool,
    /// Select the WASI ABI; defaults to pit.toml or wasi-preview2.
    #[arg(long, value_parser = clap::value_parser!(RuntimeAbi))]
    pub abi: Option<RuntimeAbi>,
    /// Select the standard P2 component world.
    #[arg(long, requires = "abi")]
    pub world: Option<ComponentWorld>,
}

pub fn default_crew() -> PitCrew {
    PitCrew::with_default_adapters(vec![
        Arc::new(RustBuilder::new()) as Arc<dyn LanguageBuilder>,
        Arc::new(GoBuilder::new()),
        Arc::new(NativeBuilder::c()),
        Arc::new(NativeBuilder::cpp()),
        Arc::new(JsBuilder::javascript()),
        Arc::new(JsBuilder::typescript()),
        Arc::new(PythonBuilder::new()),
        Arc::new(ExperimentalBuilder::csharp()),
        Arc::new(ExperimentalBuilder::java()),
    ])
}

pub async fn run(args: BuildArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let profile = if args.debug {
        BuildProfile::Debug
    } else {
        BuildProfile::Release
    };
    if let Some(resolved) = crate::manifest::resolve_optional(&project_dir, args.file.as_deref())? {
        let plan = resolved.plan()?;
        let services = selected_services(&plan, args.service.as_deref())?;
        println!("Pit Manifest: {}", plan.manifest.path.display());
        for service in services {
            let outcome = build_service(&plan, service, profile, args.force).await?;
            print_build(service, &outcome);
        }
        return Ok(());
    }
    if args.service.is_some() {
        anyhow::bail!("service selection requires a Pit Manifest; pass --file or create *.pit");
    }
    let config = project::load(&project_dir)?;
    let defaults = project::execution_defaults(&config)?;
    let request = BuildRequest {
        project_dir: project_dir.clone(),
        bin: args.bin.or(config.build.bin),
        wit_path: None,
        profile,
        abi: args
            .abi
            .or(config.build.abi)
            .unwrap_or_else(RuntimeAbi::wasi_preview2),
        world: args.world.or(config.build.world),
        execution_defaults: defaults,
        force: args.force,
        language: args.language.or(config.build.language),
        application_interface: args.interface.or(config.build.interface),
        entrypoint: args.entry.or(config.build.entry),
        adapter: args.adapter.or(config.build.adapter),
        adapter_workspace: None,
        raw_artifact: args.artifact,
    };
    let crew = default_crew();
    let outcome = crew.build_with_status(request).await?;
    let artifact = outcome.artifact;
    let display_path = artifact
        .artifact_path
        .strip_prefix(&project_dir)
        .map(PathBuf::from)
        .unwrap_or(artifact.artifact_path.clone());

    println!("PitCrew");
    println!();
    println!("Language: {}", artifact.manifest.build.language);
    println!(
        "Toolchain: {} {}",
        artifact.toolchain.name, artifact.toolchain.version
    );
    println!("Target: {}", artifact.manifest.build.target);
    println!("Profile: {}", artifact.manifest.build.profile.as_str());
    if let Some(interface) = &artifact.manifest.build.application_interface {
        println!("Interface: {interface}");
    }
    if let Some(adapter) = &artifact.manifest.build.adapter {
        println!("Adapter: {adapter}");
    }
    if let Some(world) = artifact.manifest.runtime.world {
        println!("World: {}", world.as_str());
    }
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

pub fn selected_services<'a>(
    plan: &'a ApplicationPlan,
    selected: Option<&str>,
) -> Result<Vec<&'a ResolvedService>> {
    if let Some(selected) = selected {
        let service: pit_lane_core::ServiceId = selected.parse()?;
        let service = plan
            .services
            .iter()
            .find(|candidate| candidate.id == service)
            .ok_or_else(|| anyhow::anyhow!("manifest has no service '{selected}'"))?;
        Ok(vec![service])
    } else {
        Ok(plan.services.iter().collect())
    }
}

pub async fn build_service(
    plan: &ApplicationPlan,
    service: &ResolvedService,
    profile: BuildProfile,
    force: bool,
) -> Result<BuildOutcome> {
    let request = request_for_service(plan, service, profile, force)?;
    default_crew().build_with_status(request).await
}

pub fn request_for_service(
    plan: &ApplicationPlan,
    service: &ResolvedService,
    profile: BuildProfile,
    force: bool,
) -> Result<BuildRequest> {
    let config = project::load(&service.project_dir)?;
    let defaults = if plan.manifest.manifest.execution.timeout.is_some()
        || plan.manifest.manifest.execution.memory.is_some()
    {
        plan.execution.clone()
    } else {
        project::execution_defaults(&config)?
    };
    Ok(BuildRequest {
        project_dir: service.project_dir.clone(),
        bin: config.build.bin,
        wit_path: None,
        profile,
        abi: service
            .spec
            .abi
            .clone()
            .or(config.build.abi)
            .unwrap_or_else(RuntimeAbi::wasi_preview2),
        world: service.spec.world.or(config.build.world),
        execution_defaults: defaults,
        force,
        language: service.spec.language.or(config.build.language),
        application_interface: service.spec.interface.clone().or(config.build.interface),
        entrypoint: service.spec.entry.clone().or(config.build.entry),
        adapter: service.spec.adapter.clone().or(config.build.adapter),
        adapter_workspace: None,
        raw_artifact: service.artifact_path.clone(),
    })
}

fn print_build(service: &ResolvedService, outcome: &BuildOutcome) {
    let artifact = &outcome.artifact;
    println!(
        "  {}  {} / {}  {}",
        service.id,
        artifact.manifest.build.language,
        artifact
            .manifest
            .build
            .application_interface
            .as_deref()
            .unwrap_or("native"),
        if outcome.reused { "cached" } else { "built" }
    );
}
