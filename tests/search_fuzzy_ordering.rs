use pi_search::{decode_token, SearchQuery, SearchService, SearchServiceConfig};
use std::fs;

/// Fuzzy scoring ordering + pagination smoke test for `pi-search`.
#[tokio::test]
async fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus() {
    let tmp = tempfile::tempdir().expect("tempdir");

    for rel in [
        "src/main.rs",
        "crates/app/src/main.rs",
        "src/domain/main_service.rs",
        "src/maintenance.rs",
    ] {
        let path = tmp.path().join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir -p");
        fs::write(path, "fn main() {}\n").expect("write");
    }

    let svc = SearchService::new(SearchServiceConfig {
        workspace_root: tmp.path().to_path_buf(),
        use_git_status: false,
        watcher_enabled: false,
        ..Default::default()
    })
    .await
    .expect("search service");

    let first_page = svc
        .search(SearchQuery {
            text: "main".to_string(),
            scope: Some("src".to_string()),
            filters: vec![],
            limit: 2,
            token: None,
            offset: 0,
        })
        .await
        .expect("search first page");

    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.items[0].relative_path, "src/main.rs");
    assert_eq!(
        first_page.items[1].relative_path,
        "src/domain/main_service.rs"
    );

    let token = first_page.token.clone().expect("continuation token");
    assert_eq!(decode_token(&token).expect("decode token"), 2);

    let second_page = svc
        .search(SearchQuery {
            text: "main".to_string(),
            scope: Some("src".to_string()),
            filters: vec![],
            limit: 2,
            token: Some(token),
            offset: 999,
        })
        .await
        .expect("search second page");

    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].relative_path, "src/maintenance.rs");
    assert!(second_page.token.is_none());
    assert!(second_page.stats.token_used);
}
