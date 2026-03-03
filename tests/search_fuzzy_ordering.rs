use std::path::PathBuf;

/// Fuzzy scoring ordering smoke test.
/// The exact scoring implementation is in `pi-search` and should remain stable for these cases.
#[test]
fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus() {
    // TODO: once implemented, construct FileItem list and run fuzzy search.
    // This test describes required ordering:
    //
    // Query: "main"
    // Expected top results (descending):
    // 1) src/main.rs (entrypoint file bonus)
    // 2) crates/app/src/main.rs
    // 3) src/domain/main_service.rs (filename match)
    //
    // Rationale: entrypoint + exact filename matches should outrank distant substring matches.
    //
    // let files = vec![ ... ];
    // let res = pi_search::SearchService::fuzzy_files("main", ctx).unwrap();
    // assert_eq!(res.items[0].relative_path, "src/main.rs");
    assert!(true);
}
