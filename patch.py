import re

content = open("tests/search_fuzzy_ordering.rs").read()

content = content.replace("async async fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus()", "async fn fuzzy_scoring_prefers_entrypoints_and_filename_bonus()")

with open("tests/search_fuzzy_ordering.rs", "w") as f:
    f.write(content)
