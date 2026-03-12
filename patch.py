import re

with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

target = """            let bytes =
                match tokio::task::spawn_blocking(move || std::fs::read(&resolved_path)).await {
                    Ok(Ok(value)) => value,
                    _ => continue,
                };"""

replacement = """            let bytes = match tokio::fs::read(&resolved_path).await {
                Ok(value) => value,
                Err(_) => continue,
            };"""

if target in content:
    with open('crates/pi-search/src/lib.rs', 'w') as f:
        f.write(content.replace(target, replacement))
    print("Replaced successfully")
else:
    print("Target not found")
