use pi_search::{SearchService, SearchServiceConfig};
use std::time::{Duration, Instant};

#[tokio::test]
async fn path_completion_respects_latency_budget_and_completes_matches() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    std::fs::create_dir_all(root.join("src")).expect("mkdir");
    std::fs::write(root.join("src/alpha.rs"), "pub fn alpha() {}\n").expect("write alpha");

    let service = SearchService::new(SearchServiceConfig {
        workspace_root: root.to_path_buf(),
        watcher_enabled: false,
        use_git_status: false,
        ..Default::default()
    })
    .await
    .expect("service");

    let start = Instant::now();
    let completed = service.complete_path_refs("open @src/al", 40).await;
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_millis(250));
    assert!(completed.contains("@src/alpha.rs"));
}
