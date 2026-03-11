import sys
content = open("crates/pi-search/src/lib.rs").read()
content = content.replace("mod tests {", "#[allow(clippy::items_after_test_module)]\nmod tests {")
open("crates/pi-search/src/lib.rs", "w").write(content)
