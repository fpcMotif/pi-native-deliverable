use pi_search::{SearchQuery, SearchService, SearchServiceConfig};
use std::fs;
use std::path::Path;
use std::process::Command;

#[tokio::test]
async fn fuzzy_scoring_prefers_entrypoints_and_git_frecency() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_fixture_dir(Path::new("fixtures/repo_small"), tmp.path());

    Command::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .status()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "tests@example.com"])
        .current_dir(tmp.path())
        .status()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Tests"])
        .current_dir(tmp.path())
        .status()
        .expect("git config name");
    Command::new("git")
        .args(["add", "."])
        .current_dir(tmp.path())
        .status()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "baseline", "-q"])
        .current_dir(tmp.path())
        .status()
        .expect("git commit");

    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { println!(\"changed\"); }\n",
    )
    .expect("modify main.rs");

    let service = SearchService::new(SearchServiceConfig {
        workspace_root: tmp.path().to_path_buf(),
        watcher_enabled: false,
        ..Default::default()
    })
    .await
    .expect("search service");

    let result = service
        .find_files(&SearchQuery {
            text: "main".to_string(),
            scope: Some(".".to_string()),
            filters: Vec::new(),
            limit: 10,
            token: None,
            offset: 0,
        })
        .await
        .expect("find files");

    let top = result.items.first().expect("at least one result");
    assert_eq!(top.relative_path, "src/main.rs");
    assert!(
        top.frecency >= 10,
        "modified file should include frecency from git status"
    );
    assert!(
        top.git_status.is_some(),
        "modified file should have git status in indexed item"
    );
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
