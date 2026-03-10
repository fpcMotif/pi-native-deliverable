use std::process::Command;

const EXIT_INVALID_USAGE: i32 = 64;
const EXIT_PROTOCOL_PARSE: i32 = 65;
const EXIT_RUNTIME_FAILURE: i32 = 70;

#[test]
fn print_mode_success_writes_answer_to_stdout_only() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "-p",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--prompt",
            "hello",
        ])
        .output()
        .expect("spawn pi print mode");

    assert_eq!(output.status.code(), Some(0), "expected successful exit");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.trim().is_empty(),
        "expected answer content on stdout"
    );
    assert!(
        !stdout.contains("\"type\":"),
        "stdout should not contain protocol event json lines: {stdout}"
    );
    assert!(
        stderr.trim().is_empty(),
        "stderr should be empty on success: {stderr}"
    );
}

#[test]
fn print_mode_missing_prompt_reports_usage_error_on_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args(["-p", "--provider", "mock", "--model", "mock-tool-call"])
        .output()
        .expect("spawn pi print mode without prompt");

    assert_eq!(output.status.code(), Some(EXIT_INVALID_USAGE));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty on usage error: {stdout}"
    );
    assert!(
        stderr.contains("missing --prompt"),
        "stderr should contain usage diagnostics: {stderr}"
    );
}

#[test]
fn print_mode_protocol_parse_error_reports_to_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "-p",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--prompt",
            "hello",
            "--request-version",
            "not-a-version",
        ])
        .output()
        .expect("spawn pi print mode parse failure");

    assert_eq!(output.status.code(), Some(EXIT_PROTOCOL_PARSE));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty on parse failure: {stdout}"
    );
    assert!(
        stderr.contains("request parse error"),
        "stderr should contain parse diagnostics: {stderr}"
    );
}

#[test]
fn print_mode_runtime_failure_reports_to_stderr() {
    let workspace = std::env::temp_dir().join("pi-print-mode-workspace-does-not-exist");
    let workspace_str = workspace.to_string_lossy().to_string();

    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "-p",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--prompt",
            "hello",
            "--workspace",
            &workspace_str,
        ])
        .output()
        .expect("spawn pi print mode runtime failure");

    assert_eq!(output.status.code(), Some(EXIT_RUNTIME_FAILURE));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty on runtime failure: {stdout}"
    );
    assert!(
        stderr.contains("runtime config error"),
        "stderr should contain runtime diagnostics: {stderr}"
    );
}
