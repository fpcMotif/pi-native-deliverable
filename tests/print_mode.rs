use std::process::Command;

const EXIT_PARSE_ERROR: i32 = 2;
const EXIT_PROVIDER_OR_TOOL_ERROR: i32 = 3;

#[test]
fn print_mode_supports_short_p_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "-p",
            "hello world",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
        ])
        .output()
        .expect("run pi print mode");

    assert!(output.status.success(), "expected success: {output:?}");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Mock response: hello world"),
        "stdout: {stdout}"
    );
    assert!(stderr.trim().is_empty(), "stderr should be empty: {stderr}");
}

#[test]
fn parse_validation_errors_use_stable_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args(["--mode", "definitely-not-a-mode"])
        .output()
        .expect("run pi parse failure");

    assert_eq!(output.status.code(), Some(EXIT_PARSE_ERROR));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.trim().is_empty(), "stdout should be empty: {stdout}");
    assert!(
        stderr.contains("invalid value") || stderr.contains("possible values"),
        "stderr: {stderr}"
    );
}

#[test]
fn provider_failures_use_stable_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "-p",
            "hello",
            "--provider",
            "openai",
            "--model",
            "mock-tool-call",
        ])
        .env("PI_OPENAI_URL", "http://127.0.0.1:1")
        .output()
        .expect("run pi provider failure");

    assert_eq!(output.status.code(), Some(EXIT_PROVIDER_OR_TOOL_ERROR));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.trim().is_empty(), "stdout should be empty: {stdout}");
    assert!(
        stderr.contains("provider/tool error") || stderr.contains("provider_error"),
        "stderr: {stderr}"
    );
}
