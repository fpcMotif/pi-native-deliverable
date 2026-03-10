use pi_search::{GrepMode, SearchService, SearchServiceConfig};
use std::fs;

/// Grep modes smoke test.
/// Ensures PlainText, Regex, and Fuzzy behave differently and include context metadata.
#[tokio::test]
async fn grep_modes_plain_vs_regex_vs_fuzzy() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("a.txt");
    fs::write(&path, "abc 123\nA-C 123\nabx 123\n").expect("write");

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
    assert_eq!(plain.matches.len(), 0, "literal A.C should not match");

    let regex = svc
        .grep("A.C", GrepMode::Regex, "", 10)
        .await
        .expect("regex grep");
    assert_eq!(regex.matches.len(), 1);
    assert_eq!(regex.matches[0].line_number, 2);
    assert_eq!(regex.matches[0].line, "A-C 123");
    assert_eq!(regex.matches[0].context, "a.txt:2");

    let fuzzy = svc
        .grep("abc 123", GrepMode::Fuzzy, "", 10)
        .await
        .expect("fuzzy grep");
    assert_eq!(fuzzy.matches.len(), 2);
    assert_eq!(fuzzy.matches[0].line_number, 1);
    assert_eq!(fuzzy.matches[1].line_number, 3);

    assert!(!regex.truncated);
    assert_eq!(regex.stats.matched_files, 1);
    assert_eq!(regex.stats.scanned_files, 1);
}
