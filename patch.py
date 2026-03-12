with open("crates/pi-search/src/lib.rs", "r") as f:
    text = f.read()

new_text = text.replace(
    """fn collect_fuzzy_spans(line: &str, pattern: &str) -> Vec<GrepMatchSpan> {
    let line_lower = line.to_lowercase();
    let pattern_lower = pattern.to_lowercase();
    line_lower
        .find(&pattern_lower)""",
    """fn collect_fuzzy_spans(line: &str, line_lower: &str, pattern_lower: &str) -> Vec<GrepMatchSpan> {
    line_lower
        .find(pattern_lower)"""
)

new_text = new_text.replace(
    """                            let line_match = normalized_levenshtein(&lower_line, &lower) >= 0.72;
                            if line_match {
                                collect_fuzzy_spans(line, &query.pattern)
                            } else {""",
    """                            let line_match = normalized_levenshtein(&lower_line, &lower) >= 0.72;
                            if line_match {
                                collect_fuzzy_spans(line, &lower_line, &lower)
                            } else {"""
)

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(new_text)
