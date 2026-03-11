import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    code = f.read()

# Fix usage of PersistedIndex in persist_index that we missed!
code = code.replace("let payload = PersistedIndex {", "let payload = _PersistedIndex {")

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(code)
