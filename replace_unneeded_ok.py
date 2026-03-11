with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

import re
old_ok = """    Ok(value
        .try_into()
        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))?)"""

new_ok = """    value
        .try_into()
        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))"""

content = content.replace(old_ok, new_ok)

with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
print("Replaced unneeded Ok")
