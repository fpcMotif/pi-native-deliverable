import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    code = f.read()

replacement = r"""
                    Matcher::Fuzzy => {
                        let line_char_count = line.chars().count();
                        let lower_char_count = lower.chars().count();

                        if line_char_count.abs_diff(lower_char_count) as f64 > 0.28 * (line_char_count.max(lower_char_count) as f64) {
                            Vec::new()
                        } else {
                            lower_line.clear();
                            // If line is ascii, we can do this much faster
                            if line.is_ascii() {
                                lower_line.push_str(line);
                                lower_line.make_ascii_lowercase();
                            } else {
                                for c in line.chars() {
                                    for lc in c.to_lowercase() {
                                        lower_line.push(lc);
                                    }
                                }
                            }
                            let line_match = normalized_levenshtein(&lower_line, &lower) >= 0.72;
                            if line_match {
                                collect_fuzzy_spans(line, &query.pattern)
                            } else {
                                Vec::new()
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
