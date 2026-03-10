#![forbid(unsafe_code)]

use pi_tools::{Tool, ToolCall, ToolCallResult, ToolRegistry, ToolStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub capabilities: Vec<Capability>,
    pub entrypoint: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ExtensionDescriptor {
    pub manifest: ExtensionManifest,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ExtensionLifecycleEvent {
    pub manifest: String,
    pub action: String,
}

pub trait ExtensionHostApi {
    fn register_tool(&mut self, tool: Box<dyn Tool>);
}

impl ExtensionHostApi for ToolRegistry {
    fn register_tool(&mut self, tool: Box<dyn Tool>) {
        self.register_boxed(tool);
    }
}

#[derive(Debug)]
pub struct ExtensionLoader {
    root: PathBuf,
    policy: Policy,
    loaded: HashMap<String, ExtensionDescriptor>,
}

impl ExtensionLoader {
    pub fn new(root: impl Into<PathBuf>, policy: Policy) -> Self {
        Self {
            root: root.into(),
            policy,
            loaded: HashMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn discover(&self) -> Result<Vec<ExtensionDescriptor>, LoaderError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut descriptors = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("manifest.json");
            if !manifest_path.exists() {
                continue;
            }

            let text = fs::read_to_string(&manifest_path)?;
            let manifest = serde_json::from_str::<ExtensionManifest>(&text).map_err(|err| {
                LoaderError::ManifestParse(manifest_path.clone(), err.to_string())
            })?;
            Self::validate_manifest(&manifest, &manifest_path)?;
            descriptors.push(ExtensionDescriptor { manifest, path });
        }

        descriptors.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        Ok(descriptors)
    }

    fn validate_manifest(manifest: &ExtensionManifest, path: &Path) -> Result<(), LoaderError> {
        if manifest.name.trim().is_empty() {
            return Err(LoaderError::ManifestValidation(
                path.to_path_buf(),
                "name is required".to_string(),
            ));
        }
        if manifest.version.trim().is_empty() {
            return Err(LoaderError::ManifestValidation(
                path.to_path_buf(),
                "version is required".to_string(),
            ));
        }
        if manifest.entrypoint.trim().is_empty() {
            return Err(LoaderError::ManifestValidation(
                path.to_path_buf(),
                "entrypoint is required".to_string(),
            ));
        }
        Ok(())
    }

    pub fn initialize(
        &mut self,
        host: &mut dyn ExtensionHostApi,
    ) -> Result<Vec<ExtensionLifecycleEvent>, LoaderError> {
        self.reload(host)
    }

    pub fn reload(
        &mut self,
        host: &mut dyn ExtensionHostApi,
    ) -> Result<Vec<ExtensionLifecycleEvent>, LoaderError> {
        let discovered = self.discover()?;
        let mut events = Vec::new();

        let next_map: HashMap<String, ExtensionDescriptor> = discovered
            .iter()
            .map(|item| (item.manifest.name.clone(), item.clone()))
            .collect();

        for removed in self
            .loaded
            .keys()
            .filter(|name| !next_map.contains_key(*name))
            .cloned()
            .collect::<Vec<_>>()
        {
            events.push(ExtensionLifecycleEvent {
                manifest: removed,
                action: "unload".to_string(),
            });
        }

        for descriptor in discovered {
            self.mount_extension(host, &descriptor)?;

            let action = if self.loaded.contains_key(&descriptor.manifest.name) {
                "reload"
            } else {
                "load"
            };

            events.push(ExtensionLifecycleEvent {
                manifest: descriptor.manifest.name.clone(),
                action: action.to_string(),
            });
        }

        self.loaded = next_map;
        Ok(events)
    }

    fn mount_extension(
        &self,
        host: &mut dyn ExtensionHostApi,
        descriptor: &ExtensionDescriptor,
    ) -> Result<(), LoaderError> {
        let tool_name = descriptor
            .manifest
            .metadata
            .get("tool_name")
            .cloned()
            .unwrap_or_else(|| format!("ext_{}", descriptor.manifest.name));

        let required = parse_required_capability(
            descriptor
                .manifest
                .metadata
                .get("required_capability")
                .map(String::as_str),
        )
        .unwrap_or(Capability::FileRead);

        if !descriptor.manifest.capabilities.contains(&required) {
            return Err(LoaderError::ManifestValidation(
                descriptor.path.join("manifest.json"),
                format!(
                    "required_capability {:?} not listed in capabilities",
                    required
                ),
            ));
        }

        host.register_tool(Box::new(ManifestTool {
            extension_name: descriptor.manifest.name.clone(),
            tool_name,
            description: descriptor
                .manifest
                .metadata
                .get("tool_description")
                .cloned()
                .unwrap_or_else(|| "Extension-provided tool".to_string()),
            response: descriptor
                .manifest
                .metadata
                .get("tool_response")
                .cloned()
                .unwrap_or_else(|| descriptor.manifest.entrypoint.clone()),
            required_capability: required,
            extension_policy: self.policy.clone(),
        }));

        Ok(())
    }

    pub fn loaded_extension_names(&self) -> HashSet<String> {
        self.loaded.keys().cloned().collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest parse failed at {0}: {1}")]
    ManifestParse(PathBuf, String),
    #[error("manifest validation failed at {0}: {1}")]
    ManifestValidation(PathBuf, String),
}

#[derive(Debug)]
struct ManifestTool {
    extension_name: String,
    tool_name: String,
    description: String,
    response: String,
    required_capability: Capability,
    extension_policy: Policy,
}

impl Tool for ManifestTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> Value {
        json!({
            "name": self.tool_name,
            "type": "object",
            "properties": {
                "input": {"type": "string"}
            }
        })
    }

    fn execute(
        &self,
        call: &ToolCall,
        _policy: &pi_tools::Policy,
        _cwd: &Path,
    ) -> pi_tools::Result<ToolCallResult> {
        let decision = self
            .extension_policy
            .check(self.required_capability.clone());
        if !decision.allowed {
            return Ok(ToolCallResult {
                stdout: String::new(),
                status: ToolStatus::Denied,
                error: Some(decision.reason),
                truncated: false,
                metadata: BTreeMap::from_iter([
                    ("extension_manifest".to_string(), json!(self.extension_name)),
                    (
                        "required_capability".to_string(),
                        json!(format!("{:?}", self.required_capability)),
                    ),
                ]),
            });
        }

        let input = call.args.get("input").and_then(Value::as_str).unwrap_or("");
        Ok(ToolCallResult {
            stdout: format!("{}{}", self.response, input),
            status: ToolStatus::Ok,
            error: None,
            truncated: false,
            metadata: BTreeMap::from_iter([
                ("extension_manifest".to_string(), json!(self.extension_name)),
                (
                    "required_capability".to_string(),
                    json!(format!("{:?}", self.required_capability)),
                ),
            ]),
        })
    }
}

fn parse_required_capability(value: Option<&str>) -> Option<Capability> {
    match value? {
        "file_read" => Some(Capability::FileRead),
        "file_write" => Some(Capability::FileWrite),
        "file_edit" => Some(Capability::FileEdit),
        "network_http" => Some(Capability::NetworkHttp),
        "bash" => Some(Capability::Bash),
        "session_export" => Some(Capability::SessionExport),
        _ => None,
    }
}

pub fn explain_policy(policy: &Policy, capability: Capability) -> String {
    let decision = policy.check(capability);
    format!("allowed={}: {}", decision.allowed, decision.reason)
}
