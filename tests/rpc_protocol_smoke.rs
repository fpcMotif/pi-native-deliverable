use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Black-box RPC smoke test.
/// Requires the `pi` binary to support `--mode rpc` and line-delimited JSON.
///
/// NOTE: This test is intentionally minimal and should not rely on any specific provider.
/// It should run against the built-in mock provider (feature: `mock-llm`) in CI.
#[test]
fn rpc_smoke_prompt_and_events() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--mode",
            "rpc",
            "--provider",
            "mock",
            "--model",
            "mock-tool-call",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn pi rpc");

    let stdin = child.stdin.as_mut().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");

    writeln!(
        stdin,
        r#"{{"v":"1.0.0","type":"prompt","id":"req-1","message":"List files in current directory using tools."}}"#
    )
    .expect("write prompt");

    let mut reader = BufReader::new(stdout);
    let mut saw_ready = false;
    let mut saw_message_update = false;

    let timeout = Duration::from_secs(10);
    let deadline = Instant::now() + timeout;

    // Read lines until we see expected events or timeout
    while Instant::now() < deadline {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains(r#""type":"ready""#) {
            saw_ready = true;
        }
        if line.contains(r#""type":"message_update""#) {
            saw_message_update = true;
        }
        if saw_ready && saw_message_update {
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(saw_ready, "expected a ready event within {timeout:?}");
    assert!(
        saw_message_update,
        "expected at least one message_update event within {timeout:?}"
    );
}
