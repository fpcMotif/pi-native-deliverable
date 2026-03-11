import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    code = f.read()

replacement = r"""
                    Matcher::Fuzzy => {
                        let line_char_count = line.chars().count();
                        let lower_char_count = lower.chars().count();
                        let max_chars = line_char_count.max(lower_char_count);
                        let diff = line_char_count.abs_diff(lower_char_count);

                        if (diff as f64) > 0.28 * (max_chars as f64) {
                            Vec::new()
                        } else {
                            lower_line.clear();
                            for c in line.chars() {
                                for lc in c.to_lowercase() {
                                    lower_line.push(lc);
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
    flags=re.MULTILINE
)

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(new_code)
