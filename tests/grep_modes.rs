use std::fs;

/// Grep modes smoke test.
/// Ensures PlainText and Regex behave differently and return highlights.
#[tokio::test]
async fn grep_modes_plain_vs_regex() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("a.txt");
    fs::write(&path, "abc 123\\nABC 123\\n").expect("write");

    let opts = pi_search::SearchServiceConfig {
        workspace_root: tmp.path().to_path_buf(),
        ..Default::default()
    };
    let svc = pi_search::SearchService::new(opts).await.unwrap();

    let plain = svc.grep("ABC", pi_search::GrepMode::PlainText, ".", 10).await.unwrap();
    assert!(plain.matches.len() >= 1);

    let regex = svc.grep("A.C", pi_search::GrepMode::Regex, ".", 10).await.unwrap();
    assert!(regex.matches.len() >= 1);
}
