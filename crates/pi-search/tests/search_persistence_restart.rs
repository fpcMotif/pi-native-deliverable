use pi_search::{SearchQuery, SearchService, SearchServiceConfig};

#[tokio::test]
async fn search_index_persists_and_falls_back_to_rebuild_on_stale_cache() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    std::fs::create_dir_all(root.join("src")).expect("mkdir");
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");

    let config = SearchServiceConfig {
        workspace_root: root.to_path_buf(),
        watcher_enabled: false,
        use_git_status: false,
        ..Default::default()
    };

    let first = SearchService::new(config.clone())
        .await
        .expect("first service");
    let first_response = first
        .find_files(&SearchQuery {
            text: "main.rs".to_string(),
            scope: Some(".".to_string()),
            filters: Vec::new(),
            limit: 10,
            token: None,
            offset: 0,
        })
        .await
        .expect("first search");
    assert!(first_response
        .items
        .iter()
        .any(|item| item.relative_path == "src/main.rs"));

    drop(first);

    std::fs::remove_file(root.join("src/main.rs")).expect("remove main");
    std::fs::write(root.join("src/other.rs"), "pub fn other() {}\n").expect("write other");

    let second = SearchService::new(config).await.expect("second service");
    let second_response = second
        .find_files(&SearchQuery {
            text: "main.rs".to_string(),
            scope: Some(".".to_string()),
            filters: Vec::new(),
            limit: 10,
            token: None,
            offset: 0,
        })
        .await
        .expect("second search");
    assert!(second_response
        .items
        .iter()
        .all(|item| item.relative_path != "src/main.rs"));
}
