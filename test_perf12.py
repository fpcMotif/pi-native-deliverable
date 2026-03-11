import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    code = f.read()

replacement = r"""
                    Matcher::Fuzzy => {
                        let line_len = line.len();
                        let lower_len = lower.len();
                        let diff = line_len.abs_diff(lower_len);

                        // Normalized Levenshtein is 1.0 - (dist / max)
                        // This means `dist <= 0.28 * max` is required for >= 0.72.
                        // `dist` is at least the length difference.
                        // We use byte lengths here as a fast early-out.
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

                            // If lengths are exactly the same and strings contain identical chars
                            // but order might be slightly off.
                            // But usually, if it doesn't contain a huge chunk of the characters,
                            // we could filter it even more.

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
    r"Matcher::Fuzzy => \{\s+let line_len.*Vec::new\(\)\s+\}\s+\}\s+\}",
    replacement,
    code,
    flags=re.MULTILINE | re.DOTALL
)

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(new_code)
