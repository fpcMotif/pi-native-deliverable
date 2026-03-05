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

        // Exact main.rs entrypoints should be top 2.
        // Both have identical scores (contains match + filename + entrypoint + extension bonuses),
        // so alphabetical path tiebreak determines order.
        assert_eq!(
            res.items[0].relative_path, "crates/app/src/main.rs",
            "alphabetically first entrypoint should rank first on score tie"
        );
        assert_eq!(
            res.items[1].relative_path, "src/main.rs",
            "alphabetically second entrypoint should rank second on score tie"
        );

        // Verify tied scores for the two exact entrypoint matches
        assert!(
            (res.items[0].score - res.items[1].score).abs() < f64::EPSILON,
            "both entrypoint matches should have equal scores, got {} vs {}",
            res.items[0].score,
            res.items[1].score
        );

        // Third result should score strictly lower than the tied entrypoints
        assert!(
            res.items[1].score > res.items[2].score,
            "entrypoint score ({}) should exceed non-entrypoint ({})",
            res.items[1].score,
            res.items[2].score
        );

        // Third should be the exact filename match prefix
        assert_eq!(res.items[2].relative_path, "src/domain/main_service.rs");
    });
}
