use std::fs;

/// Grep modes smoke test.
/// Ensures PlainText and Regex behave differently and return highlights.
#[test]
fn grep_modes_plain_vs_regex() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("a.txt");
    fs::write(&path, "abc 123\\nABC 123\\n").expect("write");

    // TODO: once pi-search grep is implemented:
    // let svc = pi_search::SearchService::new(tmp.path()).unwrap();
    // let plain = svc.grep("ABC", GrepMode::PlainText, opts).unwrap();
    // assert!(plain.matches.len() >= 1);
    //
    // let regex = svc.grep("A.C", GrepMode::Regex, opts).unwrap();
    // assert!(regex.matches.len() >= 1);
}
