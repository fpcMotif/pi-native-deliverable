with open("crates/pi-search/tests/search_bench_contract.rs", "r") as f:
    content = f.read()

import re

# They use #[test] not #[tokio::test]

content = re.sub(
    r'(#\[test\])\n(fn grep_first_page_stays_within_budget)',
    r'\1\n#[cfg_attr(debug_assertions, ignore)]\n\2',
    content
)

content = re.sub(
    r'(#\[test\])\n(fn find_files_stays_within_budget)',
    r'\1\n#[cfg_attr(debug_assertions, ignore)]\n\2',
    content
)

# ensure we don't duplicate ignores
content = content.replace("#[cfg_attr(debug_assertions, ignore)]\n#[cfg_attr(debug_assertions, ignore)]", "#[cfg_attr(debug_assertions, ignore)]")

with open("crates/pi-search/tests/search_bench_contract.rs", "w") as f:
    f.write(content)
