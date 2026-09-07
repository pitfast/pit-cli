use anyhow::Result;
use pit_builder_experimental::ExperimentalBuilder;
use pit_builder_go::GoBuilder;
use pit_builder_js::JsBuilder;
use pit_builder_native::NativeBuilder;
use pit_builder_python::PythonBuilder;
use pit_builder_rust::RustBuilder;
use pit_crew::{Language, LanguageBuilder};

use crate::DoctorCommand;

pub async fn run(command: Option<DoctorCommand>) -> Result<()> {
    if command.is_none() {
        return project_doctor();
    }
    println!("LANGUAGE      TOOLCHAIN                 STATUS");
    for language in Language::ALL {
        let (toolchain, status) = probe(language).await;
        println!("{:<13} {:<42} {}", language.as_str(), toolchain, status);
    }
    Ok(())
}

fn project_doctor() -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let config = crate::project::load(&project_dir)?;
    let crew = crate::commands::build::default_crew();
    let languages = crew.detect_languages(&project_dir)?;
    let candidates = crew.inspect(&project_dir)?;
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
    if let Some(candidate) = candidates.first() {
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
        println!("\nCapabilities\n  ✓ interface adapter can target wasi:http/proxy");
    } else if let Some(interface) = config.build.interface {
        println!("\nInterface\n  ✓ {interface} (pit.toml)");
        if let Some(entry) = config.build.entry {
            println!("\nEntrypoint\n  ✓ {entry} (pit.toml)");
        }
        println!("\nCapabilities\n  ✓ manually configured interface; run pit build to validate");
    } else {
        println!("\nInterface\n  ? specify [build].interface or --interface");
    }
    Ok(())
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
