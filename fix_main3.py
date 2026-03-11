import re

with open('src/main.rs', 'r') as f:
    content = f.read()

content = content.replace(
'''            "{}",
            "{\\"error\\":\\"protocol-schema feature is disabled\\"}"
        );''',
'''            "\\"{{\\\\\\"error\\\\\\":\\\\\\"protocol-schema feature is disabled\\\\\\"}}\\""
        );'''
)

with open('src/main.rs', 'w') as f:
    f.write(content)
