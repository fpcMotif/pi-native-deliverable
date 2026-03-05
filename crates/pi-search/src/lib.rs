#![forbid(unsafe_code)]

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ignore::WalkBuilder;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrepMode {
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
pub struct GrepMatch {
    pub path: String,
    pub line_number: usize,
    pub byte_offset: usize,
    pub line: String,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepStats {
    pub scanned_files: usize,
    pub total_matches: usize,
    pub matched_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepResponse {
    pub matches: Vec<GrepMatch>,
    pub token: Option<String>,
    pub truncated: bool,
    pub stats: GrepStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchServiceConfig {
    pub workspace_root: PathBuf,
    pub max_file_size: u64,
    pub max_lines_returned: usize,
    pub grep_line_limit: usize,
    pub use_git_status: bool,
    pub watcher_enabled: bool,
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

        service.rebuild_index().await?;
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
        walker.hidden(false).git_ignore(true).parents(true).build().for_each(|result| {
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
                .map(|map| map.get(path).map(|status| status_to_frecency(status)).unwrap_or(0))
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
                            service.rebuild_index().await.ok();
                            if service.config.use_git_status {
                                service.refresh_git_status().await.ok();
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

        Ok(SearchResponse { items, token, stats })
    }

    pub async fn grep(
        &self,
        pattern: &str,
        mode: GrepMode,
        scope: &str,
        limit: usize,
    ) -> SearchResult<GrepResponse> {
        let index = self.index.read().await;
        let mut matches = Vec::new();
        let mut stats = GrepStats {
            scanned_files: 0,
            total_matches: 0,
            matched_files: 0,
        };

        let regex = if let GrepMode::Regex = mode {
            Some(Regex::new(pattern)?)
        } else {
            None
        };

        let lower = pattern.to_lowercase();
        for entry in index.iter() {
            if !entry.relative_path.starts_with(scope) {
                continue;
            }

            let bytes = match std::fs::read(&entry.absolute_path) {
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
            for (line_idx, line) in lines.iter().enumerate() {
                if matches.len() >= limit || matches.len() >= self.config.grep_line_limit {
                    break;
                }

                let matched = match mode {
                    GrepMode::PlainText => line.to_lowercase().contains(&lower),
                    GrepMode::Regex => {
                        let regex = regex.as_ref().expect("regex");
                        regex.is_match(line)
                    }
                    GrepMode::Fuzzy => {
                        normalized_levenshtein(&line.to_lowercase(), &lower) >= 0.72
                    }
                };

                if !matched {
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
                let byte_offset: usize = lines.iter().take(line_idx).map(|line| line.len() + 1).sum();

                matches.push(GrepMatch {
                    path: entry.relative_path.clone(),
                    line_number: line_idx + 1,
                    byte_offset,
                    line: (*line).to_string(),
                    before,
                    after,
                    context: format!("{}:{}", entry.relative_path, line_idx + 1),
                });

                file_matched = true;
                stats.total_matches += 1;
            }

            if file_matched {
                stats.matched_files += 1;
            }

            if matches.len() >= limit || matches.len() >= self.config.grep_line_limit {
                break;
            }
        }

        let truncated = matches.len() >= limit || matches.len() >= self.config.grep_line_limit;
        Ok(GrepResponse {
            matches,
            token: None,
            truncated,
            stats,
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
    match scope {
        Some(scope) if !scope.is_empty() => {
            if scope == "." {
                true
            } else {
                entry.relative_path.starts_with(scope)
            }
        }
        None => true,
        Some(_) => true,
    }
}

fn matches_filters(entry: &IndexedFile, filters: &[SearchFilter], query: &str) -> bool {
    if filters.is_empty() {
        return !query.is_empty();
    }

    for filter in filters {
        let ext_ok = filter
            .extension
            .as_ref()
            .is_none_or(|ext| entry.relative_path.ends_with(&format!(".{ext}")));
        let scope_ok = filter
            .path_prefix
            .as_ref()
            .is_none_or(|prefix| entry.relative_path.starts_with(prefix));
        if !ext_ok || !scope_ok {
            return false;
        }
    }
    true
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
        return Err(SearchError::InvalidToken("token payload size mismatch".to_string()));
    }
    let value = u64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| SearchError::InvalidToken("invalid token payload".to_string()))?,
    );
    Ok(value.try_into().map_err(|_| SearchError::InvalidToken("token overflow".to_string()))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_token() {
        // 0 as u64 is 8 bytes of 0.
        let token = encode_token(0);
        assert_eq!(token, "AAAAAAAAAAA=");

        // 1 as u64 is 7 bytes of 0, followed by 1.
        let token = encode_token(1);
        assert_eq!(token, "AAAAAAAAAAE=");

        // 42 as u64
        let token = encode_token(42);
        assert_eq!(token, "AAAAAAAAACo=");
    }

    #[test]
    fn test_token_roundtrip() {
        let test_cases = vec![
            0,
            1,
            42,
            100,
            1024,
            usize::MAX,
        ];

        for &val in &test_cases {
            let encoded = encode_token(val);
            let decoded = decode_token(&encoded).expect("Should decode successfully");
            assert_eq!(decoded, val, "Failed roundtrip for value: {}", val);
        }
    }

    #[test]
    fn test_decode_invalid_token() {
        // Invalid base64
        assert!(matches!(decode_token("not base64!"), Err(SearchError::InvalidToken(_))));

        // Valid base64 but wrong size (e.g. 4 bytes instead of 8)
        let wrong_size = STANDARD.encode(1u32.to_be_bytes());
        assert!(matches!(decode_token(&wrong_size), Err(SearchError::InvalidToken(_))));

        // Valid base64 but wrong size (e.g. 9 bytes)
        let mut nine_bytes = [0u8; 9];
        nine_bytes[8] = 1;
        let wrong_size = STANDARD.encode(nine_bytes);
        assert!(matches!(decode_token(&wrong_size), Err(SearchError::InvalidToken(_))));
    }
}
