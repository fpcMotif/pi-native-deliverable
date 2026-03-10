#![forbid(unsafe_code)]

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ignore::WalkBuilder;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use strsim::normalized_levenshtein;
use tokio::sync::{mpsc, RwLock};

pub type SearchResult<T> = std::result::Result<T, SearchError>;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("regex: {0}")]
    Regex(#[from] regex::Error),
    #[error("notify: {0}")]
    Notify(String),
    #[error("invalid token: {0}")]
    InvalidToken(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilter {
    pub path_prefix: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrepMode {
    #[default]
    PlainText,
    Regex,
    Fuzzy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub filters: Vec<SearchFilter>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub offset: usize,
}

const fn default_limit() -> usize {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchStats {
    pub scanned_files: usize,
    pub matched_files: usize,
    pub total_matches: usize,
    pub token_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub score: f64,
    pub mtime_ms: u64,
    pub frecency: u32,
    pub git_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub items: Vec<SearchItem>,
    pub token: Option<String>,
    pub stats: SearchStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepMatchSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepMatch {
    pub path: String,
    pub line_number: usize,
    pub byte_offset: usize,
    pub line: String,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub context: String,
    #[serde(default)]
    pub highlights: Vec<GrepMatchSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepStats {
    pub scanned_files: usize,
    pub total_matches: usize,
    pub matched_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepQuery {
    pub pattern: String,
    #[serde(default)]
    pub mode: GrepMode,
    #[serde(default)]
    pub scope: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepResponse {
    pub matches: Vec<GrepMatch>,
    pub token: Option<String>,
    pub truncated: bool,
    pub stats: GrepStats,
    #[serde(default)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchServiceConfig {
    pub workspace_root: PathBuf,
    pub max_file_size: u64,
    pub max_lines_returned: usize,
    pub grep_line_limit: usize,
    pub use_git_status: bool,
    pub watcher_enabled: bool,
    pub index_path: Option<PathBuf>,
}

impl Default for SearchServiceConfig {
    fn default() -> Self {
        Self {
            workspace_root: PathBuf::from("."),
            max_file_size: 4 * 1024 * 1024,
            max_lines_returned: 100,
            grep_line_limit: 300,
            use_git_status: true,
            watcher_enabled: true,
            index_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexedFile {
    relative_path: String,
    absolute_path: PathBuf,
    size_bytes: u64,
    mtime_ms: u64,
    frecency: u32,
    git_status: Option<String>,
}

#[derive(Debug)]
pub struct SearchService {
    config: SearchServiceConfig,
    index: RwLock<Vec<IndexedFile>>,
    git_index: RwLock<HashMap<PathBuf, String>>,
}

impl SearchService {
    pub async fn new(config: SearchServiceConfig) -> SearchResult<std::sync::Arc<Self>> {
        let service = std::sync::Arc::new(Self {
            config,
            index: RwLock::new(Vec::new()),
            git_index: RwLock::new(HashMap::new()),
        });

        if !service.load_index().await {
            service.rebuild_index().await?;
            service.save_index().await.ok();
        }

        if service.config.use_git_status {
            service.refresh_git_status().await?;
        }

        if service.config.watcher_enabled {
            let _ = service.start_watcher().await;
        }

        Ok(service)
    }

    pub async fn rebuild_index(&self) -> SearchResult<()> {
        let mut items = Vec::new();
        let root = self.config.workspace_root.clone();

        let mut walker = WalkBuilder::new(&root);
        walker
            .hidden(false)
            .git_ignore(true)
            .parents(true)
            .build()
            .for_each(|result| {
                let entry = match result {
                    Ok(value) => value,
                    Err(_) => return,
                };
                let path = entry.path();
                if !path.is_file() {
                    return;
                }

                let metadata = match path.metadata() {
                    Ok(value) => value,
                    Err(_) => return,
                };
                if metadata.len() > self.config.max_file_size {
                    return;
                }

                let relative = path
                    .strip_prefix(&root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                if should_ignore_path(&relative) {
                    return;
                }

                let modified_ms = metadata
                    .modified()
                    .unwrap_or_else(|_| SystemTime::now())
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let frecency = self
                    .git_index
                    .try_read()
                    .ok()
                    .map(|map| {
                        map.get(path)
                            .map(|status| status_to_frecency(status))
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);

                items.push(IndexedFile {
                    relative_path: relative,
                    absolute_path: path.to_path_buf(),
                    size_bytes: metadata.len(),
                    mtime_ms: modified_ms,
                    frecency,
                    git_status: None,
                });
            });

        items.sort_by(|left, right| {
            left.relative_path
                .cmp(&right.relative_path)
                .then(left.mtime_ms.cmp(&right.mtime_ms))
        });

        let mut index = self.index.write().await;
        *index = items;
        Ok(())
    }

    pub async fn load_index(&self) -> bool {
        if let Some(path) = &self.config.index_path {
            if let Ok(bytes) = tokio::fs::read(path).await {
                if let Ok(items) = serde_json::from_slice::<Vec<IndexedFile>>(&bytes) {
                    *self.index.write().await = items;
                    return true;
                }
            }
        }
        false
    }

    pub async fn save_index(&self) -> SearchResult<()> {
        if let Some(path) = &self.config.index_path {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let json = {
                let index = self.index.read().await;
                serde_json::to_string(&*index).ok()
            };
            if let Some(json) = json {
                tokio::fs::write(path, json).await?;
            }
        }
        Ok(())
    }

    pub async fn start_watcher(self: &std::sync::Arc<Self>) -> SearchResult<()> {
        let (tx, mut rx) =
            mpsc::unbounded_channel::<std::result::Result<notify::Event, notify::Error>>();
        let mut watcher = RecommendedWatcher::new(
            move |value| {
                let _ = tx.send(value);
            },
            Config::default(),
        )
        .map_err(|err| SearchError::Notify(err.to_string()))?;

        watcher
            .watch(&self.config.workspace_root, RecursiveMode::Recursive)
            .map_err(|err| SearchError::Notify(err.to_string()))?;

        let service = self.clone();
        tokio::spawn(async move {
            let _watcher = watcher;
            while let Some(event) = rx.recv().await {
                if let Ok(event) = event {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                            if let Err(err) = service.rebuild_index().await {
                                eprintln!("pi-search: watcher rebuild_index failed: {err}");
                            }
                            if let Err(err) = service.save_index().await {
                                eprintln!("pi-search: watcher save_index failed: {err}");
                            }
                            if service.config.use_git_status {
                                if let Err(err) = service.refresh_git_status().await {
                                    eprintln!(
                                        "pi-search: watcher refresh_git_status failed: {err}"
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });
        Ok(())
    }

    pub async fn search(&self, query: SearchQuery) -> SearchResult<SearchResponse> {
        self.find_files(&query).await
    }

    pub async fn find_files(&self, query: &SearchQuery) -> SearchResult<SearchResponse> {
        if query.token.is_some() && query.offset != 0 {
            return Err(SearchError::InvalidToken(
                "cannot provide both token and non-zero offset".to_string(),
            ));
        }

        let start = if let Some(token) = query.token.as_deref() {
            decode_token(token)?
        } else {
            query.offset
        };

        let index = self.index.read().await;
        let needle = query.text.to_lowercase();
        let mut matched = Vec::new();
        let mut stats = SearchStats {
            scanned_files: index.len(),
            matched_files: 0,
            total_matches: 0,
            token_used: query.token.is_some(),
        };

        for entry in index.iter() {
            if !matches_scope(entry, query.scope.as_deref()) {
                continue;
            }
            if !matches_filters(entry, &query.filters, &needle) {
                continue;
            }

            let base = score_path_match(&entry.relative_path, &needle);
            if base <= 0.0 {
                continue;
            }

            let bonus = score_filename_bonus(&entry.relative_path, &needle)
                + score_extension_bonus(&entry.absolute_path)
                + score_entrypoint_bonus(&entry.relative_path)
                + score_git_bonus(entry.git_status.as_deref())
                + frecency_score(entry.frecency);
            let score = base + bonus;

            matched.push(SearchItem {
                relative_path: entry.relative_path.clone(),
                absolute_path: entry.absolute_path.clone(),
                score,
                mtime_ms: entry.mtime_ms,
                frecency: entry.frecency,
                git_status: entry.git_status.clone(),
            });
            stats.matched_files += 1;
            stats.total_matches += 1;
        }

        matched.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });

        let max = (start.saturating_add(query.limit)).min(matched.len());
        let items = matched.get(start..max).unwrap_or_default().to_vec();
        let token = if max < matched.len() {
            Some(encode_token(max))
        } else {
            None
        };

        Ok(SearchResponse {
            items,
            token,
            stats,
        })
    }

    pub async fn grep(
        &self,
        pattern: &str,
        mode: GrepMode,
        scope: &str,
        limit: usize,
    ) -> SearchResult<GrepResponse> {
        self.grep_query(GrepQuery {
            pattern: pattern.to_string(),
            mode,
            scope: scope.to_string(),
            limit,
            token: None,
            offset: 0,
        })
        .await
    }

    pub async fn grep_query(&self, query: GrepQuery) -> SearchResult<GrepResponse> {
        if query.token.is_some() && query.offset != 0 {
            return Err(SearchError::InvalidToken(
                "cannot provide both token and non-zero offset".to_string(),
            ));
        }

        let index = self.index.read().await;
        let limit = query.limit.max(1).min(self.config.grep_line_limit);
        let start = query
            .token
            .as_deref()
            .map(decode_token)
            .transpose()?
            .unwrap_or(query.offset);

        enum Matcher {
            Regex(Regex),
            Fuzzy,
        }

        let mut warning = None;
        let matcher = match query.mode {
            GrepMode::Fuzzy => Matcher::Fuzzy,
            GrepMode::PlainText => Matcher::Regex(
                regex::RegexBuilder::new(&regex::escape(&query.pattern))
                    .case_insensitive(true)
                    .build()?,
            ),
            GrepMode::Regex => match Regex::new(&query.pattern) {
                Ok(regex) => Matcher::Regex(regex),
                Err(err) => {
                    warning = Some(format!(
                        "invalid regex pattern: {err}. Falling back to plain text matching."
                    ));
                    Matcher::Regex(
                        regex::RegexBuilder::new(&regex::escape(&query.pattern))
                            .case_insensitive(true)
                            .build()?,
                    )
                }
            },
        };

        let lower = query.pattern.to_lowercase();
        let query_char_count = lower.chars().count();
        let mut lower_line = String::new();
        let mut matches = Vec::new();
        let required = start.saturating_add(limit).saturating_add(1);
        let mut has_more = false;
        let mut stats = GrepStats {
            scanned_files: 0,
            total_matches: 0,
            matched_files: 0,
        };

        let workspace_canonical = self.config.workspace_root.canonicalize().ok();

        for entry in index.iter() {
            if !scope_is_prefix(&entry.relative_path, &query.scope) {
                continue;
            }

            // Resolve symlinks and verify path stays within workspace
            let resolved_path = match entry.absolute_path.canonicalize() {
                Ok(path) => path,
                Err(_) => continue,
            };
            if let Some(ref root) = workspace_canonical {
                if !resolved_path.starts_with(root) {
                    continue;
                }
            }

            let bytes = match std::fs::read(&resolved_path) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if bytes.contains(&0) {
                continue;
            }

            stats.scanned_files += 1;
            let text = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = text.lines().collect();

            let mut file_matched = false;
            let mut byte_offset = 0usize;

            for (line_idx, line) in lines.iter().enumerate() {
                let line_match_spans = match &matcher {
                    Matcher::Regex(regex) => collect_match_spans(line, regex),
                    Matcher::Fuzzy => {
                        let line_char_count = line.chars().count();
                        let max_len = line_char_count.max(query_char_count);
                        let len_diff = line_char_count.abs_diff(query_char_count);

                        if max_len > 0 && (len_diff as f64) / (max_len as f64) > 0.28 {
                            Vec::new()
                        } else {
                            lower_line.clear();
                            for c in line.chars() {
                                for lc in c.to_lowercase() {
                                    lower_line.push(lc);
                                }
                            }
                            let line_match = normalized_levenshtein(&lower_line, &lower) >= 0.72;
                            if line_match {
                                collect_fuzzy_spans(line, &query.pattern)
                            } else {
                                Vec::new()
                            }
                        }
                    }
                };

                if line_match_spans.is_empty() {
                    byte_offset += line.len() + 1;
                    continue;
                }

                let before = lines
                    .iter()
                    .take(line_idx)
                    .skip(line_idx.saturating_sub(2))
                    .map(|line| (*line).to_string())
                    .collect::<Vec<_>>();
                let after = lines
                    .iter()
                    .skip(line_idx + 1)
                    .take(2)
                    .map(|line| (*line).to_string())
                    .collect::<Vec<_>>();

                matches.push(GrepMatch {
                    path: entry.relative_path.clone(),
                    line_number: line_idx + 1,
                    byte_offset,
                    line: (*line).to_string(),
                    before,
                    after,
                    context: format!("{}:{}", entry.relative_path, line_idx + 1),
                    highlights: line_match_spans,
                });

                file_matched = true;
                if matches.len() >= required {
                    has_more = true;
                    break;
                }
                byte_offset += line.len() + 1;
            }

            if file_matched {
                stats.matched_files += 1;
            }

            if has_more {
                break;
            }
        }

        matches.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line_number.cmp(&right.line_number))
                .then(left.byte_offset.cmp(&right.byte_offset))
                .then(left.line.cmp(&right.line))
        });

        let max = (start.saturating_add(limit)).min(matches.len());
        let page = matches.get(start..max).unwrap_or_default().to_vec();
        let truncated = if max < matches.len() { true } else { has_more };
        let token = if truncated {
            Some(encode_token(max))
        } else {
            None
        };
        stats.total_matches = matches.len();

        Ok(GrepResponse {
            matches: page,
            token,
            truncated,
            stats,
            warning,
        })
    }

    pub async fn refresh_git_status(&self) -> SearchResult<()> {
        let git_root = self.config.workspace_root.to_string_lossy().to_string();
        let output = Command::new("git")
            .arg("-C")
            .arg(git_root)
            .args(["status", "--porcelain"])
            .output()?;

        if !output.status.success() {
            return Ok(());
        }

        let status = String::from_utf8_lossy(&output.stdout);
        let mut status_map = self.git_index.write().await;
        status_map.clear();

        for line in status.lines() {
            if line.len() < 4 {
                continue;
            }
            let code = &line[..2];
            let path = line[3..].trim();
            let absolute = self.config.workspace_root.join(path);
            status_map.insert(absolute, code.to_string());
        }

        Ok(())
    }
}

fn matches_scope(entry: &IndexedFile, scope: Option<&str>) -> bool {
    scope.map_or(true, |scope| scope_is_prefix(&entry.relative_path, scope))
}

fn matches_filters(entry: &IndexedFile, filters: &[SearchFilter], query: &str) -> bool {
    if filters.is_empty() {
        return !query.is_empty();
    }

    for filter in filters {
        let ext_ok = filter
            .extension
            .as_ref()
            .map_or(true, |ext| entry.relative_path.ends_with(&format!(".{ext}")));
        let scope_ok = filter
            .path_prefix
            .as_ref()
            .map_or(true, |prefix| scope_is_prefix(&entry.relative_path, prefix));
        if !ext_ok || !scope_ok {
            return false;
        }
    }
    true
}

fn scope_is_prefix(path: &str, scope: &str) -> bool {
    let trimmed = scope.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return true;
    }

    let mut normalized_scope = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => normalized_scope.push(part),
            Component::RootDir => normalized_scope.push(component.as_os_str()),
            Component::Prefix(_) => normalized_scope.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => return false,
        }
    }

    if normalized_scope.as_os_str().is_empty() {
        return true;
    }

    Path::new(path).starts_with(&normalized_scope)
}

fn collect_match_spans(line: &str, regex: &Regex) -> Vec<GrepMatchSpan> {
    regex
        .find_iter(line)
        .map(|capture| GrepMatchSpan {
            start: capture.start(),
            end: capture.end(),
        })
        .collect()
}

fn collect_fuzzy_spans(line: &str, pattern: &str) -> Vec<GrepMatchSpan> {
    let line_lower = line.to_lowercase();
    let pattern_lower = pattern.to_lowercase();
    line_lower
        .find(&pattern_lower)
        .map(|start| {
            vec![GrepMatchSpan {
                start,
                end: start + pattern_lower.len(),
            }]
        })
        .unwrap_or_else(|| {
            if line.is_empty() {
                Vec::new()
            } else {
                vec![GrepMatchSpan {
                    start: 0,
                    end: line.len(),
                }]
            }
        })
}


fn should_ignore_path(relative: &str) -> bool {
    relative.starts_with(".git/")
        || relative.starts_with("target/")
        || relative.starts_with(".pi/")
        || relative.starts_with("node_modules/")
}

fn score_path_match(path: &str, query: &str) -> f64 {
    if query.is_empty() {
        return 0.0;
    }
    let path_lc = path.to_lowercase();
    if path_lc == query {
        return 1.0;
    }
    if path_lc.contains(query) {
        return 0.9;
    }
    normalized_levenshtein(&path_lc, query)
}

fn score_filename_bonus(path: &str, query: &str) -> f64 {
    let filename = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    if filename == query {
        0.6
    } else if filename.starts_with(query) {
        0.35
    } else if filename.contains(query) {
        0.25
    } else {
        0.0
    }
}

fn score_extension_bonus(path: &Path) -> f64 {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => 0.07,
        Some("toml") | Some("json") | Some("md") => 0.05,
        Some(_) => 0.02,
        None => 0.0,
    }
}

fn score_entrypoint_bonus(path: &str) -> f64 {
    if path.ends_with("/main.rs") || path.ends_with("/lib.rs") || path.ends_with("/mod.rs") {
        0.08
    } else {
        0.0
    }
}

fn score_git_bonus(status: Option<&str>) -> f64 {
    match status {
        Some("M") | Some("MM") | Some("??") => 0.12,
        Some("A") | Some("AM") | Some(" D") => 0.08,
        _ => 0.0,
    }
}

fn frecency_score(v: u32) -> f64 {
    (v as f64).min(20.0) / 200.0
}

fn status_to_frecency(status: &str) -> u32 {
    match status {
        "M" | "MM" => 10,
        "A" | "AM" => 8,
        "??" => 6,
        _ => 2,
    }
}

pub fn encode_token(index: usize) -> String {
    STANDARD.encode((index as u64).to_be_bytes())
}

pub fn decode_token(token: &str) -> SearchResult<usize> {
    let bytes = STANDARD
        .decode(token)
        .map_err(|err| SearchError::InvalidToken(err.to_string()))?;
    if bytes.len() != 8 {
        return Err(SearchError::InvalidToken(
            "token payload size mismatch".to_string(),
        ));
    }
    let value = u64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| SearchError::InvalidToken("invalid token payload".to_string()))?,
    );
    value
        .try_into()
        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_prefix_matches_components_not_prefix_fragments() {
        assert!(scope_is_prefix("foo/bar/baz.txt", "foo"));
        assert!(scope_is_prefix("foo/bar/baz.txt", "foo/"));
        assert!(scope_is_prefix("foo/bar/baz.txt", "foo/bar"));
        assert!(!scope_is_prefix("foo-secret/bar.txt", "foo"));
    }

    #[test]
    fn scope_prefix_rejects_parent_directory_segments() {
        assert!(!scope_is_prefix("other/bar.txt", "../other"));
        assert!(!scope_is_prefix("foo/bar.txt", "foo/../foo"));
    }

    #[test]
    fn scope_is_prefix_supports_special_inputs() {
        assert!(scope_is_prefix("any/path.txt", "."));
        assert!(scope_is_prefix("any/path.txt", ""));
        assert!(!scope_is_prefix("foo/bar.txt", "baz"));
    }

    #[test]
    fn test_encode_token() {
        let token = encode_token(0);
        assert_eq!(token, "AAAAAAAAAAA=");

        let token = encode_token(1);
        assert_eq!(token, "AAAAAAAAAAE=");

        let token = encode_token(42);
        assert_eq!(token, "AAAAAAAAACo=");
    }

    #[test]
    fn test_token_roundtrip() {
        let test_cases = vec![0, 1, 42, 100, 1024, usize::MAX];

        for &val in &test_cases {
            let encoded = encode_token(val);
            let decoded = decode_token(&encoded).expect("Should decode successfully");
            assert_eq!(decoded, val, "Failed roundtrip for value: {}", val);
        }
    }

    #[test]
    fn test_decode_invalid_token() {
        // Invalid base64
        assert!(matches!(
            decode_token("not base64!"),
            Err(SearchError::InvalidToken(_))
        ));

        // Valid base64 but wrong size (e.g. 4 bytes instead of 8)
        let wrong_size = STANDARD.encode(1u32.to_be_bytes());
        assert!(matches!(
            decode_token(&wrong_size),
            Err(SearchError::InvalidToken(_))
        ));

        // Valid base64 but wrong size (e.g. 9 bytes)
        let mut nine_bytes = [0u8; 9];
        nine_bytes[8] = 1;
        let wrong_size = STANDARD.encode(nine_bytes);
        assert!(matches!(
            decode_token(&wrong_size),
            Err(SearchError::InvalidToken(_))
        ));
    }
}