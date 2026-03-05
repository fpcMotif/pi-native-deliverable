use std::fs;

#[test]
fn grep_modes_plain_vs_regex_and_highlights() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("a.txt");
    fs::write(&path, "line abc\nABC abc\nfoo abc bar\n").expect("write");

    let grep_limit = 50;

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let svc = pi_search::SearchService::new(pi_search::SearchServiceConfig {
            workspace_root: tmp.path().to_path_buf(),
            ..Default::default()
        })
        .await
        .unwrap();

        let plain = svc
            .grep("ABC", pi_search::GrepMode::PlainText, ".", grep_limit)
            .await
            .unwrap();
        assert!(
            plain.matches.len() >= 1,
            "PlainText mode should find case-insensitive ABC"
        );
        assert!(
            plain.matches.iter().all(|item| !item.highlights.is_empty()),
            "PlainText results should include match highlight spans"
        );

        let first = &plain.matches[0];
        assert!(first.highlights[0].start < first.highlights[0].end);

        let regex = svc
            .grep("A.C", pi_search::GrepMode::Regex, ".", grep_limit)
            .await
            .unwrap();
        assert!(
            regex.matches.len() >= 1,
            "Regex mode should resolve A.C to ABC"
        );
        assert!(
            regex.matches.iter().any(|item| !item.highlights.is_empty()),
            "Regex results should include match highlight spans"
        );
    });
}

#[test]
fn grep_pagination_is_stable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut fixture = String::new();
    for index in 1..=12 {
        fixture.push_str(&format!("line {index}: this is Needle value\n"));
    }
    let path = tmp.path().join("needles.txt");
    fs::write(&path, fixture).expect("write");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let svc = pi_search::SearchService::new(pi_search::SearchServiceConfig {
            workspace_root: tmp.path().to_path_buf(),
            ..Default::default()
        })
        .await
        .unwrap();

        let first = svc
            .grep_query(pi_search::GrepQuery {
                pattern: "Needle".to_string(),
                mode: pi_search::GrepMode::PlainText,
                scope: ".".to_string(),
                limit: 5,
                token: None,
                offset: 0,
            })
            .await
            .unwrap();

        assert_eq!(first.matches.len(), 5);
        assert!(
            first.token.is_some(),
            "expected pagination token on partial page"
        );

        let second = svc
            .grep_query(pi_search::GrepQuery {
                pattern: "Needle".to_string(),
                mode: pi_search::GrepMode::PlainText,
                scope: ".".to_string(),
                limit: 5,
                token: first.token,
                offset: 0,
            })
            .await
            .unwrap();

        assert_eq!(second.matches.len(), 5);
        assert_ne!(
            first.matches[4].line, second.matches[0].line,
            "paginated results should not overlap"
        );
        assert!(
            second.token.is_some(),
            "expected another pagination token while more matches exist"
        );

        let third = svc
            .grep_query(pi_search::GrepQuery {
                pattern: "Needle".to_string(),
                mode: pi_search::GrepMode::PlainText,
                scope: ".".to_string(),
                limit: 5,
                token: second.token,
                offset: 0,
            })
            .await
            .unwrap();

        assert_eq!(third.matches.len(), 2);
        assert!(
            third.token.is_none(),
            "no additional pagination token expected on final page"
        );
        assert!(third.matches[0].line_number > second.matches[4].line_number);
    });
}

#[test]
fn grep_regex_invalid_pattern_falls_back_to_plaintext() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("regex-error.txt");
    fs::write(
        &path,
        "literal [bracket test\nplain text search still works\n",
    )
    .expect("write");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let svc = pi_search::SearchService::new(pi_search::SearchServiceConfig {
            workspace_root: tmp.path().to_path_buf(),
            ..Default::default()
        })
        .await
        .unwrap();

        let result = svc
            .grep("[", pi_search::GrepMode::Regex, ".", 50)
            .await
            .unwrap();

        assert!(
            result.warning.is_some(),
            "expected readable warning when invalid regex falls back"
        );
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].line, "literal [bracket test");
        assert!(
            result
                .matches
                .iter()
                .flat_map(|item| &item.highlights)
                .any(|span| span.start < span.end),
            "fallback matches should include highlights"
        );
    });
}
