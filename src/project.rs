use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pit_artifact::{ComponentWorld, ExecutionDefaults, RuntimeAbi};
use pit_crew::{ApplicationInterface, Language};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub project: ProjectSection,
    pub build: BuildSection,
    pub execution: ExecutionSection,
    pub resources: Vec<ResourceSection>,
    pub service: ServiceSection,
    pub network: NetworkSection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSection {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildSection {
    pub language: Option<Language>,
    pub interface: Option<ApplicationInterface>,
    pub entry: Option<String>,
    pub adapter: Option<String>,
    pub bin: Option<String>,
    pub abi: Option<RuntimeAbi>,
    pub world: Option<ComponentWorld>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExecutionSection {
    pub timeout: Option<String>,
    pub memory: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceSection {
    pub id: String,
    pub kind: String,
    pub provider: String,
    pub url_env: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceSection {
    pub name: Option<String>,
    pub artifact: Option<String>,
    pub resources: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSection {
    pub internal_services: Option<bool>,
    pub external_http: Option<bool>,
    pub tcp_allowlist: Vec<String>,
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

pub fn init_with_options(
    project_dir: &Path,
    language: Language,
    interface: Option<&ApplicationInterface>,
    entry: Option<&str>,
    adapter: Option<&str>,
) -> Result<bool> {
    let marker = match language {
        Language::Rust => "Cargo.toml",
        Language::Go => "go.mod",
        Language::Python => "pyproject.toml",
        Language::JavaScript | Language::TypeScript => "package.json",
        Language::C | Language::Cpp => "CMakeLists.txt",
        Language::CSharp => "*.csproj",
        Language::Java => "pom.xml or build.gradle",
    };
    if !project_has_marker(project_dir, language) {
        bail!("pit init requires an existing {language} project ({marker})");
    }
    let manifests = fs::read_dir(project_dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("pit")
        })
        .collect::<Vec<_>>();
    let manifest = project_dir.join("app.pit");
    let created_config = if !manifests.is_empty() {
        false
    } else {
        let name = project_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pitfast-project");
        let mut contents = format!(
            "schema = 1\nname = \"{name}\"\n\n[service]\nbuild = \".\"\nlanguage = \"{language}\"\nabi = \"wasi-preview2\"\n"
        );
        if let Some(interface) = interface {
            contents.push_str(&format!("interface = \"{interface}\"\n"));
        }
        if let Some(entry) = entry {
            contents.push_str(&format!("entry = \"{entry}\"\n"));
        }
        if let Some(adapter) = adapter {
            contents.push_str(&format!("adapter = \"{adapter}\"\n"));
        }
        contents.push_str(
            "# interface = \"asgi\"\n# entry = \"main:app\"\n# adapter = \"python/asgi\"\n\n[execution]\n# timeout = \"2s\"\n# memory = \"64MiB\"\n",
        );
        fs::write(&manifest, contents)?;
        true
    };
    ensure_gitignore(project_dir)?;
    Ok(created_config)
}

fn project_has_marker(project_dir: &Path, language: Language) -> bool {
    match language {
        Language::Rust => project_dir.join("Cargo.toml").is_file(),
        Language::Go => project_dir.join("go.mod").is_file(),
        Language::Python => {
            project_dir.join("pyproject.toml").is_file()
                || project_dir.join("requirements.txt").is_file()
                || project_dir.join("setup.py").is_file()
        }
        Language::JavaScript | Language::TypeScript => project_dir.join("package.json").is_file(),
        Language::C | Language::Cpp => {
            project_dir.join("CMakeLists.txt").is_file() || project_dir.join("pit.toml").is_file()
        }
        Language::CSharp => fs::read_dir(project_dir)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "csproj")
                })
            })
            .unwrap_or(false),
        Language::Java => {
            project_dir.join("pom.xml").is_file()
                || project_dir.join("build.gradle").is_file()
                || project_dir.join("build.gradle.kts").is_file()
        }
    }
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
    use super::{ProjectConfig, execution_defaults, init_with_options};
    use std::fs;

    #[test]
    fn init_creates_config_and_ignore_entry() {
        let project = std::env::temp_dir().join(format!("pit-project-{}", std::process::id()));
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        assert!(init_with_options(&project, pit_crew::Language::Rust, None, None, None).unwrap());
        assert!(!init_with_options(&project, pit_crew::Language::Rust, None, None, None).unwrap());
        let manifest = fs::read_to_string(project.join("app.pit")).unwrap();
        assert!(manifest.contains("schema = 1"));
        assert!(manifest.contains("build = \".\""));
        assert!(manifest.contains("abi = \"wasi-preview2\""));
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
