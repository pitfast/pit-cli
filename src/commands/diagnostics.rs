use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use clap::Args;
use serde_json::{Map, Value, json};

#[derive(Debug, Args)]
pub struct DiagnosticsArgs {
    /// Write the diagnostic document to this path instead of stdout.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
    /// Select a Pit Manifest explicitly.
    #[arg(short = 'f', long = "file")]
    pub file: Option<PathBuf>,
}

/// Produce support metadata without copying environment values, manifest
/// secrets, credentials, or raw logs. This remains useful when a project is
/// malformed because diagnostics should help explain the failure.
pub fn run(args: DiagnosticsArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (manifest, manifest_error) =
        match crate::manifest::resolve_optional(&cwd, args.file.as_deref()) {
            Ok(Some(resolved)) => match resolved.plan() {
                Ok(plan) => (Some(plan), None),
                Err(error) => (None, Some(error.to_string())),
            },
            Ok(None) => (None, None),
            Err(error) => (None, Some(error.to_string())),
        };

    let toolchains = [
        ("node", &["--version"][..]),
        ("npm", &["--version"][..]),
        ("pnpm", &["--version"][..]),
        ("cargo", &["--version"][..]),
        ("go", &["version"][..]),
        ("python3", &["--version"][..]),
    ]
    .into_iter()
    .map(|(name, probe)| {
        let version = command_version(name, probe);
        (
            name.to_owned(),
            json!({
                "available": version.is_some(),
                "version": version,
            }),
        )
    })
    .collect::<Map<_, _>>();

    let mut document = json!({
        "schema_version": 1,
        "pitfast": {
            "version": env!("CARGO_PKG_VERSION"),
            "software_version_source": "pit-cli package",
        },
        "host": {
            "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
            "current_directory": cwd,
        },
        "toolchains": toolchains,
        "runtime": {
            "local_control_endpoint": "http://127.0.0.1:7081",
            "raw_logs_included": false,
        },
        "manifest": {
            "selected": manifest.is_some(),
            "path": Value::Null,
            "schema": Value::Null,
            "application": Value::Null,
            "services": [],
            "routes": [],
            "resources": [],
        },
        "errors": [],
        "sharing": {
            "warning": "Review this file before sharing it publicly.",
            "secret_values_included": false,
            "environment_values_included": false,
            "authorization_headers_included": false,
        },
    });

    if let Some(plan) = manifest {
        let manifest_json = document
            .get_mut("manifest")
            .and_then(Value::as_object_mut)
            .expect("diagnostic manifest object");
        manifest_json.insert("path".into(), json!(plan.manifest.path));
        manifest_json.insert("schema".into(), json!(plan.manifest.manifest.schema));
        manifest_json.insert("application".into(), json!(plan.application_name));
        manifest_json.insert(
            "services".into(),
            json!(
                plan.services
                    .iter()
                    .map(|service| {
                        json!({
                            "id": service.id.to_string(),
                            "project": service.project_dir,
                            "language": service.spec.language.map(|value| value.to_string()),
                            "interface": service.spec.interface.as_ref().map(ToString::to_string),
                            "adapter": service.spec.adapter,
                            "has_artifact_override": service.artifact_path.is_some(),
                        })
                    })
                    .collect::<Vec<_>>()
            ),
        );
        manifest_json.insert(
            "routes".into(),
            json!(
                plan.routes
                    .iter()
                    .map(|(path, service)| json!({"path": path, "service": service.to_string()}))
                    .collect::<Vec<_>>()
            ),
        );
        manifest_json.insert(
            "resources".into(),
            json!(
                plan.resources
                    .keys()
                    .map(|id| json!({"id": id, "configured": true}))
                    .collect::<Vec<_>>()
            ),
        );
    }
    if let Some(error) = manifest_error {
        document["errors"] = json!([{"code": "manifest", "message": error}]);
    }

    let encoded = serde_json::to_string_pretty(&document)? + "\n";
    if let Some(path) = args.output {
        std::fs::write(&path, encoded)?;
        println!("Wrote redacted PitFast diagnostics to {}", path.display());
        println!("Review this file before sharing it publicly.");
    } else {
        print!("{encoded}");
    }
    Ok(())
}

fn command_version(command: &str, args: &[&str]) -> Option<String> {
    Command::new(command)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(if output.stdout.is_empty() {
                &output.stderr
            } else {
                &output.stdout
            })
            .trim()
            .to_owned()
        })
}

#[cfg(test)]
mod tests {
    use super::run;
    use std::fs;

    #[test]
    fn diagnostic_output_does_not_include_secret_values() {
        let root = std::env::temp_dir().join(format!("pit-diagnostics-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("app.pit"),
            "schema = 1\nname = 'safe'\n[service]\nbuild = '.'\n[resources.db]\ntype = 'postgres'\nfrom = 'DATABASE_URL'\n",
        )
        .unwrap();
        let output = root.join("diagnostics.json");
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&root).unwrap();
        run(crate::commands::diagnostics::DiagnosticsArgs {
            output: Some(output.clone()),
            file: None,
        })
        .unwrap();
        std::env::set_current_dir(previous).unwrap();
        let content = fs::read_to_string(output).unwrap();
        assert!(!content.contains("postgres://"));
        assert!(!content.contains("Authorization"));
        assert!(content.contains("secret_values_included"));
        let _ = fs::remove_dir_all(root);
    }
}
