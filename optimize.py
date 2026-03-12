import re

with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

# 1. Update IndexedFile struct
content = re.sub(r'struct IndexedFile \{\s*relative_path: String,', 'struct IndexedFile {\n    relative_path: String,\n    relative_path_lc: String,', content, count=1)

# 2. Update SearchItem struct
content = re.sub(r'pub struct SearchItem \{\s*pub relative_path: String,', 'pub struct SearchItem {\n    pub relative_path: String,\n    pub relative_path_lc: String,', content, count=1)

# 3. Update instances of IndexedFile construction
content = re.sub(
    r'items\.push\(IndexedFile \{\s*relative_path: relative,',
    'items.push(IndexedFile {\n                    relative_path: relative.clone(),\n                    relative_path_lc: relative.to_lowercase(),',
    content, count=1
)

content = re.sub(
    r'index\.push\(IndexedFile \{\s*relative_path: relative,',
    'index.push(IndexedFile {\n                        relative_path: relative.clone(),\n                        relative_path_lc: relative.to_lowercase(),',
    content, count=1
)

# 4. Refactor score_path_match
pattern = r"""fn score_path_match\(path: &str, query: &str\) -> f64 \{
    if query\.is_empty\(\) \{
        return 0\.0;
    \}
    let path_lc = path\.to_lowercase\(\);
    if path_lc == query \{
        return 1\.0;
    \}
    if path_lc\.contains\(query\) \{
        return 0\.9;
    \}
    normalized_levenshtein\(&path_lc, query\)
\}"""

replacement = """fn score_path_match(path_lc: &str, query: &str) -> f64 {
    if query.is_empty() {
        return 0.0;
    }
    if path_lc == query {
        return 1.0;
    }
    if path_lc.contains(query) {
        return 0.9;
    }
    normalized_levenshtein(path_lc, query)
}"""

content = re.sub(pattern, replacement, content, count=1)

# 5. Update find_files call site
content = content.replace(
    "let base = score_path_match(&entry.relative_path, &needle);",
    "let base = score_path_match(&entry.relative_path_lc, &needle);", 1
)

# 6. Update SearchItem construction
content = re.sub(
    r'matched\.push\(SearchItem \{\s*relative_path: entry\.relative_path\.clone\(\),',
    'matched.push(SearchItem {\n                relative_path: entry.relative_path.clone(),\n                relative_path_lc: entry.relative_path_lc.clone(),',
    content, count=1
)

with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
