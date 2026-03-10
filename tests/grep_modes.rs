use pi_search::{GrepMode, SearchService, SearchServiceConfig};
use std::fs;
use std::path::Path;

#[tokio::test]
async fn grep_modes_plain_vs_regex() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_fixture_dir(Path::new("fixtures/corpus_grep"), tmp.path());

    let service = SearchService::new(SearchServiceConfig {
        workspace_root: tmp.path().to_path_buf(),
        watcher_enabled: false,
        ..Default::default()
    })
    .await
    .expect("search service");

    let plain = service
        .grep("HELLO WORLD", GrepMode::PlainText, "", 10)
        .await
        .expect("plain grep");
    assert_eq!(
        plain.matches.len(),
        2,
        "plain text grep is case-insensitive in current engine"
    );

    let regex = service
        .grep("^HELLO WORLD$", GrepMode::Regex, "", 10)
        .await
        .expect("regex grep");
    assert_eq!(
        regex.matches.len(),
        1,
        "regex mode should honor exact-case anchored pattern"
    );

    let contexts: Vec<String> = regex.matches.iter().map(|m| m.context.clone()).collect();
    assert!(contexts.iter().any(|ctx| ctx.ends_with("simple.txt:2")));
}

fn copy_fixture_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create target fixture dir");
    for entry in fs::read_dir(src).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_fixture_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap_or_else(|err| {
                panic!(
                    "copy {} -> {} failed: {err}",
                    path.display(),
                    target.display()
                )
            });
        }
    }
}
