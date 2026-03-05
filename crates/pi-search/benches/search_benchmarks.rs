use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use pi_search::{GrepMode, GrepQuery, SearchQuery, SearchService, SearchServiceConfig};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;
use tokio::runtime::Runtime;

// Optional knobs for local benchmark sizing:
//  - PI_BENCH_GREP_FILES: number of grep text fixtures
//  - PI_BENCH_GREP_LINES: number of lines per grep fixture
//  - PI_BENCH_FIND_FILES: number of indexed files for find_files benchmark

const DEFAULT_GREP_FILES: usize = 12_000;
const DEFAULT_GREP_LINES: usize = 24;
const DEFAULT_GREP_MATCH_STRIDE: usize = 15;
const DEFAULT_FIND_FILES: usize = 50_000;

fn read_or_default(name: &str, fallback: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(fallback)
}

fn make_line_fileset(root: &Path, file_count: usize, lines_per_file: usize) -> std::io::Result<()> {
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
                    "Needle marker for benchmark file={} line={}",
                    file_idx, line_idx
                )?;
            } else {
                writeln!(out, "benchmark filler {} {}", file_idx, line_idx)?;
            }
        }
    }

    Ok(())
}

fn make_named_file_set(root: &Path, file_count: usize) -> std::io::Result<()> {
    for file_idx in 0..file_count {
        let dir = root.join("repo").join(format!("pkg_{:05}", file_idx));
        fs::create_dir_all(&dir)?;
        let path = dir.join("index.rs");
        let file = File::create(path)?;
        let mut out = BufWriter::new(file);

        writeln!(out, "pub fn item_{file_idx}() {{")?;
        writeln!(out, "    println!(\"needle marker {file_idx}\");")?;
        writeln!(out, "}}")?;
    }

    Ok(())
}

struct ServiceContext {
    _tmp: tempfile::TempDir,
    runtime: Runtime,
    service: std::sync::Arc<pi_search::SearchService>,
}

fn with_service<F>(file_count: usize, line_count: usize, builder: F) -> ServiceContext
where
    F: Fn(&Path, usize, usize) -> std::io::Result<()>,
{
    let tmp = tempdir().expect("temporary benchmark workspace");
    let root = tmp.path().to_path_buf();
    builder(&root, file_count, line_count).expect("bench fixture creation");

    let runtime = Runtime::new().expect("tokio runtime");
    let service = runtime.block_on(async {
        SearchService::new(SearchServiceConfig {
            workspace_root: root.clone(),
            watcher_enabled: false,
            ..Default::default()
        })
        .await
        .expect("search service init")
    });

    ServiceContext {
        _tmp: tmp,
        runtime,
        service,
    }
}

fn benchmark_grep_budget(c: &mut Criterion) {
    let file_count = read_or_default("PI_BENCH_GREP_FILES", DEFAULT_GREP_FILES);
    let line_count = read_or_default("PI_BENCH_GREP_LINES", DEFAULT_GREP_LINES);

    let context = with_service(file_count, line_count, |root, count, lines| {
        make_line_fileset(root, count, lines)
    });

    let query = GrepQuery {
        pattern: "Needle marker for benchmark".to_string(),
        mode: GrepMode::PlainText,
        scope: ".".to_string(),
        limit: 50,
        token: None,
        offset: 0,
    };

    let mut group = c.benchmark_group("grep_corpus_budget");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(4));
    group.warm_up_time(Duration::from_secs(1));

    group.bench_function("first_page", |bench| {
        let service = context.service.clone();
        let runtime = &context.runtime;
        bench.iter(|| {
            let response = runtime
                .block_on(service.grep_query(query.clone()))
                .expect("grep query");
            black_box(response.matches.len());
        });
    });

    group.finish();
}

fn benchmark_find_files_50k(c: &mut Criterion) {
    let file_count = read_or_default("PI_BENCH_FIND_FILES", DEFAULT_FIND_FILES);

    let context = with_service(file_count, 1, |root, count, _| {
        make_named_file_set(root, count)
    });

    let query = SearchQuery {
        text: "index".to_string(),
        scope: None,
        filters: vec![],
        limit: 50,
        token: None,
        offset: 0,
    };

    let mut group = c.benchmark_group("fuzzy_50k_files_p95");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(4));
    group.warm_up_time(Duration::from_secs(1));

    group.bench_function("find_files", |bench| {
        let service = context.service.clone();
        let runtime = &context.runtime;
        bench.iter(|| {
            let response = runtime
                .block_on(service.find_files(&query))
                .expect("find files query");
            black_box(response.items.len());
        });
    });

    group.finish();
}

criterion_group!(
    search_benchmarks,
    benchmark_grep_budget,
    benchmark_find_files_50k
);
criterion_main!(search_benchmarks);
