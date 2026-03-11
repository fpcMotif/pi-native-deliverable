with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace("scope.is_none_or(|scope| scope_is_prefix(&entry.relative_path, scope))",
                          "scope.map_or(true, |scope| scope_is_prefix(&entry.relative_path, scope))")

content = content.replace(".is_none_or(|ext| entry.relative_path.ends_with(&format!(\".{ext}\")))",
                          ".map_or(true, |ext| entry.relative_path.ends_with(&format!(\".{ext}\")))")

content = content.replace(".is_none_or(|prefix| scope_is_prefix(&entry.relative_path, prefix))",
                          ".map_or(true, |prefix| scope_is_prefix(&entry.relative_path, prefix))")

with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
print("Replaced is_none_or")
