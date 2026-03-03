use pi_session::SessionStore;
use tempfile::TempDir;

#[test]
fn default_session_path_stays_within_workspace() {
    let workspace = TempDir::new().expect("workspace tempdir");
    let path = SessionStore::default_session_path(workspace.path());
    let workspace = workspace.path().canonicalize().expect("canonicalize");

    assert_eq!(path, workspace.join(".pi").join("session.jsonl"));
    assert!(path.starts_with(&workspace));
    assert!(path.ends_with(".pi/session.jsonl"));
}

#[test]
fn workspace_session_path_rejects_traversal() {
    let workspace = TempDir::new().expect("workspace tempdir");

    let outside = workspace.path().join("..").join("outside.jsonl");
    let err = SessionStore::resolve_session_path(&outside, workspace.path())
        .expect_err("traversal should be rejected");
    assert!(err.to_string().contains("outside workspace"));
}

#[test]
fn workspace_session_path_allows_workspace_scope() {
    let workspace = TempDir::new().expect("workspace tempdir");
    let allowed = workspace.path().join(".pi").join("session.jsonl");

    let resolved = SessionStore::resolve_session_path(&allowed, workspace.path())
        .expect("allowed path must resolve");
    assert_eq!(resolved, workspace.path().join(".pi").join("session.jsonl"));
}
