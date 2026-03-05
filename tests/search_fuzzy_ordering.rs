/// Fuzzy scoring ordering smoke test.
/// The exact scoring implementation is in `pi-search` and should remain stable for these cases.
#[test]
fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = ["src", "crates/app/src", "src/domain"];
        for d in dirs {
            std::fs::create_dir_all(tmp.path().join(d)).unwrap();
        }

        std::fs::write(tmp.path().join("src/main.rs"), "").unwrap();
        std::fs::write(tmp.path().join("crates/app/src/main.rs"), "").unwrap();
        std::fs::write(tmp.path().join("src/domain/main_service.rs"), "").unwrap();
        // and a distraction
        std::fs::write(tmp.path().join("src/domain/domain_model.rs"), "").unwrap();

        let svc = pi_search::SearchService::new(pi_search::SearchServiceConfig {
            workspace_root: tmp.path().to_path_buf(),
            ..Default::default()
        })
        .await
        .unwrap();

        let query = pi_search::SearchQuery {
            text: "main".to_string(),
            scope: None,
            filters: vec![],
            limit: 10,
            token: None,
            offset: 0,
        };

        let res = svc.find_files(&query).await.unwrap();
        assert!(res.items.len() >= 3);

        let path0 = &res.items[0].relative_path;
        let path1 = &res.items[1].relative_path;

        // Exact main.rs entrypoints should be top 2
        let top_two = vec![path0.as_str(), path1.as_str()];
        assert!(top_two.contains(&"src/main.rs"));
        assert!(top_two.contains(&"crates/app/src/main.rs"));

        // Third should be the exact filename match prefix
        assert_eq!(res.items[2].relative_path, "src/domain/main_service.rs");
    });
}
