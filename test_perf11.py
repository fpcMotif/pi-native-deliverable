import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    code = f.read()

replacement = r"""
                    Matcher::Fuzzy => {
                        let line_len = line.len();
                        let lower_len = lower.len();
                        let diff = line_len.abs_diff(lower_len);

                        // We do a fast byte-length check first to skip obvious mismatches.
                        // While utf-8 chars can be multi-byte, byte length difference > limit
                        // means it's extremely unlikely to be a good fuzzy match.
                        if (diff as f64) > 0.28 * (line_len.max(lower_len) as f64) {
                            Vec::new()
                        } else {
                            lower_line.clear();
                            // If line is ascii, we can do this without unicode char boundaries.
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
    r"Matcher::Fuzzy => \{\s+lower_line\.clear\(\);\s+for c in line\.chars\(\) \{\s+for lc in c\.to_lowercase\(\) \{\s+lower_line\.push\(lc\);\s+\}\s+\}\s+let line_match = normalized_levenshtein\(&lower_line, &lower\) >= 0\.72;\s+if line_match \{\s+collect_fuzzy_spans\(line, &query\.pattern\)\s+\} else \{\s+Vec::new\(\)\s+\}\s+\}",
    replacement,
    code,
    flags=re.MULTILINE | re.DOTALL
)

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(new_code)
