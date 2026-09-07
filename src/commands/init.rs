use anyhow::{Result, bail};
use clap::Args;
use pit_crew::{ApplicationInterface, Language};

use crate::project;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Source language. Auto-detected when omitted.
    #[arg(long)]
    pub language: Option<Language>,
    /// Application interface, for example asgi or wasi-http.
    #[arg(long)]
    pub interface: Option<ApplicationInterface>,
    /// Application entrypoint, for example main:app.
    #[arg(long)]
    pub entry: Option<String>,
    /// Built-in adapter id or project-local adapter directory.
    #[arg(long)]
    pub adapter: Option<String>,
    /// Explain the proposed configuration without modifying the project.
    #[arg(long, alias = "explain")]
    pub dry_run: bool,
}

pub fn run(args: InitArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let crew = crate::commands::build::default_crew();
    let candidates = crew.inspect(&project_dir)?;
    let languages = crew.detect_languages(&project_dir)?;
    let language = match args.language {
        Some(language) => language,
        None => match languages.as_slice() {
            [language] => *language,
            [] => bail!("unable to detect a language; use --language"),
            _ => bail!("multiple languages detected; use --language"),
        },
    };
    let candidate = candidates
        .iter()
        .filter(|candidate| candidate.language == language)
        .max_by_key(|candidate| candidate.confidence);
    let interface = args
        .interface
        .or_else(|| candidate.and_then(|candidate| candidate.application_interface.clone()));
    let entry = args
        .entry
        .or_else(|| candidate.and_then(|candidate| candidate.entrypoint.clone()));
    println!("Inspecting project...");
    println!("\nLanguage\n  {language}");
    if let Some(candidate) = candidate {
        if let Some(hint) = &candidate.framework_hint {
            println!("\nFramework hint\n  {hint}");
        }
        println!("\nEvidence");
        for evidence in &candidate.evidence {
            println!("  ✓ {}: {}", evidence.source, evidence.detail);
        }
    }
    if let Some(interface) = &interface {
        println!("\nApplication interface\n  {interface}");
    }
    if let Some(entry) = &entry {
        println!("\nEntrypoint\n  {entry}");
    }
    if let (Some(interface), Some(entry)) = (&interface, &entry) {
        let request = pit_crew::BuildRequest {
            project_dir: project_dir.clone(),
            abi: pit_artifact::RuntimeAbi::wasi_preview2(),
            world: None,
            language: Some(language),
            application_interface: Some(interface.clone()),
            entrypoint: Some(entry.clone()),
            adapter: args.adapter.clone(),
            ..pit_crew::BuildRequest::new(project_dir.clone())
        };
        if let Some(report) = crew.compatibility(&project_dir, &request)? {
            println!("\nCompatibility\n  status: {:?}", report.status);
            for finding in &report.findings {
                let marker = if finding.blocks_build { "✗" } else { "⚠" };
                println!("  {marker} [{}] {}", finding.category, finding.reason);
                println!("    action: {}", finding.recommendation);
            }
            if report.findings.is_empty() {
                for message in &report.messages {
                    println!("  ✓ {message}");
                }
            }
        }
    }
    if args.dry_run {
        println!("\n(dry run: no files changed)");
        return Ok(());
    }
    let created = project::init_with_options(
        &project_dir,
        language,
        interface.as_ref(),
        entry.as_deref(),
        args.adapter.as_deref(),
    )?;
    println!("PitFast project initialized");
    if created {
        println!("✓ Created app.pit");
    } else {
        println!("✓ Kept existing Pit Manifest");
    }
    println!("✓ .pit/ is ignored as generated output");
    println!();
    println!("Next steps:");
    println!("  pit doctor");
    println!("  pit up");
    Ok(())
}
