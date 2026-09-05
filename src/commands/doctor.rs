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
    let _ = command;
    println!("LANGUAGE      TOOLCHAIN                 STATUS");
    for language in Language::ALL {
        let (toolchain, status) = probe(language).await;
        println!("{:<13} {:<42} {}", language.as_str(), toolchain, status);
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
