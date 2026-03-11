with open("crates/pi-search/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("if let Err(err) = service.save_index().await {", "if let Err(_err) = service.save_index().await {")

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(content)
