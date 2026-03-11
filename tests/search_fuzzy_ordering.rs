use pi_search::{SearchQuery, SearchService, SearchServiceConfig};
use std::fs;

/// Fuzzy scoring ordering smoke test.
/// The exact scoring implementation is in `pi-search` and should remain stable for these cases.
#[tokio::test]
async fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    let files = vec![
        "src/main.rs",
        "crates/app/src/main.rs",
        "src/domain/main_service.rs",
    ];

    for file in &files {
        let path = root.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, "fn main() {}").unwrap();
    }

    let config = SearchServiceConfig {
        workspace_root: root.to_path_buf(),
        max_file_size: 1024 * 1024,
        max_lines_returned: 100,
        grep_line_limit: 300,
        use_git_status: false,
        watcher_enabled: false,
    };

    let service = SearchService::new(config).await.unwrap();
    let query = SearchQuery { text: "main".to_string(), scope: None, filters: vec![], limit: 10, token: None, offset: 0 };
    let res = service.find_files(&query).await.unwrap();

    assert!(res.items.len() >= 3, "Expected at least 3 items");
    assert_eq!(res.items[0].relative_path, "src/main.rs");
    assert_eq!(res.items[1].relative_path, "crates/app/src/main.rs");
    assert_eq!(res.items[2].relative_path, "src/domain/main_service.rs");
}
