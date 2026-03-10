#![forbid(unsafe_code)]

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Skill,
    Prompt,
    Theme,
    Package,
}

impl ResourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Prompt => "prompt",
            Self::Theme => "theme",
            Self::Package => "package",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CatalogItem {
    pub kind: ResourceKind,
    pub name: String,
    pub description: String,
    pub manifest_path: PathBuf,
    pub detail_path: PathBuf,
    pub enabled: bool,
}

impl CatalogItem {
    pub fn id(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.name)
    }
}

#[derive(Debug, Clone)]
pub struct ManifestDiagnostic {
    pub kind: ResourceKind,
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub items: Vec<CatalogItem>,
    pub diagnostics: Vec<ManifestDiagnostic>,
}

impl Catalog {
    pub fn discover(workspace_root: &Path) -> Self {
        let mut diagnostics = Vec::new();
        let switches = load_switches(workspace_root, &mut diagnostics);
        let mut items = Vec::new();

        for root in standard_roots(workspace_root) {
            discover_skills(
                &root.join("skills"),
                &switches,
                &mut items,
                &mut diagnostics,
            );
            discover_json_resources(
                ResourceKind::Prompt,
                &root.join("prompts"),
                &switches,
                &mut items,
                &mut diagnostics,
            );
            discover_json_resources(
                ResourceKind::Theme,
                &root.join("themes"),
                &switches,
                &mut items,
                &mut diagnostics,
            );
            discover_json_resources(
                ResourceKind::Package,
                &root.join("packages"),
                &switches,
                &mut items,
                &mut diagnostics,
            );
        }

        let mut deduped = HashMap::new();
        for item in items {
            deduped.entry(item.id()).or_insert(item);
        }

        Self {
            items: deduped.into_values().collect(),
            diagnostics,
        }
    }

    pub fn enabled_items(&self, kind: ResourceKind) -> Vec<&CatalogItem> {
        self.items
            .iter()
            .filter(|item| item.kind == kind && item.enabled)
            .collect()
    }

    pub fn load_detail(&self, kind: ResourceKind, name: &str) -> Option<std::io::Result<String>> {
        self.items
            .iter()
            .find(|item| item.kind == kind && item.name == name && item.enabled)
            .map(|item| fs::read_to_string(&item.detail_path))
    }
}

#[derive(Debug, Deserialize, Default)]
struct ToggleFile {
    #[serde(default)]
    disabled: Vec<String>,
    #[serde(default)]
    enabled: Vec<String>,
}

#[derive(Debug, Default)]
struct Switches {
    disabled: HashSet<String>,
    enabled: HashSet<String>,
}

fn load_switches(workspace_root: &Path, diagnostics: &mut Vec<ManifestDiagnostic>) -> Switches {
    let mut switches = Switches::default();
    for path in config_locations(workspace_root) {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        match serde_json::from_str::<ToggleFile>(&contents) {
            Ok(value) => {
                switches.disabled.extend(value.disabled);
                switches.enabled.extend(value.enabled);
            }
            Err(err) => diagnostics.push(ManifestDiagnostic {
                kind: ResourceKind::Package,
                path,
                message: format!("invalid toggle config: {err}"),
            }),
        }
    }
    switches
}

fn config_locations(workspace_root: &Path) -> Vec<PathBuf> {
    let mut out = vec![workspace_root.join(".pi/pi-config.json")];
    if let Some(home) = home_dir() {
        out.push(home.join(".config/pi/pi-config.json"));
    }
    out
}

fn standard_roots(workspace_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        workspace_root.join(".pi"),
        workspace_root.join(".config/pi"),
    ];
    if let Some(home) = home_dir() {
        roots.push(home.join(".config/pi"));
        roots.push(home.join(".local/share/pi"));
    }
    roots
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn discover_skills(
    root: &Path,
    switches: &Switches,
    items: &mut Vec<CatalogItem>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let manifest_path = path.join("SKILL.md");
        if !manifest_path.exists() {
            continue;
        }

        let raw = match fs::read_to_string(&manifest_path) {
            Ok(raw) => raw,
            Err(err) => {
                diagnostics.push(ManifestDiagnostic {
                    kind: ResourceKind::Skill,
                    path: manifest_path,
                    message: format!("failed reading manifest: {err}"),
                });
                continue;
            }
        };

        let frontmatter = parse_frontmatter(&raw);
        let name = frontmatter.get("name").cloned().unwrap_or_else(|| {
            path.file_name()
                .and_then(|p| p.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        let description = match frontmatter.get("description") {
            Some(description) if !description.trim().is_empty() => description.clone(),
            _ => {
                diagnostics.push(ManifestDiagnostic {
                    kind: ResourceKind::Skill,
                    path: manifest_path,
                    message: "missing required metadata: description".to_string(),
                });
                continue;
            }
        };

        items.push(CatalogItem {
            kind: ResourceKind::Skill,
            enabled: is_enabled(switches, ResourceKind::Skill, &name),
            name,
            description,
            manifest_path: manifest_path.clone(),
            detail_path: manifest_path,
        });
    }
}

fn discover_json_resources(
    kind: ResourceKind,
    root: &Path,
    switches: &Switches,
    items: &mut Vec<CatalogItem>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        let raw = match fs::read_to_string(&manifest_path) {
            Ok(raw) => raw,
            Err(err) => {
                diagnostics.push(ManifestDiagnostic {
                    kind: kind.clone(),
                    path: manifest_path,
                    message: format!("failed reading manifest: {err}"),
                });
                continue;
            }
        };

        #[derive(Deserialize)]
        struct Manifest {
            name: String,
            description: String,
            #[serde(default)]
            content_file: Option<String>,
        }

        let manifest = match serde_json::from_str::<Manifest>(&raw) {
            Ok(value) => value,
            Err(err) => {
                diagnostics.push(ManifestDiagnostic {
                    kind: kind.clone(),
                    path: manifest_path,
                    message: format!("invalid manifest: {err}"),
                });
                continue;
            }
        };

        if manifest.description.trim().is_empty() {
            diagnostics.push(ManifestDiagnostic {
                kind: kind.clone(),
                path: manifest_path,
                message: "missing required metadata: description".to_string(),
            });
            continue;
        }

        let detail_path = manifest
            .content_file
            .as_deref()
            .map(|content| path.join(content))
            .unwrap_or_else(|| manifest_path.clone());

        items.push(CatalogItem {
            kind: kind.clone(),
            enabled: is_enabled(switches, kind.clone(), &manifest.name),
            name: manifest.name,
            description: manifest.description,
            manifest_path,
            detail_path,
        });
    }
}

fn is_enabled(switches: &Switches, kind: ResourceKind, name: &str) -> bool {
    let id = format!("{}:{name}", kind.as_str());
    if switches.enabled.contains(&id) {
        return true;
    }
    !switches.disabled.contains(&id)
}

fn parse_frontmatter(content: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return out;
    }

    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            out.insert(
                key.trim().to_string(),
                value.trim().trim_matches('"').to_string(),
            );
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter() {
        let map = parse_frontmatter("---\nname: demo\ndescription: test skill\n---\n# body");
        assert_eq!(map.get("name"), Some(&"demo".to_string()));
        assert_eq!(map.get("description"), Some(&"test skill".to_string()));
    }
}
