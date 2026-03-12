use criterion::{criterion_group, criterion_main, Criterion};
use pi_session::SessionStore;
use tokio::runtime::Runtime;

fn bench_session_load(c: &mut Criterion) {
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let path = "test_session_bench.jsonl";

    // Setup
    rt.block_on(async {
        let _ = std::fs::remove_file(path);
        let mut store = SessionStore::new(path)
            .await
            .expect("Failed to create SessionStore");
        for _ in 0..10000 {
            store
                .append(pi_protocol::session::SessionEntryKind::UserMessage {
                    text: "hello world ".repeat(20).to_string(),
                })
                .await
                .unwrap();
        }
    });

    c.bench_function("session load 10000 entries", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _store = SessionStore::new(path)
                    .await
                    .expect("Failed to load SessionStore");
            });
        });
    });

    // Teardown
    std::fs::remove_file(path).unwrap_or(());
}

criterion_group!(benches, bench_session_load);
criterion_main!(benches);
