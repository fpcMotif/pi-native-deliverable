# The prompt says: "The `lower_line` variable is cleared, but its definition might be inside the loop or reallocated. A better approach is to hoist `lower_line` outside the loop and reuse its capacity."
# BUT wait! `lower_line` IS ALREADY HOISTED!
# Wait, let me look at `collect_fuzzy_spans(line, &query.pattern)`
# Did you notice `let line_lower = line.to_lowercase();` inside `collect_fuzzy_spans`?
# 1047: fn collect_fuzzy_spans(line: &str, pattern: &str) -> Vec<GrepMatchSpan> {
# 1048:     let line_lower = line.to_lowercase();
# 1049:     let pattern_lower = pattern.to_lowercase();
# YES! That allocates TWO STRINGS! And it's called INSIDE the inner loop when there is a match!
# BUT wait, the prompt specifically highlights:
# ```rust
#             for (line_idx, line) in lines.iter().enumerate() {
#                 let line_match_spans = match &matcher {
#                     Matcher::Regex(regex) => collect_match_spans(line, regex),
#                     Matcher::Fuzzy => {
#                         lower_line.clear();
#                         for c in line.chars() {
#                             for lc in c.to_lowercase() {
#                                 lower_line.push(lc);
#                             }
#                         }
# ```
# Wait, look closely at what the prompt says: "The `lower_line` variable is cleared, but its definition might be inside the loop or reallocated. A better approach is to hoist `lower_line` outside the loop and reuse its capacity."
# Is `lower_line` defined inside the loop in the original code, BUT it was hoisted? No, I see it's hoisted at line 571. Wait! Maybe there's ANOTHER `lower_line`? No, I checked `grep`.
# Oh! Could the user mean `lower_line` should be hoisted outside `for entry in index.iter()`?
# Yes! Right now it is:
# ```rust
#         let mut lower_line = String::new();
# ...
#         for entry in index.iter() {
# ```
# Wait, it IS hoisted outside `for entry in index.iter()`!
# `let mut lower_line = String::new();` is at line 571.
# `for entry in index.iter() {` is at line 583.
# So `lower_line` is OUTSIDE both the `index.iter()` loop AND the `lines.iter()` loop.
# Is it reallocated? `clear` retains capacity.
# Wait! Wait! Look at `line.to_lowercase()` in `Matcher::Fuzzy`? No, the code is:
# `for lc in c.to_lowercase() { lower_line.push(lc); }`
# `c.to_lowercase()` returns an iterator!
