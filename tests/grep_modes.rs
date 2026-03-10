use pi_search::{GrepMode, SearchService, SearchServiceConfig};
use std::fs;

/// PRD mapping:
/// - US-SEARCH-001: plain/regex modes produce expected matching semantics.
/// - test_suite.md §1.6 `grep_modes`: expected outputs for mode behavior.
#[tokio::test]
async fn grep_modes_plain_vs_regex() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("a.txt");
    fs::write(&path, "A.C literal\nABC wildcard\nAXC wildcard\n").expect("write");

    let svc = SearchService::new(SearchServiceConfig {
        workspace_root: tmp.path().to_path_buf(),
        use_git_status: false,
        watcher_enabled: false,
        ..Default::default()
    })
    .await
    .expect("search service");

    let plain = svc
        .grep("A.C", GrepMode::PlainText, "", 10)
        .await
        .expect("plain grep");
    assert_eq!(
        plain.matches.len(),
        1,
        "plain mode matches literal text only"
    );
    assert_eq!(plain.matches[0].line_number, 1);
    assert_eq!(plain.matches[0].line, "A.C literal");
    assert_eq!(plain.matches[0].context, "a.txt:1");

    let regex = svc
        .grep("A.C", GrepMode::Regex, "", 10)
        .await
        .expect("regex grep");
    let lines: Vec<&str> = regex.matches.iter().map(|m| m.line.as_str()).collect();
    assert_eq!(lines, vec!["A.C literal", "ABC wildcard", "AXC wildcard"]);
    assert_eq!(regex.stats.total_matches, 3);
    assert_eq!(regex.stats.matched_files, 1);
}
