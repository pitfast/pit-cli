use anyhow::Result;
use pit_artifact::RuntimeAbi;
use pit_builder_experimental::ExperimentalBuilder;
use pit_builder_go::GoBuilder;
use pit_builder_js::JsBuilder;
use pit_builder_native::NativeBuilder;
use pit_builder_python::PythonBuilder;
use pit_builder_rust::RustBuilder;
use pit_crew::{ApplicationInterface, Language, LanguageBuilder};
use std::path::{Path, PathBuf};

use crate::DoctorCommand;

pub async fn run(
    command: Option<DoctorCommand>,
    json: bool,
    verbose: bool,
    file: Option<PathBuf>,
) -> Result<()> {
    if command.is_none() {
        return project_doctor(json, verbose, file.as_deref());
    }
    println!("LANGUAGE      TOOLCHAIN                 STATUS");
    for language in Language::ALL {
        let (toolchain, status) = probe(language).await;
        println!("{:<13} {:<42} {}", language.as_str(), toolchain, status);
    }
    Ok(())
}

fn project_doctor(json: bool, verbose: bool, file: Option<&Path>) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    if let Some(resolved) = crate::manifest::resolve_optional(&project_dir, file)? {
        return manifest_doctor(resolved, json, verbose);
    }
    let config = crate::project::load(&project_dir)?;
    let crew = crate::commands::build::default_crew();
    let languages = crew.detect_languages(&project_dir)?;
    let candidates = crew.inspect(&project_dir)?;
    let language = if languages.len() == 1 {
        Some(languages[0])
    } else {
        config.build.language
    };
    let candidate = language.and_then(|language| {
        candidates
            .iter()
            .filter(|candidate| candidate.language == language)
            .max_by_key(|candidate| candidate.confidence)
    });
    let interface = config
        .build
        .interface
        .clone()
        .or_else(|| candidate.and_then(|candidate| candidate.application_interface.clone()));
    let entrypoint = config
        .build
        .entry
        .clone()
        .or_else(|| candidate.and_then(|candidate| candidate.entrypoint.clone()));
    let request = pit_crew::BuildRequest {
        project_dir: project_dir.clone(),
        abi: config.build.abi.unwrap_or_else(RuntimeAbi::wasi_preview2),
        world: config.build.world,
        language,
        application_interface: interface.clone(),
        entrypoint: entrypoint.clone(),
        adapter: config.build.adapter.clone(),
        ..pit_crew::BuildRequest::new(project_dir.clone())
    };
    let compatibility = if language.is_some() && interface.is_some() {
        crew.compatibility(&project_dir, &request)?
    } else {
        None
    };
    let adapter_id = config.build.adapter.clone().or_else(|| {
        language.and_then(|language| {
            interface.as_ref().and_then(|interface| {
                crew.application_adapters()
                    .iter()
                    .find(|adapter| {
                        adapter.language() == language && adapter.interface() == interface
                    })
                    .map(|adapter| adapter.id().to_string())
            })
        })
    });
    let capabilities = capability_report(language, interface.as_ref(), adapter_id.as_deref());

    if json {
        let report = serde_json::json!({
            "schema_version": 1,
            "project": project_dir,
            "language": language.map(|value| value.to_string()),
            "interface": interface.map(|value| value.to_string()),
            "entrypoint": entrypoint,
            "adapter": adapter_id,
            "capabilities": capabilities,
            "framework_hint": candidate.and_then(|value| value.framework_hint.clone()),
            "evidence": candidate.map(|value| value.evidence.clone()).unwrap_or_default(),
            "compatibility": compatibility,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
        if compatibility
            .as_ref()
            .is_some_and(pit_crew::CompatibilityReport::blocks_build)
        {
            anyhow::bail!("project compatibility has confirmed blockers; see pit doctor --verbose");
        }
        return Ok(());
    }

    println!("PitFast Compatibility");
    if languages.len() == 1 {
        println!("\nLanguage\n  ✓ {}", languages[0]);
    } else if let Some(language) = config.build.language {
        println!("\nLanguage\n  ✓ {} (pit.toml)", language);
    } else if languages.is_empty() {
        println!("\nLanguage\n  ? no language detected");
    } else {
        println!("\nLanguage\n  ! multiple candidates: {:?}", languages);
    }
    if let Some(candidate) = candidate {
        if let Some(interface) = &candidate.application_interface {
            println!("\nInterface\n  ✓ {interface}");
        }
        if let Some(entry) = &candidate.entrypoint {
            println!("\nEntrypoint\n  ✓ {entry}");
        }
        if let Some(framework) = &candidate.framework_hint {
            println!("\nFramework hint\n  {framework} (convenience metadata only)");
        }
        println!("\nEvidence");
        for evidence in &candidate.evidence {
            println!("  ✓ {}: {}", evidence.source, evidence.detail);
        }
        println!(
            "\nAdapter\n  ✓ {}",
            adapter_id
                .as_deref()
                .unwrap_or("built-in interface adapter")
        );
        println!("\nCapabilities");
        print_capabilities(language, interface.as_ref(), adapter_id.as_deref());
    } else if let Some(interface) = config.build.interface {
        println!("\nInterface\n  ✓ {interface} (pit.toml)");
        if let Some(entry) = config.build.entry {
            println!("\nEntrypoint\n  ✓ {entry} (pit.toml)");
        }
        println!("\nCapabilities\n  ✓ manually configured interface; run pit build to validate");
    } else {
        println!("\nInterface\n  ? specify [build].interface or --interface");
    }
    if let Some(report) = compatibility {
        println!("\nDependencies\n  status: {:?}", report.status);
        for message in &report.messages {
            println!("  • {message}");
        }
        for finding in &report.findings {
            let marker = if finding.blocks_build { "✗" } else { "⚠" };
            println!("  {marker} [{}] {}", finding.category, finding.reason);
            if let Some(package) = &finding.package {
                println!("    package: {package}");
            }
            if verbose && !finding.dependency_path.is_empty() {
                println!("    path: {}", finding.dependency_path.join(" -> "));
            }
            if verbose {
                for evidence in &finding.evidence {
                    println!("    evidence: {} — {}", evidence.source, evidence.detail);
                }
            }
            println!("    action: {}", finding.recommendation);
        }
        if report.blocks_build() {
            println!("\nResult\n  Build is currently blocked by confirmed compatibility findings.");
            anyhow::bail!("project compatibility has confirmed blockers; see the findings above");
        } else {
            println!("\nResult\n  No confirmed compatibility blocker found.");
        }
    }
    Ok(())
}

fn manifest_doctor(
    resolved: crate::manifest::ResolvedManifest,
    json: bool,
    verbose: bool,
) -> Result<()> {
    let plan = resolved.plan()?;
    let crew = crate::commands::build::default_crew();
    let mut services = Vec::new();
    let mut blocked = false;
    for service in &plan.services {
        let request = crate::commands::build::request_for_service(
            &plan,
            service,
            pit_artifact::BuildProfile::Release,
            false,
        )?;
        let candidates = crew.inspect(&service.project_dir)?;
        let report = crew.compatibility(&service.project_dir, &request)?;
        if report.as_ref().is_some_and(|value| value.blocks_build()) {
            blocked = true;
        }
        services.push(serde_json::json!({
            "id": service.id.to_string(),
            "build": service.project_dir,
            "interface": request.application_interface.as_ref().map(ToString::to_string),
            "entrypoint": request.entrypoint,
            "adapter": request.adapter,
            "evidence": candidates.iter().flat_map(|candidate| candidate.evidence.clone()).collect::<Vec<_>>(),
            "compatibility": report,
        }));
    }
    if json {
        let output = serde_json::json!({
            "schema_version": 2,
            "manifest": plan.manifest.path,
            "manifest_digest": plan.manifest.digest,
            "application": plan.application_name,
            "services": services,
            "resources": plan.resources,
            "routes": plan.routes.iter().map(|(path, service)| (path, service.to_string())).collect::<std::collections::BTreeMap<_, _>>(),
            "status": if blocked { "blocked" } else { "ready" },
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "Pit Application\n  {}\n  manifest: {}",
            plan.application_name,
            plan.manifest.path.display()
        );
        for service in &plan.services {
            let request = crate::commands::build::request_for_service(
                &plan,
                service,
                pit_artifact::BuildProfile::Release,
                false,
            )?;
            let report = crew.compatibility(&service.project_dir, &request)?;
            println!("\n{}", service.id);
            println!("  build: {}", service.project_dir.display());
            println!(
                "  interface: {}",
                request
                    .application_interface
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "auto-detect".into())
            );
            if let Some(adapter) = request.adapter {
                println!("  adapter: {adapter}");
            }
            if let Some(report) = report {
                println!("  compatibility: {:?}", report.status);
                for finding in report.findings {
                    let marker = if finding.blocks_build { "✗" } else { "⚠" };
                    println!("  {marker} [{}] {}", finding.category, finding.reason);
                    if verbose && !finding.dependency_path.is_empty() {
                        println!("    path: {}", finding.dependency_path.join(" -> "));
                    }
                    if verbose {
                        for evidence in finding.evidence {
                            println!("    evidence: {} — {}", evidence.source, evidence.detail);
                        }
                    }
                    println!("    action: {}", finding.recommendation);
                }
            }
        }
        println!("\nOverall: {}", if blocked { "BLOCKED" } else { "READY" });
    }
    if blocked {
        anyhow::bail!("Pit Manifest compatibility has confirmed service blockers");
    }
    Ok(())
}

fn capability_report(
    language: Option<Language>,
    interface: Option<&ApplicationInterface>,
    adapter: Option<&str>,
) -> serde_json::Value {
    let interface_name = interface.map(ToString::to_string);
    let (status, runtime_contract, capabilities, notes) = match interface_name.as_deref() {
        Some("asgi") => (
            "supported",
            "wasi:http/proxy",
            vec!["HTTP request/response", "headers", "body", "query"],
            vec!["Python dependency compatibility remains application-specific"],
        ),
        Some("fetch") => (
            "supported",
            "wasi:http/proxy",
            vec!["Fetch Request/Response", "headers", "body", "promises"],
            vec!["Node-only APIs are not part of the guest contract"],
        ),
        Some("net-http") => (
            "supported",
            "wasi:http/proxy",
            vec!["net/http Handler", "headers", "body", "query"],
            vec!["the application must export an importable Handler, not bind a listener"],
        ),
        Some("wasi-http") => (
            "supported",
            "wasi:http/proxy",
            vec!["WASI HTTP Component"],
            vec!["raw Component validation remains the final compatibility authority"],
        ),
        Some("wasi-cli") => (
            "supported",
            "wasi:cli/command",
            vec!["args", "stdin", "stdout", "stderr", "exit code"],
            vec!["host process and unrestricted filesystem access are unavailable"],
        ),
        Some(_) => (
            if adapter.is_some() {
                "configured"
            } else {
                "unknown"
            },
            "adapter-defined",
            Vec::new(),
            vec!["validate the selected local adapter before building"],
        ),
        None => (
            "unknown",
            "not selected",
            Vec::new(),
            vec!["select an application interface"],
        ),
    };
    serde_json::json!({
        "language": language.map(|value| value.to_string()),
        "interface": interface_name,
        "adapter": adapter,
        "status": status,
        "runtime_contract": runtime_contract,
        "capabilities": capabilities,
        "notes": notes,
    })
}

fn print_capabilities(
    language: Option<Language>,
    interface: Option<&ApplicationInterface>,
    adapter: Option<&str>,
) {
    let report = capability_report(language, interface, adapter);
    if let Some(status) = report.get("status").and_then(serde_json::Value::as_str) {
        println!("  {status}: {}", report["runtime_contract"]);
    }
    if let Some(values) = report
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
    {
        for value in values.iter().filter_map(serde_json::Value::as_str) {
            println!("  ✓ {value}");
        }
    }
    if let Some(values) = report.get("notes").and_then(serde_json::Value::as_array) {
        for value in values.iter().filter_map(serde_json::Value::as_str) {
            println!("  ⚠ {value}");
        }
    }
}

async fn probe(language: Language) -> (String, String) {
    match language {
        Language::Rust => match RustBuilder::new().probe_toolchain().await {
            Ok(info) => (format!("{} {}", info.name, info.version), "ready".into()),
            Err(error) => ("rustc/rustup".into(), format!("blocked: {error}")),
        },
        Language::Go => match GoBuilder::new().probe_toolchain().await {
            Ok(info) => (format!("{} {}", info.name, info.version), "ready".into()),
            Err(error) => ("Go + componentize-go".into(), format!("blocked: {error}")),
        },
        Language::C => probe_builder(NativeBuilder::c()).await,
        Language::Cpp => probe_builder(NativeBuilder::cpp()).await,
        Language::JavaScript => probe_builder(JsBuilder::javascript()).await,
        Language::TypeScript => probe_builder(JsBuilder::typescript()).await,
        Language::Python => probe_builder(PythonBuilder::new()).await,
        Language::CSharp => probe_builder(ExperimentalBuilder::csharp()).await,
        Language::Java => probe_builder(ExperimentalBuilder::java()).await,
    }
}

async fn probe_builder(builder: impl LanguageBuilder) -> (String, String) {
    match builder.probe_toolchain().await {
        Ok(info) => (format!("{} {}", info.name, info.version), "ready".into()),
        Err(error) => (
            builder.language().as_str().into(),
            format!("blocked: {error}"),
        ),
    }
}
