#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    pub capabilities: Vec<Capability>,
    pub entrypoint: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl ExtensionManifest {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ManifestLoadError> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path).map_err(|source| ManifestLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| ManifestLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestLoadError {
    #[error("manifest io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("manifest parse error at {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ManifestDiagnostic {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ManifestLoadReport {
    pub loaded: Vec<ExtensionManifest>,
    pub diagnostics: Vec<ManifestDiagnostic>,
}

pub fn load_manifests_from_dir(path: impl AsRef<Path>) -> ManifestLoadReport {
    let mut report = ManifestLoadReport::default();
    let Ok(entries) = std::fs::read_dir(path) else {
        return report;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }

        match ExtensionManifest::from_path(&path) {
            Ok(manifest) => report.loaded.push(manifest),
            Err(err) => report.diagnostics.push(ManifestDiagnostic {
                path: path.clone(),
                message: err.to_string(),
            }),
        }
    }

    report
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleAction {
    Loaded,
    Reloaded,
    Unloaded,
    InvocationAllowed,
    InvocationDenied,
}

#[derive(Debug, Clone)]
pub struct LifecycleEvent {
    pub manifest: String,
    pub action: LifecycleAction,
    pub detail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("extension not registered: {0}")]
    MissingExtension(String),
    #[error("extension {extension} missing required capability {capability:?}")]
    MissingCapability {
        extension: String,
        capability: Capability,
    },
    #[error("capability denied for extension {extension}: {reason}")]
    CapabilityDenied { extension: String, reason: String },
}

#[derive(Debug, Clone)]
struct RegisteredExtension {
    manifest: ExtensionManifest,
}

#[derive(Debug, Clone)]
pub struct HostcallOutcome {
    pub extension: String,
    pub hostcall: String,
    pub allowed: bool,
    pub reason: String,
}

#[derive(Debug)]
pub struct ExtensionRuntime {
    policy: Policy,
    manifest_dir: PathBuf,
    extensions: HashMap<String, RegisteredExtension>,
}

impl ExtensionRuntime {
    pub fn new(policy: Policy, manifest_dir: PathBuf) -> Self {
        Self {
            policy,
            manifest_dir,
            extensions: HashMap::new(),
        }
    }

    pub fn register(&mut self, manifest: ExtensionManifest) {
        self.extensions
            .insert(manifest.name.clone(), RegisteredExtension { manifest });
    }

    pub fn reload(&mut self) -> (ManifestLoadReport, Vec<LifecycleEvent>) {
        let report = load_manifests_from_dir(&self.manifest_dir);
        let previous = self.extensions.clone();
        let mut next = HashMap::new();
        let mut events = Vec::new();

        for manifest in &report.loaded {
            let action = if let Some(existing) = previous.get(&manifest.name) {
                if existing.manifest.version != manifest.version {
                    LifecycleAction::Reloaded
                } else {
                    LifecycleAction::Loaded
                }
            } else {
                LifecycleAction::Loaded
            };
            events.push(LifecycleEvent {
                manifest: manifest.name.clone(),
                action,
                detail: format!("version={}", manifest.version),
            });
            next.insert(
                manifest.name.clone(),
                RegisteredExtension {
                    manifest: manifest.clone(),
                },
            );
        }

        for removed in previous.keys() {
            if !next.contains_key(removed) {
                events.push(LifecycleEvent {
                    manifest: removed.clone(),
                    action: LifecycleAction::Unloaded,
                    detail: "manifest removed".to_string(),
                });
            }
        }

        self.extensions = next;
        (report, events)
    }

    pub fn invoke_hostcall(
        &self,
        extension: &str,
        hostcall: &str,
        capability: Capability,
    ) -> Result<(HostcallOutcome, LifecycleEvent), RuntimeError> {
        let Some(registered) = self.extensions.get(extension) else {
            return Err(RuntimeError::MissingExtension(extension.to_string()));
        };
        if !registered.manifest.capabilities.contains(&capability) {
            return Err(RuntimeError::MissingCapability {
                extension: extension.to_string(),
                capability,
            });
        }

        let decision = self.policy.check(capability.clone());
        if decision.allowed {
            let outcome = HostcallOutcome {
                extension: extension.to_string(),
                hostcall: hostcall.to_string(),
                allowed: true,
                reason: decision.reason.clone(),
            };
            let event = LifecycleEvent {
                manifest: extension.to_string(),
                action: LifecycleAction::InvocationAllowed,
                detail: format!("hostcall={hostcall} capability={capability:?}"),
            };
            Ok((outcome, event))
        } else {
            Err(RuntimeError::CapabilityDenied {
                extension: extension.to_string(),
                reason: decision.reason,
            })
        }
    }
}

pub fn explain_policy(policy: &Policy, capability: Capability) -> String {
    let decision = policy.check(capability);
    format!("allowed={}: {}", decision.allowed, decision.reason)
}
