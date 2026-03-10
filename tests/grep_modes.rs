use pi_search::{GrepMode, SearchService, SearchServiceConfig};
use std::fs;

/// Grep modes smoke test.
/// Ensures PlainText and Regex behave differently and return highlights.
#[test]
fn grep_modes_plain_vs_regex() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("a.txt"), "abc 123\nABC 123\nA.C literal\n").expect("write");

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

        let plain = service
            .grep("A.C", GrepMode::PlainText, ".", 20)
            .await
            .expect("plain grep");
        assert_eq!(plain.matches.len(), 1);
        assert_eq!(plain.matches[0].line, "A.C literal");

        fs::write(tmp.path().join("b.txt"), "ABC\nAXC\nA-C\n").expect("write");
        service.rebuild_index().await.expect("rebuild");

        let regex = service
            .grep("A.C", GrepMode::Regex, ".", 20)
            .await
            .expect("regex grep");
        let lines = regex
            .matches
            .iter()
            .map(|m| m.line.clone())
            .collect::<Vec<_>>();
        assert!(lines.iter().any(|line| line == "ABC"));
        assert!(lines.iter().any(|line| line == "AXC"));
        assert!(lines.iter().any(|line| line == "A-C"));
    });
}
