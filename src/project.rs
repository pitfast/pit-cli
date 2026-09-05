use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pit_artifact::ExecutionDefaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub project: ProjectSection,
    pub build: BuildSection,
    pub execution: ExecutionSection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSection {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildSection {
    pub bin: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExecutionSection {
    pub timeout: Option<String>,
    pub memory: Option<String>,
}

pub fn config_path(project_dir: &Path) -> PathBuf {
    project_dir.join("pit.toml")
}

pub fn load(project_dir: &Path) -> Result<ProjectConfig> {
    let path = config_path(project_dir);
    if !path.is_file() {
        return Ok(ProjectConfig::default());
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("malformed {}", path.display()))
}

pub fn execution_defaults(config: &ProjectConfig) -> Result<ExecutionDefaults> {
    Ok(ExecutionDefaults {
        timeout_ms: config
            .execution
            .timeout
            .as_deref()
            .map(parse_duration_ms)
            .transpose()?,
        memory_bytes: config
            .execution
            .memory
            .as_deref()
            .map(parse_memory_bytes)
            .transpose()?,
    })
}

pub fn init(project_dir: &Path) -> Result<bool> {
    if !project_dir.join("Cargo.toml").is_file() {
        bail!("pit init requires an existing Rust project with Cargo.toml");
    }
    let config = config_path(project_dir);
    let created_config = if config.exists() {
        false
    } else {
        let name = project_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pitfast-project");
        fs::write(
            &config,
            format!(
                "[project]\nname = \"{name}\"\n\n[build]\n# bin = \"binary-name\"\n\n[execution]\n# timeout = \"2s\"\n# memory = \"64MiB\"\n"
            ),
        )?;
        true
    };
    ensure_gitignore(project_dir)?;
    Ok(created_config)
}

fn ensure_gitignore(project_dir: &Path) -> Result<()> {
    let path = project_dir.join(".gitignore");
    let mut contents = if path.is_file() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };
    if !contents
        .lines()
        .any(|line| matches!(line.trim(), ".pit" | ".pit/"))
    {
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str(".pit/\n");
        fs::write(path, contents)?;
    }
    Ok(())
}

fn parse_duration_ms(value: &str) -> Result<u64> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1_u64)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000)
    } else {
        bail!("duration must use ms, s, or m (for example 2s)");
    };
    number
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .ok_or_else(|| anyhow::anyhow!("duration is invalid or too large"))
}

fn parse_memory_bytes(value: &str) -> Result<u64> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("KiB") {
        (number, 1024_u64)
    } else if let Some(number) = value.strip_suffix("MiB") {
        (number, 1024_u64.pow(2))
    } else if let Some(number) = value.strip_suffix("GiB") {
        (number, 1024_u64.pow(3))
    } else {
        bail!("memory must use KiB, MiB, or GiB (for example 64MiB)");
    };
    number
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .ok_or_else(|| anyhow::anyhow!("memory is invalid or too large"))
}

#[cfg(test)]
mod tests {
    use super::{ProjectConfig, execution_defaults, init, load};
    use std::fs;

    #[test]
    fn init_creates_config_and_ignore_entry() {
        let project = std::env::temp_dir().join(format!("pit-project-{}", std::process::id()));
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        assert!(init(&project).unwrap());
        assert!(!init(&project).unwrap());
        let config = load(&project).unwrap();
        assert_eq!(
            config.project.name.as_deref(),
            Some(project.file_name().unwrap().to_str().unwrap())
        );
        assert!(project.join(".gitignore").exists());
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn config_execution_defaults_parse() {
        let config = ProjectConfig {
            execution: super::ExecutionSection {
                timeout: Some("2s".into()),
                memory: Some("64MiB".into()),
            },
            ..ProjectConfig::default()
        };
        let defaults = execution_defaults(&config).unwrap();
        assert_eq!(defaults.timeout_ms, Some(2_000));
        assert_eq!(defaults.memory_bytes, Some(64 * 1024 * 1024));
    }
}
