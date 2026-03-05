use pi_tools::{BashTool, Policy, make_call, Tool};
use serde_json::json;

fn main() {
    let policy = Policy::default();
    let call = make_call("bash", json!({"command": "echo hello"}));
    let bash = BashTool;
    let result = bash.execute(&call, &policy, std::path::Path::new(".")).unwrap();
    println!("result: {:?}", result);
}
