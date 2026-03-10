use std::process::Command;

#[test]
fn print_mode_writes_assistant_text_to_stdout_only() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--print",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--prompt",
            "hello from print mode",
        ])
        .output()
        .expect("run pi print");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("Mock response: hello from print mode"));
    assert!(!stdout.contains("\"type\":\"message_update\""));
    assert!(stderr.trim().is_empty(), "stderr should be empty: {stderr}");
}

#[test]
fn print_mode_missing_prompt_returns_invalid_input_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args(["--print", "--provider", "mock", "--model", "mock-tool-call"])
        .output()
        .expect("run pi print missing prompt");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty on invalid input"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing --prompt in print mode"));
}

#[test]
fn print_mode_provider_failures_use_stderr_and_exit_code_20() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--print",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--prompt",
            "please provider_fail now",
        ])
        .output()
        .expect("run pi print provider fail");

    assert_eq!(output.status.code(), Some(20));
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty on provider failure"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("provider error: forced provider failure"));
}

#[test]
fn print_mode_tool_failures_use_stderr_and_exit_code_21() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--print",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
            "--prompt",
            "please tool_fail now",
        ])
        .output()
        .expect("run pi print tool fail");

    assert_eq!(output.status.code(), Some(21));
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty on tool failure"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tool error:"));
}
