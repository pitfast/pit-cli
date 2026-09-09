use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Args;

use super::up::UpArgs;

const PIT_WEB_APP: &str = r#"schema = 1
name = "pit-web"

[service]
build = "."
language = "javascript"
interface = "static-web"

[routes]
"/__pit" = "pit-web"
"#;

const PIT_WEB_PACKAGE: &str = r#"{"name":"pit-web","version":"0.1.0-alpha.1","private":true}"#;

#[derive(Debug, Args)]
pub struct WebArgs {
    /// Do not open a browser after Pit Web is activated.
    #[arg(long)]
    pub no_open: bool,
    /// PitLane's loopback control endpoint.
    #[arg(long, default_value = "http://127.0.0.1:7081")]
    pub control_endpoint: String,
    /// PitLane's loopback HTTP endpoint where the static app is routed.
    #[arg(long, default_value = "http://127.0.0.1:7080")]
    pub http_endpoint: String,
}

pub async fn run(args: WebArgs) -> Result<()> {
    let project = materialize_bundled_app()?;
    let activation = super::up::run(UpArgs {
        file: Some(project.join("app.pit")),
        control_endpoint: args.control_endpoint,
        artifact_store: None,
        force: false,
        debug: false,
        timings: false,
    })
    .await
    .context("failed to activate the bundled Pit Web static application");
    let cleanup = std::fs::remove_dir_all(&project);
    if let Err(error) = cleanup
        && activation.is_ok()
    {
        eprintln!("Warning: could not clean temporary Pit Web project: {error}");
    }
    activation?;

    let url = format!("{}/__pit/", args.http_endpoint.trim_end_matches('/'));
    println!("\nPit Web ready\n{url}");
    if !args.no_open
        && let Err(error) = std::process::Command::new("xdg-open").arg(&url).spawn()
    {
        eprintln!("Could not open a browser automatically: {error}");
        eprintln!("Open {url} manually.");
    }
    Ok(())
}

fn materialize_bundled_app() -> Result<PathBuf> {
    let base = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| anyhow!(error))?
        .as_nanos();
    let project = base.join(format!("pit-web-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(project.join("dist/__pit/assets"))?;
    std::fs::write(project.join("app.pit"), PIT_WEB_APP)?;
    std::fs::write(project.join("package.json"), PIT_WEB_PACKAGE)?;
    let index = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/apps/pit-web/dist/index.html"
    ));
    let css = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/apps/pit-web/dist/assets/pit-web.css"
    ));
    let js = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/apps/pit-web/dist/assets/pit-web.js"
    ));
    write_asset(&project.join("dist/index.html"), index)?;
    write_asset(&project.join("dist/__pit/index.html"), index)?;
    write_asset(&project.join("dist/__pit/assets/pit-web.css"), css)?;
    write_asset(&project.join("dist/__pit/assets/pit-web.js"), js)?;
    Ok(project)
}

fn write_asset(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Pit Web asset has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::PIT_WEB_APP;

    #[test]
    fn bundled_app_is_a_read_only_prefixed_static_route() {
        assert!(PIT_WEB_APP.contains("interface = \"static-web\""));
        assert!(PIT_WEB_APP.contains("\"/__pit\" = \"pit-web\""));
        assert!(!PIT_WEB_APP.contains("[resources"));
    }
}
