use std::path::Path;
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_pi")
}

#[test]
fn help_lists_required_commands_and_flags() {
    let output = Command::new(bin())
        .arg("--help")
        .output()
        .expect("run --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("-p, --print <PROMPT>"));
    assert!(stdout.contains("--continue"));
    assert!(stdout.contains("--session <SESSION>"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("search"));
    assert!(stdout.contains("info"));
    assert!(stdout.contains("update-index"));
}

#[test]
fn print_short_flag_writes_to_stdout() {
    let workspace = tempfile::tempdir().expect("workspace");
    let output = Command::new(bin())
        .args(["-p", "hello", "--workspace"])
        .arg(workspace.path())
        .output()
        .expect("run print");

    assert!(output.status.success());
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    let filtered_stderr: Vec<&str> = stderr_str
        .lines()
        .filter(|line| !line.contains("pi-search: watcher save_index failed"))
        .filter(|line| !line.trim().is_empty())
        .collect();

    assert!(filtered_stderr.is_empty(), "stderr: {:?}", filtered_stderr);
    assert!(!output.stdout.is_empty());
}

#[test]
fn continue_and_session_flags_wire_session_store() {
    let workspace = tempfile::tempdir().expect("workspace");
    let session = workspace.path().join(".pi/custom-session.jsonl");

    let first = Command::new(bin())
        .args(["-p", "first", "--workspace"])
        .arg(workspace.path())
        .args(["--session"])
        .arg(&session)
        .output()
        .expect("first run");
    assert!(first.status.success());

    let first_len = line_count(&session);
    assert!(first_len > 0);

    let second = Command::new(bin())
        .args(["-p", "second", "--workspace"])
        .arg(workspace.path())
        .args(["--session"])
        .arg(&session)
        .args(["--continue"])
        .output()
        .expect("second run");
    assert!(second.status.success());

    let second_len = line_count(&session);
    assert!(second_len > first_len);
}

#[test]
fn doctor_search_info_and_update_index_commands_work() {
    let doctor = Command::new(bin()).arg("doctor").output().expect("doctor");
    assert!(doctor.status.success());
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("doctor: ok"));

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = repo_root.join("fixtures/repo_small");
    let search = Command::new(bin())
        .args(["search", "main"])
        .current_dir(&fixture)
        .output()
        .expect("search");
    assert!(search.status.success());
    assert!(String::from_utf8_lossy(&search.stdout).contains("src/main.rs"));

    let info = Command::new(bin())
        .args(["info", "core"])
        .output()
        .expect("info");
    assert!(info.status.success());
    assert!(String::from_utf8_lossy(&info.stdout).contains("core: available"));

    let update = Command::new(bin())
        .arg("update-index")
        .current_dir(&fixture)
        .output()
        .expect("update-index");
    assert!(update.status.success());
    assert!(String::from_utf8_lossy(&update.stdout).contains("index updated"));
}

#[test]
fn interactive_slash_commands_are_handled() {
    let workspace = tempfile::tempdir().expect("workspace");
    let mut child = Command::new(bin())
        .args(["--workspace"])
        .arg(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn interactive");

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin
            .write_all(b"/help\n/model\n/tree\n/compact\n/reload\n/exit\n")
            .expect("write input");
    }

    let output = child.wait_with_output().expect("wait output");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("/help /model /tree /clear /compact /exit /reload"));
    assert!(stdout.contains("model: mock-tool-call"));
    assert!(stdout.contains("session tree:"));
    assert!(stdout.contains("compacted"));
    assert!(stdout.contains("reload complete"));
}

fn line_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .expect("read session")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}
