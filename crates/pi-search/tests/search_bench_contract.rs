use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio::runtime::Runtime;

use pi_search::{GrepMode, GrepQuery, SearchQuery, SearchService, SearchServiceConfig};

const DEFAULT_GREP_FILES: usize = 5_000;
const DEFAULT_GREP_LINES: usize = 24;
const DEFAULT_GREP_MATCH_STRIDE: usize = 15;
const DEFAULT_FIND_FILES: usize = 10_000;

const DEFAULT_GREP_BUDGET_MS: u64 = 25;
const DEFAULT_FIND_BUDGET_MS: u64 = 20;

const BENCH_ITERATIONS: usize = 50;

fn read_or_default(name: &str, fallback: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(fallback)
}

fn read_or_default_u64(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(fallback)
}

fn make_grep_line_fileset(
    root: &Path,
    file_count: usize,
    lines_per_file: usize,
) -> std::io::Result<()> {
    for file_idx in 0..file_count {
        let dir = root.join(format!("mod_{:04x}", file_idx % 256));
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("source_{file_idx:08}.txt"));
        let file = File::create(&path)?;
        let mut out = BufWriter::new(file);

        for line_idx in 0..lines_per_file {
            if line_idx % DEFAULT_GREP_MATCH_STRIDE == 0 {
                writeln!(
                    out,
                    "Needle marker for budget test file={} line={}",
                    file_idx, line_idx
                )?;
            } else {
                writeln!(out, "budget fixture filler {} {}", file_idx, line_idx)?;
            }
        }
    }

    Ok(())
}

fn make_find_file_set(root: &Path, file_count: usize) -> std::io::Result<()> {
    for file_idx in 0..file_count {
        let dir = root.join("repo").join(format!("pkg_{:05}", file_idx));
        fs::create_dir_all(&dir)?;
        let path = dir.join("index.rs");
        let file = File::create(path)?;
        let mut out = BufWriter::new(file);

        writeln!(out, "pub fn item_{file_idx}() {{")?;
        writeln!(out, "    println!(\"index marker {file_idx}\");")?;
        writeln!(out, "}}")?;
    }

    Ok(())
}

fn percentile_nanos(durations: &mut [u128], percentile: f64) -> u128 {
    durations.sort_unstable();
    let idx = (((durations.len().saturating_sub(1) as f64) * percentile / 100.0).round() as usize)
        .min(durations.len().saturating_sub(1));
    durations[idx]
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn grep_first_page_stays_within_budget() {
    let file_count = read_or_default("PI_BENCH_ASSERT_GREP_FILES", DEFAULT_GREP_FILES);
    let line_count = read_or_default("PI_BENCH_ASSERT_GREP_LINES", DEFAULT_GREP_LINES);
    let budget_ms = read_or_default_u64("PI_BENCH_ASSERT_GREP_BUDGET_MS", DEFAULT_GREP_BUDGET_MS);

    let tmp = tempdir().expect("temporary benchmark workspace");
    make_grep_line_fileset(tmp.path(), file_count, line_count).expect("build grep fixture set");

    let runtime = Runtime::new().expect("tokio runtime");
    let service = runtime.block_on(async {
        SearchService::new(SearchServiceConfig {
            workspace_root: tmp.path().to_path_buf(),
            watcher_enabled: false,
            ..Default::default()
        })
        .await
        .expect("search service init")
    });

    let query = GrepQuery {
        pattern: "Needle marker for budget test".to_string(),
        mode: GrepMode::PlainText,
        scope: ".".to_string(),
        limit: 100,
        token: None,
        offset: 0,
    };

    let mut times = Vec::with_capacity(BENCH_ITERATIONS);
    for _ in 0..BENCH_ITERATIONS {
        let started = Instant::now();
        let response = runtime
            .block_on(service.grep_query(query.clone()))
            .expect("grep query");
        assert!(!response.matches.is_empty());
        times.push(started.elapsed().as_nanos());
    }

    let mut p95_input = times.clone();
    let p95 = percentile_nanos(&mut p95_input, 95.0);
    let budget = Duration::from_millis(budget_ms);
    assert!(
        p95 <= budget.as_nanos(),
        "grep first page p95 {}ns above budget {}ms (p95={p95}ns)",
        p95,
        budget_ms
    );

    let max = *times.iter().max().expect("has samples");
    assert!(
        max <= budget.as_nanos() * 2,
        "grep first page max {max}ns above double budget {}ms",
        budget_ms
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn find_files_stays_within_budget() {
    let file_count = read_or_default("PI_BENCH_ASSERT_FIND_FILES", DEFAULT_FIND_FILES);
    let budget_ms = read_or_default_u64("PI_BENCH_ASSERT_FIND_BUDGET_MS", DEFAULT_FIND_BUDGET_MS);

    let tmp = tempdir().expect("temporary benchmark workspace");
    make_find_file_set(tmp.path(), file_count).expect("build find fixture set");

    let runtime = Runtime::new().expect("tokio runtime");
    let service = runtime.block_on(async {
        SearchService::new(SearchServiceConfig {
            workspace_root: tmp.path().to_path_buf(),
            watcher_enabled: false,
            ..Default::default()
        })
        .await
        .expect("search service init")
    });

    let query = SearchQuery {
        text: "index".to_string(),
        scope: None,
        filters: vec![],
        limit: 100,
        token: None,
        offset: 0,
    };

    let mut times = Vec::with_capacity(BENCH_ITERATIONS);
    for _ in 0..BENCH_ITERATIONS {
        let started = Instant::now();
        let response = runtime
            .block_on(service.find_files(&query))
            .expect("find query");
        assert!(!response.items.is_empty());
        times.push(started.elapsed().as_nanos());
    }

    let mut p95_input = times.clone();
    let p95 = percentile_nanos(&mut p95_input, 95.0);
    let budget = Duration::from_millis(budget_ms);
    assert!(
        p95 <= budget.as_nanos(),
        "find_files p95 {}ns above budget {}ms (p95={p95}ns)",
        p95,
        budget_ms
    );

    let max = *times.iter().max().expect("has samples");
    assert!(
        max <= budget.as_nanos() * 2,
        "find_files max {max}ns above double budget {}ms",
        budget_ms
    );
}
