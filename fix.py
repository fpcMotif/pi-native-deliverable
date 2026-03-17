import re

with open("src/main.rs", "r") as f:
    content = f.read()

# Fix the partial move
content = content.replace("Some(path) => path,", "Some(ref path) => path.clone(),")

with open("src/main.rs", "w") as f:
    f.write(content)
