import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    code = f.read()

replacement = r"""
                    Matcher::Fuzzy => {
                        let line_len = line.len();
                        let lower_len = lower.len();

                        // Fast path byte length difference check.
                        // While utf-8 chars can be multi-byte, a byte length difference
                        // is an incredibly fast heuristic to skip most lines.
                        // If byte lengths are vastly different, it's highly likely character lengths are too.
                        // However, we should be slightly more permissive since byte length != char length.
                        // But since length diff is exact for ASCII, and lower bound for chars in most cases...
                        // Actually, calculating `chars().count()` is O(N).
                        // Let's just do `line.is_ascii()`.

                        lower_line.clear();
                        if line.is_ascii() {
                            let diff = line_len.abs_diff(lower_len);
                            if (diff as f64) > 0.28 * (line_len.max(lower_len) as f64) {
                                // Skip
                            } else {
                                lower_line.push_str(line);
                                lower_line.make_ascii_lowercase();
                                if normalized_levenshtein(&lower_line, &lower) >= 0.72 {
                                    return collect_fuzzy_spans(line, &query.pattern);
                                }
                            }
                        } else {
                            for c in line.chars() {
                                for lc in c.to_lowercase() {
                                    lower_line.push(lc);
                                }
                            }
                            let diff = lower_line.chars().count().abs_diff(lower.chars().count());
                            if (diff as f64) <= 0.28 * (lower_line.chars().count().max(lower.chars().count()) as f64) {
                                if normalized_levenshtein(&lower_line, &lower) >= 0.72 {
                                    return collect_fuzzy_spans(line, &query.pattern);
                                }
                            }
                        }
                        Vec::new()
                    }
"""

# Wait, `collect_fuzzy_spans` shouldn't be returned directly, the outer block expects Vec.
replacement = r"""
                    Matcher::Fuzzy => {
                        let line_len = line.len();
                        let lower_len = lower.len();

                        lower_line.clear();
                        if line.is_ascii() {
                            let diff = line_len.abs_diff(lower_len);
                            if (diff as f64) > 0.28 * (line_len.max(lower_len) as f64) {
                                Vec::new()
                            } else {
                                lower_line.push_str(line);
                                lower_line.make_ascii_lowercase();
                                if normalized_levenshtein(&lower_line, &lower) >= 0.72 {
                                    collect_fuzzy_spans(line, &query.pattern)
                                } else {
                                    Vec::new()
                                }
                            }
                        } else {
                            let line_char_count = line.chars().count();
                            let lower_char_count = lower.chars().count();
                            let max_chars = line_char_count.max(lower_char_count);
                            let diff = line_char_count.abs_diff(lower_char_count);
                            if (diff as f64) > 0.28 * (max_chars as f64) {
                                Vec::new()
                            } else {
                                for c in line.chars() {
                                    for lc in c.to_lowercase() {
                                        lower_line.push(lc);
                                    }
                                }
                                if normalized_levenshtein(&lower_line, &lower) >= 0.72 {
                                    collect_fuzzy_spans(line, &query.pattern)
                                } else {
                                    Vec::new()
                                }
                            }
                        }
                    }
"""

new_code = re.sub(
    r"Matcher::Fuzzy => \{\s+let line_char_count.*Vec::new\(\)\s+\}\s+\}\s+\}",
    replacement,
    code,
    flags=re.MULTILINE | re.DOTALL
)

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(new_code)
