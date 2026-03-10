use pi_search::{SearchQuery, SearchService, SearchServiceConfig};
use std::fs;

/// Fuzzy scoring ordering smoke test.
/// The exact scoring implementation is in `pi-search` and should remain stable for these cases.
#[test]
fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(tmp.path().join("src/domain")).expect("mkdir");
    fs::create_dir_all(tmp.path().join("crates/app/src")).expect("mkdir");

    fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(tmp.path().join("crates/app/src/main.rs"), "fn main() {}\n").expect("write app main");
    fs::write(
        tmp.path().join("src/domain/main_service.rs"),
        "pub fn run_main_service() {}\n",
    )
    .expect("write service");

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let service = SearchService::new(SearchServiceConfig {
            workspace_root: tmp.path().to_path_buf(),
            watcher_enabled: false,
            use_git_status: false,
            ..SearchServiceConfig::default()
        })
        .await
        .expect("search service");

        let result = service
            .search(SearchQuery {
                text: "main".to_string(),
                scope: Some(".".to_string()),
                filters: Vec::new(),
                limit: 10,
                token: None,
                offset: 0,
            })
            .await
            .expect("search");

        let paths = result
            .items
            .iter()
            .map(|item| item.relative_path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(paths[0], "src/main.rs");
        assert_eq!(paths[1], "crates/app/src/main.rs");
        assert_eq!(paths[2], "src/domain/main_service.rs");
    });
}
