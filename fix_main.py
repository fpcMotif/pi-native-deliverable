with open('src/main.rs', 'r') as f:
    content = f.read()

content = content.replace('"{{\\"error\\":\\"protocol-schema feature is disabled\\"}}"\n        serde_json::json!({',
                          '"{}",\n        serde_json::json!({')

with open('src/main.rs', 'w') as f:
    f.write(content)

print("Fixed main.rs")
