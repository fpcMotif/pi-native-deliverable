use criterion::{criterion_group, criterion_main, Criterion};
use pi_session::SessionStore;
use tokio::runtime::Runtime;

fn bench_session_load(c: &mut Criterion) {
    let rt = Runtime::new().expect("benchmark error");
    let path = "test_session_bench.jsonl";

    // Setup
    rt.block_on(async {
        let mut store = SessionStore::new(path).await.expect("benchmark error");
        for _ in 0..10000 {
            store
                .append(pi_protocol::session::SessionEntryKind::UserMessage {
                    text: "hello world ".repeat(20).to_string(),
                })
                .await
                .expect("benchmark error");
        }
    });

    c.bench_function("session load 10000 entries", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _store = SessionStore::new(path).await.expect("benchmark error");
            });
        });
    });

    // Teardown
    std::fs::remove_file(path).expect("benchmark error");
}

criterion_group!(benches, bench_session_load);
criterion_main!(benches);
