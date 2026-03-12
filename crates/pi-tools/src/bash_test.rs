#[cfg(test)]
mod tests {
    use crate::*;
    use serde_json::json;

    #[test]
    fn test_bash_tool_output() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let policy = Policy::safe_defaults(std::env::current_dir()?);
        let call = make_call("bash", json!({"command": "echo 'hello bash'"}));
        let tool = BashTool;
        let res = tool.execute(&call, &policy, std::path::Path::new("."))?;
        println!("result: {:?}", res);
        assert_eq!(res.status.as_str(), "ok");
        assert!(
            res.stdout.contains("hello bash"),
            "stdout was: {:?}",
            res.stdout
        );
        Ok(())
    }

    #[test]
    fn test_bash_tool_timeout() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut policy = Policy::safe_defaults(std::env::current_dir().unwrap());
        policy.command_timeout_ms = 100;
        let call = make_call("bash", json!({"command": "sleep 1"}));
        let tool = BashTool;
        let res = tool.execute(&call, &policy, std::path::Path::new("."))?;
        assert_eq!(res.status.as_str(), "error");
        assert_eq!(res.error, Some("command timed out".to_string()));
        Ok(())
    }

    #[test]
    fn test_is_dangerous_command() {
        let dangerous = vec![
            "rm -rf /",
            "sudo rm -rf /",
            "sudo /bin/rm -rf /",
            "rm -f -r /",
            "rm -r -f /",
            "rm -rf ./dir",
            "sudo rm -rf ./dir",
            "mkfs.ext4 /dev/sda1",
            "sudo mkfs.ext4 /dev/sda1",
            "dd if=/dev/zero of=/dev/sda",
            "sudo dd if=/dev/zero of=/dev/sda",
            "chmod -R 777 /",
            "sudo chmod -R 777 /",
            ":(){ :|:& };:",
            "echo 'hello' > /dev/sda",
            "cat file > /dev/nvme0n1",
            "time rm -rf /",
            "$(rm -rf /)",
            "`rm -rf /`",
            "/usr/bin/rm -rf /",
            "xargs rm -rf /",
            "nohup rm -rf / &",
            "env rm -rf /",
            "echo hello; rm -rf /",
            "echo hello && rm -rf /",
            "echo hello | rm -rf /",
        ];

        for cmd in dangerous {
            assert!(
                is_dangerous_command(cmd),
                "expected '{}' to be detected as dangerous",
                cmd
            );
        }

        let safe = vec![
            "ls -la",
            "echo rm -rf",
            "echo mkfs",
            "cat file.txt",
            "git status",
            "cargo test",
            "rm my-report.txt",
            "rm test-results.json",
            "rm foo-bar.txt",
            "rm --report.txt",
            "rm -f my-report.txt",
            "dd if=in.txt of=out.txt",
        ];

        for cmd in safe {
            assert!(
                !is_dangerous_command(cmd),
                "expected '{}' to be considered safe",
                cmd
            );
        }
    }
}
