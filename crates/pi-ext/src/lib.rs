#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
    ToolRegister,
    CommandRegister,
    EventHookRegister,
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
    pub tools: Vec<RuntimeRegistration>,
    #[serde(default)]
    pub commands: Vec<RuntimeRegistration>,
    #[serde(default)]
    pub event_hooks: Vec<EventHookRegistration>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRegistration {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventHookRegistration {
    pub event: String,
    pub handler: String,
}

#[derive(Debug, Clone)]
pub struct LoadedExtension {
    pub manifest_path: PathBuf,
    pub manifest: ExtensionManifest,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeHost {
    loaded: HashMap<String, LoadedExtension>,
    tools: HashMap<String, RegisteredItem>,
    commands: HashMap<String, RegisteredItem>,
    event_hooks: HashMap<String, Vec<RegisteredHook>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredItem {
    pub extension: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredHook {
    pub extension: String,
    pub event: String,
    pub handler: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("manifest io error at {path}: {source}")]
    ManifestIo {
        path: String,
        source: std::io::Error,
    },
    #[error("manifest parse error at {path}: {source}")]
    ManifestParse {
        path: String,
        source: serde_json::Error,
    },
    #[error("manifest validation error: {0}")]
    Validation(String),
    #[error("extension not loaded: {0}")]
    ExtensionNotLoaded(String),
    #[error("capability denied for extension '{extension}': {capability:?}")]
    CapabilityDenied {
        extension: String,
        capability: Capability,
    },
    #[error("registration conflict for {kind}: {name}")]
    RegistrationConflict { kind: &'static str, name: String },
}

pub fn explain_policy(policy: &Policy, capability: Capability) -> String {
    let decision = policy.check(capability);
    format!("allowed={}: {}", decision.allowed, decision.reason)
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<ExtensionManifest> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).map_err(|source| ExtensionError::ManifestIo {
        path: path.display().to_string(),
        source,
    })?;
    let manifest = serde_json::from_str::<ExtensionManifest>(&raw).map_err(|source| {
        ExtensionError::ManifestParse {
            path: path.display().to_string(),
            source,
        }
    })?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &ExtensionManifest) -> Result<()> {
    if manifest.name.trim().is_empty() {
        return Err(ExtensionError::Validation("name is required".to_string()));
    }
    if manifest.version.trim().is_empty() {
        return Err(ExtensionError::Validation(
            "version is required".to_string(),
        ));
    }
    if manifest.entrypoint.trim().is_empty() {
        return Err(ExtensionError::Validation(
            "entrypoint is required".to_string(),
        ));
    }

    ensure_unique_names("tools", &manifest.tools)?;
    ensure_unique_names("commands", &manifest.commands)?;

    let mut hook_pairs = HashSet::new();
    for hook in &manifest.event_hooks {
        if hook.event.trim().is_empty() || hook.handler.trim().is_empty() {
            return Err(ExtensionError::Validation(
                "event hooks require non-empty event and handler".to_string(),
            ));
        }
        let key = format!("{}:{}", hook.event, hook.handler);
        if !hook_pairs.insert(key) {
            return Err(ExtensionError::Validation(
                "duplicate event hook registration".to_string(),
            ));
        }
    }

    Ok(())
}

fn ensure_unique_names(kind: &str, items: &[RuntimeRegistration]) -> Result<()> {
    let mut names = HashSet::new();
    for item in items {
        if item.name.trim().is_empty() || item.description.trim().is_empty() {
            return Err(ExtensionError::Validation(format!(
                "{kind} registrations require non-empty name and description"
            )));
        }
        if !names.insert(item.name.clone()) {
            return Err(ExtensionError::Validation(format!(
                "duplicate {kind} registration: {}",
                item.name
            )));
        }
    }
    Ok(())
}

impl RuntimeHost {
    pub fn load_extension_manifest(&mut self, manifest_path: impl AsRef<Path>) -> Result<()> {
        let manifest_path = manifest_path.as_ref().to_path_buf();
        let manifest = load_manifest(&manifest_path)?;

        let extension_name = manifest.name.clone();
        self.unload_extension(&extension_name);

        self.loaded.insert(
            extension_name.clone(),
            LoadedExtension {
                manifest_path,
                manifest: manifest.clone(),
            },
        );

        for tool in manifest.tools {
            self.register_tool(&extension_name, tool)?;
        }
        for command in manifest.commands {
            self.register_command(&extension_name, command)?;
        }
        for hook in manifest.event_hooks {
            self.register_event_hook(&extension_name, hook)?;
        }

        Ok(())
    }

    pub fn reload_all(&mut self) -> Result<usize> {
        let manifests: Vec<_> = self
            .loaded
            .values()
            .map(|loaded| loaded.manifest_path.clone())
            .collect();

        let mut reloaded = 0usize;
        for path in manifests {
            self.load_extension_manifest(path)?;
            reloaded += 1;
        }

        Ok(reloaded)
    }

    pub fn register_tool(&mut self, extension: &str, tool: RuntimeRegistration) -> Result<()> {
        self.ensure_capability(extension, Capability::ToolRegister)?;
        Self::insert_item(&mut self.tools, "tool", extension, tool)
    }

    pub fn register_command(
        &mut self,
        extension: &str,
        command: RuntimeRegistration,
    ) -> Result<()> {
        self.ensure_capability(extension, Capability::CommandRegister)?;
        Self::insert_item(&mut self.commands, "command", extension, command)
    }

    pub fn register_event_hook(
        &mut self,
        extension: &str,
        hook: EventHookRegistration,
    ) -> Result<()> {
        self.ensure_capability(extension, Capability::EventHookRegister)?;
        let hooks = self.event_hooks.entry(hook.event.clone()).or_default();
        hooks.push(RegisteredHook {
            extension: extension.to_string(),
            event: hook.event,
            handler: hook.handler,
        });
        Ok(())
    }

    pub fn tools(&self) -> Vec<RegisteredItem> {
        self.tools.values().cloned().collect()
    }

    pub fn commands(&self) -> Vec<RegisteredItem> {
        self.commands.values().cloned().collect()
    }

    pub fn event_hooks(&self, event: &str) -> Vec<RegisteredHook> {
        self.event_hooks.get(event).cloned().unwrap_or_default()
    }

    fn insert_item(
        map: &mut HashMap<String, RegisteredItem>,
        kind: &'static str,
        extension: &str,
        item: RuntimeRegistration,
    ) -> Result<()> {
        if map.contains_key(&item.name) {
            return Err(ExtensionError::RegistrationConflict {
                kind,
                name: item.name,
            });
        }
        map.insert(
            item.name.clone(),
            RegisteredItem {
                extension: extension.to_string(),
                name: item.name,
                description: item.description,
            },
        );
        Ok(())
    }

    fn ensure_capability(&self, extension: &str, capability: Capability) -> Result<()> {
        let loaded = self
            .loaded
            .get(extension)
            .ok_or_else(|| ExtensionError::ExtensionNotLoaded(extension.to_string()))?;

        if loaded.manifest.capabilities.contains(&capability) {
            Ok(())
        } else {
            Err(ExtensionError::CapabilityDenied {
                extension: extension.to_string(),
                capability,
            })
        }
    }

    fn unload_extension(&mut self, extension: &str) {
        self.tools.retain(|_, tool| tool.extension != extension);
        self.commands
            .retain(|_, command| command.extension != extension);
        self.event_hooks.retain(|_, hooks| {
            hooks.retain(|hook| hook.extension != extension);
            !hooks.is_empty()
        });
    }
}
