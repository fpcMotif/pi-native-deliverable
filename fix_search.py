with open("crates/pi-search/src/lib.rs", "r") as f:
    content = f.read()
content = content.replace("#[cfg(test)]\nmod tests {", "#[cfg(test)]\n#[allow(clippy::items_after_test_module)]\nmod tests {")
with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(content)
