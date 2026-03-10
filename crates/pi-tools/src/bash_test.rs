#[cfg(test)]
mod tests {
    use crate::*;
    use serde_json::json;

    #[test]
    fn test_bash_tool_output() {
        let policy = Policy::safe_defaults(std::env::current_dir().unwrap());
        let call = make_call("bash", json!({"command": "echo 'hello bash'"}));
        let tool = BashTool;
        let res = tool.execute(&call, &policy, std::path::Path::new(".")).unwrap();
        println!("result: {:?}", res);
        assert_eq!(res.status.as_str(), "ok");
        assert!(res.stdout.contains("hello bash"), "stdout was: {:?}", res.stdout);
    }

    #[test]
    fn test_bash_tool_timeout() {
        let mut policy = Policy::safe_defaults(std::env::current_dir().unwrap());
        policy.command_timeout_ms = 100;
        let call = make_call("bash", json!({"command": "sleep 1"}));
        let tool = BashTool;
        let res = tool.execute(&call, &policy, std::path::Path::new(".")).unwrap();
        assert_eq!(res.status.as_str(), "error");
        assert_eq!(res.error, Some("command timed out".to_string()));
    }
}
