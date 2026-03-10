use pi_search::{SearchQuery, SearchService, SearchServiceConfig};
use std::fs;

/// PRD mapping:
/// - test_suite.md §1.6 `fuzzy_scoring_ordering`: filename bonus outranks path-only matches.
/// - US-SEARCH-001 fuzzy ranking expectations for deterministic ordering.
#[tokio::test]
async fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus() {
    let tmp = tempfile::tempdir().expect("tempdir");

    for relative in [
        "src/main.rs",
        "crates/app/src/main.rs",
        "src/domain/main_service.rs",
        "experiments/main/guide.rs",
    ] {
        let full = tmp.path().join(relative);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdirs");
        fs::write(full, "fn placeholder() {}\n").expect("write");
    }

    let svc = SearchService::new(SearchServiceConfig {
        workspace_root: tmp.path().to_path_buf(),
        use_git_status: false,
        watcher_enabled: false,
        ..Default::default()
    })
    .await
    .expect("search service");

    let res = svc
        .search(SearchQuery {
            text: "main".to_string(),
            scope: None,
            filters: vec![],
            limit: 10,
            token: None,
            offset: 0,
        })
        .await
        .expect("fuzzy search");

    let ranked: Vec<&str> = res
        .items
        .iter()
        .map(|item| item.relative_path.as_str())
        .collect();
    assert_eq!(
        ranked,
        vec![
            "crates/app/src/main.rs",
            "src/main.rs",
            "src/domain/main_service.rs",
            "experiments/main/guide.rs",
        ]
    );

    let score = |path: &str| {
        res.items
            .iter()
            .find(|item| item.relative_path == path)
            .expect("path present")
            .score
    };

    assert!(score("src/main.rs") > score("src/domain/main_service.rs"));
    assert!(score("src/domain/main_service.rs") > score("experiments/main/guide.rs"));
}
