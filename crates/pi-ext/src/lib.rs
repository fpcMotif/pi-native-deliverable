#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, ExtensionError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    FileRead,
    FileWrite,
    FileEdit,
    NetworkHttp,
    Bash,
    SessionExport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
    pub capability: Capability,
}

#[derive(Debug, Clone)]
pub struct Policy {
    denied: Vec<Capability>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            denied: vec![Capability::NetworkHttp],
        }
    }
}

impl Policy {
    pub fn safe() -> Self {
        Self::default()
    }

    pub fn allow(mut self, cap: Capability) -> Self {
        self.denied.retain(|current| current != &cap);
        self
    }

    pub fn deny(mut self, cap: Capability) -> Self {
        if !self.denied.contains(&cap) {
            self.denied.push(cap);
        }
        self
    }

    pub fn check(&self, capability: Capability) -> PolicyDecision {
        let denied = self.denied.iter().any(|item| item == &capability);
        PolicyDecision {
            allowed: !denied,
            reason: if denied {
                "safe policy denies this capability".to_string()
            } else {
                "allowed by policy".to_string()
            },
            capability,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    pub entrypoint: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ExtensionRegistration {
    pub manifest: ExtensionManifest,
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub enum HostCall {
    ReadFile { path: PathBuf },
    WriteFile { path: PathBuf, content: String },
    RunBash { command: String },
    HttpGet { url: String },
    ExportSession,
}

impl HostCall {
    fn required_capability(&self) -> Capability {
        match self {
            Self::ReadFile { .. } => Capability::FileRead,
            Self::WriteFile { .. } => Capability::FileWrite,
            Self::RunBash { .. } => Capability::Bash,
            Self::HttpGet { .. } => Capability::NetworkHttp,
            Self::ExportSession => Capability::SessionExport,
        }
    }
}

pub trait Host {
    fn call(&self, call: HostCall) -> Result<Value>;
}

#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest parse error: {0}")]
    ManifestParse(String),
    #[error("extension not found: {0}")]
    NotFound(String),
    #[error("capability denied: {0}")]
    CapabilityDenied(String),
}

#[derive(Debug, Clone)]
pub struct ExtensionRuntime {
    policy: Policy,
    loaded: BTreeMap<String, ExtensionRegistration>,
}

impl ExtensionRuntime {
    pub fn new(policy: Policy) -> Self {
        Self {
            policy,
            loaded: BTreeMap::new(),
        }
    }

    pub fn load_from_roots(&mut self, roots: &[PathBuf]) -> Result<usize> {
        self.loaded.clear();
        let mut count = 0usize;
        for root in roots {
            if !root.exists() {
                continue;
            }
            for entry in std::fs::read_dir(root)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let manifest_path = path.join("extension.json");
                if !manifest_path.exists() {
                    continue;
                }
                let text = std::fs::read_to_string(&manifest_path)?;
                let manifest: ExtensionManifest = serde_json::from_str(&text)
                    .map_err(|err| ExtensionError::ManifestParse(err.to_string()))?;
                self.register(manifest, path);
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn register(&mut self, manifest: ExtensionManifest, root: PathBuf) {
        self.loaded.insert(
            manifest.name.clone(),
            ExtensionRegistration { manifest, root },
        );
    }

    pub fn unload(&mut self, name: &str) -> Option<ExtensionRegistration> {
        self.loaded.remove(name)
    }

    pub fn reload(&mut self, roots: &[PathBuf]) -> Result<usize> {
        self.load_from_roots(roots)
    }

    pub fn extensions(&self) -> Vec<&ExtensionRegistration> {
        self.loaded.values().collect()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.loaded
            .values()
            .flat_map(|reg| reg.manifest.tools.iter().cloned())
            .collect()
    }

    pub fn command_names(&self) -> Vec<String> {
        self.loaded
            .values()
            .flat_map(|reg| reg.manifest.commands.iter().cloned())
            .collect()
    }

    pub fn invoke_hostcall(
        &self,
        extension_name: &str,
        call: HostCall,
        host: &dyn Host,
    ) -> Result<Value> {
        let ext = self
            .loaded
            .get(extension_name)
            .ok_or_else(|| ExtensionError::NotFound(extension_name.to_string()))?;

        let required = call.required_capability();
        if !ext.manifest.capabilities.contains(&required) {
            return Err(ExtensionError::CapabilityDenied(format!(
                "extension '{}' missing capability {:?}",
                extension_name, required
            )));
        }
        let decision = self.policy.check(required.clone());
        if !decision.allowed {
            return Err(ExtensionError::CapabilityDenied(format!(
                "policy denied {:?}: {}",
                required, decision.reason
            )));
        }
        host.call(call)
    }
}

#[derive(Debug, Clone)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SkillValidationIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SkillDiscoveryResult {
    pub loaded: Vec<SkillManifest>,
    pub warnings: Vec<SkillValidationIssue>,
}

#[derive(Debug, Default)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    disabled: Option<bool>,
}

pub fn discover_skills(workspace_root: &Path) -> SkillDiscoveryResult {
    let mut result = SkillDiscoveryResult::default();
    let mut visited = HashSet::new();
    let mut disabled = read_disabled_skill_list(&workspace_root.join(".pi/skills.disabled"));

    if let Ok(home) = std::env::var("HOME") {
        disabled.extend(read_disabled_skill_list(
            &PathBuf::from(home).join(".pi/skills.disabled"),
        ));
    }
    disabled.extend(read_disabled_from_env());

    for root in skill_roots(workspace_root) {
        if !root.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(err) => {
                result.warnings.push(SkillValidationIssue {
                    path: root.clone(),
                    message: format!("unable to read directory: {err}"),
                });
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) != Some("md") {
                continue;
            }

            let canonical = path.canonicalize().unwrap_or(path.clone());
            if !visited.insert(canonical) {
                continue;
            }

            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    result.warnings.push(SkillValidationIssue {
                        path: path.clone(),
                        message: format!("unable to read skill file: {err}"),
                    });
                    continue;
                }
            };

            let Some(frontmatter) = parse_frontmatter(&text) else {
                result.warnings.push(SkillValidationIssue {
                    path: path.clone(),
                    message: "missing YAML frontmatter".to_string(),
                });
                continue;
            };

            let parsed = match parse_skill_frontmatter(frontmatter) {
                Ok(parsed) => parsed,
                Err(err) => {
                    result.warnings.push(SkillValidationIssue {
                        path: path.clone(),
                        message: err,
                    });
                    continue;
                }
            };

            let name = parsed.name.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|v| v.to_str())
                    .unwrap_or("unnamed-skill")
                    .to_string()
            });

            if disabled.contains(&name) || parsed.disabled.unwrap_or(false) {
                result.warnings.push(SkillValidationIssue {
                    path: path.clone(),
                    message: format!("skill '{name}' is disabled"),
                });
                continue;
            }

            let Some(description) = parsed.description else {
                result.warnings.push(SkillValidationIssue {
                    path: path.clone(),
                    message: format!("skill '{name}' missing required description"),
                });
                continue;
            };

            result.loaded.push(SkillManifest {
                name,
                description,
                path,
            });
        }
    }

    result
}

fn skill_roots(workspace_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![workspace_root.join(".pi/skills")];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(home).join(".pi/skills"));
    }
    roots
}

fn read_disabled_skill_list(path: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return out;
    };
    for line in text.lines() {
        let name = line.trim();
        if name.is_empty() || name.starts_with('#') {
            continue;
        }
        out.insert(name.to_string());
    }
    out
}

fn read_disabled_from_env() -> HashSet<String> {
    let mut out = HashSet::new();
    let Ok(value) = std::env::var("PI_DISABLED_SKILLS") else {
        return out;
    };
    for chunk in value.split(',') {
        let name = chunk.trim();
        if !name.is_empty() {
            out.insert(name.to_string());
        }
    }
    out
}

fn parse_frontmatter(text: &str) -> Option<&str> {
    if !text.starts_with("---\n") {
        return None;
    }
    let rem = &text[4..];
    let end = rem.find("\n---\n")?;
    Some(&rem[..end])
}

fn parse_skill_frontmatter(frontmatter: &str) -> std::result::Result<SkillFrontmatter, String> {
    let mut parsed = SkillFrontmatter::default();
    for line in frontmatter.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(format!("invalid frontmatter line: '{line}'"));
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key {
            "name" => parsed.name = Some(value.to_string()),
            "description" => parsed.description = Some(value.to_string()),
            "disabled" => {
                parsed.disabled = match value {
                    "true" => Some(true),
                    "false" => Some(false),
                    other => return Err(format!("invalid disabled value: '{other}'")),
                }
            }
            _ => {}
        }
    }
    Ok(parsed)
}

pub fn explain_policy(policy: &Policy, capability: Capability) -> String {
    let decision = policy.check(capability);
    format!("allowed={}: {}", decision.allowed, decision.reason)
}
