//! PitFast's user-facing application manifest.
//!
//! This is deliberately separate from `.pit/artifact.json` (the immutable
//! artifact record) and from a project's legacy `pit.toml` build defaults.
//! A `.pit` file describes application intent; this module resolves and
//! validates it before commands perform side effects.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pit_artifact::{ComponentWorld, ExecutionDefaults, RuntimeAbi};
use pit_crew::{ApplicationInterface, Language};
use pit_lane_core::ServiceId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PitManifest {
    pub schema: u32,
    pub name: Option<String>,
    pub service: Option<ServiceSpec>,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceSpec>,
    #[serde(default)]
    pub resources: BTreeMap<String, ResourceSpec>,
    #[serde(default)]
    pub routes: BTreeMap<String, String>,
    #[serde(default)]
    pub execution: ManifestExecution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceSpec {
    pub build: String,
    pub id: Option<String>,
    pub language: Option<Language>,
    pub interface: Option<ApplicationInterface>,
    #[serde(alias = "entrypoint")]
    pub entry: Option<String>,
    pub adapter: Option<String>,
    pub artifact: Option<String>,
    pub abi: Option<RuntimeAbi>,
    pub world: Option<ComponentWorld>,
    #[serde(default)]
    pub resources: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceSpec {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "from")]
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ManifestExecution {
    pub timeout: Option<String>,
    pub memory: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedManifest {
    pub path: PathBuf,
    pub root: PathBuf,
    pub manifest: PitManifest,
    pub digest: String,
}

#[derive(Debug, Clone)]
pub struct ApplicationPlan {
    pub manifest: ResolvedManifest,
    pub application_name: String,
    pub services: Vec<ResolvedService>,
    pub resources: BTreeMap<String, ResourceSpec>,
    pub routes: BTreeMap<String, ServiceId>,
    pub execution: ExecutionDefaults,
}

#[derive(Debug, Clone)]
pub struct ResolvedService {
    pub id: ServiceId,
    pub spec: ServiceSpec,
    pub project_dir: PathBuf,
    pub artifact_path: Option<PathBuf>,
}

/// Resolve an explicitly selected manifest or the current directory's only
/// unambiguous `*.pit` file. This function never searches parent directories.
pub fn resolve(cwd: &Path, explicit: Option<&Path>) -> Result<ResolvedManifest> {
    let path = if let Some(explicit) = explicit {
        let path = if explicit.is_absolute() {
            explicit.to_path_buf()
        } else {
            cwd.join(explicit)
        };
        if !path.is_file() {
            bail!(
                "ManifestNotFound: explicit Pit Manifest does not exist or is not a regular file: {}",
                path.display()
            );
        }
        if path.extension().and_then(|value| value.to_str()) != Some("pit") {
            bail!(
                "invalid Pit Manifest path {}; expected a .pit file",
                path.display()
            );
        }
        path
    } else {
        let mut files = fs::read_dir(cwd)
            .with_context(|| format!("failed to inspect manifest directory {}", cwd.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("pit")
            })
            .collect::<Vec<_>>();
        files.sort();
        match files.as_slice() {
            [] => bail!(
                "ManifestNotFound: no *.pit manifest found in {}; run `pit init`",
                cwd.display()
            ),
            [only] => only.clone(),
            _ => {
                let conventional = cwd.join("app.pit");
                if conventional.is_file() {
                    conventional
                } else {
                    let files = files
                        .iter()
                        .map(|path| format!("  {}", path.display()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    bail!(
                        "AmbiguousManifest: multiple Pit manifests found:\n{}\nChoose one with `pit up --file <manifest>`",
                        files
                    );
                }
            }
        }
    };

    let path = fs::canonicalize(&path)
        .with_context(|| format!("failed to resolve Pit Manifest {}", path.display()))?;
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read Pit Manifest {}", path.display()))?;
    let manifest: PitManifest = toml::from_str(&contents).map_err(|error| {
        anyhow::anyhow!(
            "malformed Pit Manifest {} (expected schema = 1 TOML): {error}",
            path.display()
        )
    })?;
    if manifest.schema != SCHEMA_VERSION {
        bail!(
            "Unsupported Pit Manifest schema: {}\nThis PitFast version supports schema {}.",
            manifest.schema,
            SCHEMA_VERSION
        );
    }
    let digest = format!("sha256:{:x}", Sha256::digest(contents.as_bytes()));
    Ok(ResolvedManifest {
        root: path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Pit Manifest has no parent directory"))?
            .to_path_buf(),
        path,
        manifest,
        digest,
    })
}

/// Resolve a manifest when one is present, retaining legacy `pit.toml`
/// behavior for commands that still support a source project without a Pit
/// Manifest. Explicit `--file` always remains authoritative.
pub fn resolve_optional(cwd: &Path, explicit: Option<&Path>) -> Result<Option<ResolvedManifest>> {
    if explicit.is_some() {
        return resolve(cwd, explicit).map(Some);
    }
    let has_manifest = fs::read_dir(cwd)
        .with_context(|| format!("failed to inspect manifest directory {}", cwd.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .any(|path| {
            path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("pit")
        });
    if has_manifest {
        resolve(cwd, None).map(Some)
    } else {
        Ok(None)
    }
}

impl ResolvedManifest {
    pub fn plan(self) -> Result<ApplicationPlan> {
        let manifest = self;
        let mut services = Vec::new();
        if let Some(spec) = manifest.manifest.service.clone() {
            if !manifest.manifest.services.is_empty() {
                bail!("Pit Manifest cannot define both [service] and [services.*]");
            }
            let id = spec
                .id
                .clone()
                .or_else(|| manifest.manifest.name.clone())
                .unwrap_or_else(|| "app".into());
            services.push(resolve_service(&manifest, id, spec)?);
        } else {
            for (id, spec) in &manifest.manifest.services {
                services.push(resolve_service(&manifest, id.clone(), spec.clone())?);
            }
        }
        if services.is_empty() {
            bail!("Pit Manifest must define [service] or at least one [services.<id>]");
        }

        for (resource_id, resource) in &manifest.manifest.resources {
            if resource_id.parse::<pit_lane_core::ResourceId>().is_err() {
                bail!(
                    "invalid resource identity '{resource_id}' in {}",
                    manifest.path.display()
                );
            }
            if resource.kind.trim().is_empty() || resource.source.trim().is_empty() {
                bail!("resource '{resource_id}' must define non-empty type and from values");
            }
        }
        for service in &services {
            for (variable, resource) in &service.spec.resources {
                if !manifest.manifest.resources.contains_key(resource) {
                    bail!(
                        "service '{}' binds unknown resource '{}' as '{}'",
                        service.id,
                        resource,
                        variable
                    );
                }
            }
        }
        let mut routes = BTreeMap::new();
        for (path, service) in &manifest.manifest.routes {
            if !path.starts_with('/') || path.contains('\n') || path.contains('\r') {
                bail!("invalid route path '{path}'; routes must begin with '/'");
            }
            let service_id: ServiceId = service
                .parse()
                .with_context(|| format!("invalid route target '{service}' for route '{path}'"))?;
            if !services.iter().any(|entry| entry.id == service_id) {
                bail!("route '{path}' targets unknown service '{service_id}'");
            }
            if routes.insert(path.clone(), service_id).is_some() {
                bail!("duplicate route '{path}'");
            }
        }
        let application_name = manifest.manifest.name.clone().unwrap_or_else(|| {
            manifest
                .root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        });
        let execution = manifest_execution_defaults(&manifest.manifest.execution)?;
        Ok(ApplicationPlan {
            resources: manifest.manifest.resources.clone(),
            routes,
            execution,
            manifest,
            application_name,
            services,
        })
    }
}

fn resolve_service(
    manifest: &ResolvedManifest,
    id: String,
    spec: ServiceSpec,
) -> Result<ResolvedService> {
    let id: ServiceId = id.parse().with_context(|| {
        format!(
            "invalid service identity '{id}' in {}",
            manifest.path.display()
        )
    })?;
    let project_dir = manifest.root.join(&spec.build);
    if !project_dir.is_dir() {
        bail!(
            "service '{}' build directory does not exist: {}",
            id,
            project_dir.display()
        );
    }
    let project_dir = fs::canonicalize(&project_dir)?;
    let artifact_path = spec
        .artifact
        .as_ref()
        .map(|path| manifest.root.join(path))
        .map(|path| {
            if !path.is_file() {
                anyhow::bail!(
                    "service '{}' artifact does not exist: {}",
                    id,
                    path.display()
                );
            }
            Ok(fs::canonicalize(path)?)
        })
        .transpose()?;
    Ok(ResolvedService {
        id,
        spec,
        project_dir,
        artifact_path,
    })
}

fn manifest_execution_defaults(execution: &ManifestExecution) -> Result<ExecutionDefaults> {
    Ok(ExecutionDefaults {
        timeout_ms: execution
            .timeout
            .as_deref()
            .map(parse_duration_ms)
            .transpose()?,
        memory_bytes: execution
            .memory
            .as_deref()
            .map(parse_memory_bytes)
            .transpose()?,
    })
}

fn parse_duration_ms(value: &str) -> Result<u64> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1)
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
        .and_then(|value| value.checked_mul(multiplier))
        .ok_or_else(|| anyhow::anyhow!("duration is invalid or too large"))
}

fn parse_memory_bytes(value: &str) -> Result<u64> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("KiB") {
        (number, 1024)
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
        .and_then(|value| value.checked_mul(multiplier))
        .ok_or_else(|| anyhow::anyhow!("memory is invalid or too large"))
}

#[cfg(test)]
mod tests {
    use super::{resolve, resolve_optional};
    use std::{fs, path::Path};

    fn project(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("pit-manifest-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn arbitrary_single_filename_is_selected_without_parent_search() {
        let root = project("single");
        let parent =
            std::env::temp_dir().join(format!("pit-manifest-parent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(parent.join("child")).unwrap();
        let parent_manifest = parent.join("parent.pit");
        fs::write(&parent_manifest, "schema = 1\n[service]\nbuild = '.'\n").unwrap();
        fs::write(
            root.join("commerce.pit"),
            "schema = 1\nname = 'commerce'\n[service]\nbuild = '.'\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        let resolved = resolve(&root, None).unwrap();
        assert_eq!(resolved.path.file_name().unwrap(), "commerce.pit");
        let empty = parent.join("child");
        let error = resolve(&empty, None).unwrap_err();
        assert!(error.to_string().contains("ManifestNotFound"));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn app_tie_break_and_ambiguity_are_deterministic() {
        let root = project("tie");
        for name in ["dev.pit", "prod.pit"] {
            fs::write(root.join(name), "schema = 1\n[service]\nbuild = '.'\n").unwrap();
        }
        assert!(
            resolve(&root, None)
                .unwrap_err()
                .to_string()
                .contains("AmbiguousManifest")
        );
        fs::write(root.join("app.pit"), "schema = 1\n[service]\nbuild = '.'\n").unwrap();
        assert_eq!(
            resolve(&root, None).unwrap().path.file_name().unwrap(),
            "app.pit"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_missing_file_never_falls_back() {
        let root = project("explicit");
        fs::write(root.join("app.pit"), "schema = 1\n[service]\nbuild = '.'\n").unwrap();
        let error = resolve_optional(&root, Some(Path::new("missing.pit"))).unwrap_err();
        assert!(error.to_string().contains("ManifestNotFound"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn schema_and_unknown_fields_are_rejected_before_planning() {
        let root = project("schema");
        fs::write(root.join("app.pit"), "schema = 2\n[service]\nbuild = '.'\n").unwrap();
        let error = resolve(&root, None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Unsupported Pit Manifest schema")
        );

        fs::write(
            root.join("app.pit"),
            "schema = 1\nbuid = '.'\n[service]\nbuild = '.'\n",
        )
        .unwrap();
        let error = resolve(&root, None).unwrap_err();
        assert!(error.to_string().contains("unknown field `buid`"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn service_paths_are_relative_to_manifest_directory() {
        let root = project("relative");
        let config = root.join("configs");
        let source = root.join("api");
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(&source).unwrap();
        fs::write(
            config.join("customer.pit"),
            "schema = 1\n[service]\nbuild = '../api'\n",
        )
        .unwrap();

        let resolved = resolve(&root, Some(Path::new("configs/customer.pit"))).unwrap();
        let plan = resolved.plan().unwrap();
        assert_eq!(
            plan.services[0].project_dir,
            fs::canonicalize(source).unwrap()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resource_and_route_references_are_validated_in_the_plan() {
        let root = project("references");
        fs::create_dir_all(root.join("api")).unwrap();
        fs::write(
            root.join("app.pit"),
            "schema = 1\nname = 'shop'\n[services.api]\nbuild = 'api'\n[services.api.resources]\ndatabase = 'db'\n[resources.db]\ntype = 'postgres'\nfrom = 'DATABASE_URL'\n[routes]\n'/api' = 'api'\n",
        )
        .unwrap();
        let plan = resolve(&root, None).unwrap().plan().unwrap();
        assert_eq!(plan.resources["db"].kind, "postgres");
        assert_eq!(plan.routes["/api"].to_string(), "api");

        fs::write(
            root.join("app.pit"),
            "schema = 1\n[service]\nbuild = 'api'\n[service.resources]\ndatabase = 'missing'\n",
        )
        .unwrap();
        let error = resolve(&root, None).unwrap().plan().unwrap_err();
        assert!(error.to_string().contains("unknown resource 'missing'"));
        let _ = fs::remove_dir_all(root);
    }
}
