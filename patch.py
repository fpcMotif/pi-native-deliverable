import re

with open("crates/pi-protocol/src/rpc.rs", "r") as f:
    content = f.read()

content = re.sub(
    r"""    fn test_to_jsonl_value_message_update_event\(\) -> Result<\(\), serde_json::Error> \{
        let event = ServerEvent::MessageUpdate \{
            v: "1\.0\.0"\.to_string\(\),
            id: Some\("req-101"\.to_string\(\)\),
            request_id: Some\("req-101"\.to_string\(\)\),
            delta: "Hello"\.to_string\(\),
            done: false,
        \};

        let result = to_jsonl_value\(&event\);
        let parsed: Value = serde_json::from_str\(&result\)\?;

        assert_eq!\(parsed\["type"\], "message_update"\);
        assert_eq!\(parsed\["v"\], PROTOCOL_VERSION\);
        Ok\(\(\)\)
    \}""",
    r"""    fn test_to_jsonl_value_message_update_event() -> Result<(), serde_json::Error> {
        let event = ServerEvent::MessageUpdate {
            v: "1.0.0".to_string(),
            id: Some("req-101".to_string()),
            request_id: Some("req-101".to_string()),
            delta: "Hello".to_string(),
            done: false,
        };

        let result = to_jsonl_value(&event);
        let parsed: Value = serde_json::from_str(&result).expect("parse result");

        assert_eq!(parsed["type"], "message_update");
        assert_eq!(parsed["v"], PROTOCOL_VERSION);
        Ok(())
    }""",
    content
)

with open("crates/pi-protocol/src/rpc.rs", "w") as f:
    f.write(content)
