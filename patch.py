with open("crates/pi-search/src/lib.rs", "r") as f:
    content = f.read()

s1 = """
            stats.scanned_files += 1;
            let text = String::from_utf8_lossy(&bytes);

            let mut file_matched = false;
            let mut byte_offset = 0usize;

            for (line_idx, line) in text.lines().enumerate() {
"""
r1 = """
            stats.scanned_files += 1;
            let text = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = text.lines().collect();

            let mut file_matched = false;
            let mut byte_offset = 0usize;

            for (line_idx, line) in lines.iter().enumerate() {
"""

content = content.replace(s1[1:], r1[1:])

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(content)
