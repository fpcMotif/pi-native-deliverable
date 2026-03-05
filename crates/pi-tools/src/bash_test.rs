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
}
