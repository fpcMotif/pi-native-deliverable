# If the author of the prompt says: "The `lower_line` variable is cleared, but its definition might be inside the loop or reallocated. A better approach is to hoist `lower_line` outside the loop and reuse its capacity."
# And my codebase already has `let mut lower_line = String::new();` outside the loop.
# Is it possible that `lower_line` could be hoisted further? No, it's outside the outer `entry in index.iter()` loop!
# Wait! Let's check the code block again:
# ```
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
# What is wrong with THIS EXACT block?
# If `lower_line` IS hoisted, why does the prompt say "its definition MIGHT be inside the loop or reallocated"?
# Oh! The prompt says "A better approach is to hoist `lower_line` outside the loop and reuse its capacity."
# This could be an AI generated prompt from some generic linter or analysis tool that incorrectly thinks it isn't hoisted.
# Or wait! Are there ANY OTHER loops?
# Let's look at `collect_fuzzy_spans(line, &query.pattern)`
