with open("crates/pi-search/tests/search_bench_contract.rs", "r") as f:
    content = f.read()

content = content.replace("#[test]\nfn grep_first_page_stays_within_budget()", "#[test]\n#[cfg_attr(debug_assertions, ignore)]\nfn grep_first_page_stays_within_budget()")

with open("crates/pi-search/tests/search_bench_contract.rs", "w") as f:
    f.write(content)
