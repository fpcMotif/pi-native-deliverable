use std::process::Command;

#[test]
fn print_mode_success_writes_only_stdout_and_returns_zero() {
    let workspace = tempfile::tempdir().expect("tempdir");

    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--mode",
            "print",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--workspace",
            workspace.path().to_str().expect("workspace path utf8"),
            "-p",
            "hello",
        ])
        .output()
        .expect("run pi print");

    assert_eq!(output.status.code(), Some(0), "expected success exit code");
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty on success"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("message_update"),
        "expected print mode events on stdout"
    );
    assert!(
        !stdout.contains("\"type\":\"error\""),
        "did not expect error event on stdout"
    );
}

#[test]
fn print_mode_bad_input_writes_only_stderr_and_returns_two() {
    let workspace = tempfile::tempdir().expect("tempdir");

    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--mode",
            "print",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--workspace",
            workspace.path().to_str().expect("workspace path utf8"),
        ])
        .output()
        .expect("run pi print without prompt");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected bad-input exit code"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout should stay empty for input errors"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing --prompt in print mode"));
}

#[test]
fn print_mode_runtime_error_writes_errors_to_stderr_and_returns_three() {
    let workspace = tempfile::tempdir().expect("tempdir");

    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--mode",
            "print",
            "--provider",
            "openai",
            "--model",
            "gpt-mock",
            "--workspace",
            workspace.path().to_str().expect("workspace path utf8"),
            "-p",
            "hello",
        ])
        .env("PI_OPENAI_URL", "http://127.0.0.1:1")
        .output()
        .expect("run pi print with broken provider");

    assert_eq!(
        output.status.code(),
        Some(3),
        "expected runtime-error exit code"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("provider_error"),
        "expected provider error on stderr"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\"type\":\"error\""),
        "error events must not go to stdout"
    );
}
