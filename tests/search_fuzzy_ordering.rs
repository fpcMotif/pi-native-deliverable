use pi_search::{SearchQuery, SearchResponse, SearchService, SearchServiceConfig};
use std::fs;

async fn search_paths(files: &[&str], query: &str) -> SearchResponse {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    for file in files {
        let path = root.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, "fn main() {}\n").unwrap();
    }

    let service = SearchService::new(SearchServiceConfig {
        workspace_root: root.to_path_buf(),
        use_git_status: false,
        watcher_enabled: false,
        ..Default::default()
    })
    .await
    .unwrap();

    service
        .find_files(&SearchQuery {
            text: query.to_string(),
            scope: None,
            filters: vec![],
            limit: 10,
            token: None,
            offset: 0,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn fuzzy_scoring_prefers_shallower_entrypoints() {
    let res = search_paths(
        &[
            "src/main.rs",
            "crates/app/src/main.rs",
            "src/domain/main_service.rs",
            "src/domain/domain_model.rs",
        ],
        "main",
    )
    .await;

    assert!(res.items.len() >= 3);
    assert_eq!(res.items[0].relative_path, "src/main.rs");
    assert_eq!(res.items[1].relative_path, "crates/app/src/main.rs");
    assert_eq!(res.items[2].relative_path, "src/domain/main_service.rs");
    assert!(
        res.items[0].score > res.items[1].score,
        "shallower entrypoint should outrank deeper match: {} vs {}",
        res.items[0].score,
        res.items[1].score
    );
    assert!(
        res.items[1].score > res.items[2].score,
        "entrypoint bonus should outrank filename prefix match: {} vs {}",
        res.items[1].score,
        res.items[2].score
    );
}

#[tokio::test]
async fn fuzzy_scoring_breaks_equal_scores_alphabetically() {
    let res = search_paths(&["app/main.rs", "src/main.rs"], "main").await;

    assert_eq!(res.items.len(), 2);
    assert_eq!(res.items[0].relative_path, "app/main.rs");
    assert_eq!(res.items[1].relative_path, "src/main.rs");
    assert!(
        (res.items[0].score - res.items[1].score).abs() < f64::EPSILON,
        "same-length entrypoints should tie on score, got {} vs {}",
        res.items[0].score,
        res.items[1].score
    );
}
