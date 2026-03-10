use pi_session::SessionStore;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let mut store = SessionStore::new("test_session.jsonl").await.unwrap();
    // populate
    for _i in 0..10000 {
        store
            .append(pi_protocol::session::SessionEntryKind::UserMessage {
                text: "hello world ".repeat(10).to_string(),
            })
            .await
            .unwrap();
    }

    let start = Instant::now();
    for _ in 0..10 {
        store.compact(None).await.unwrap();
    }
    let duration = start.elapsed();
    println!("Time: {:?}", duration);
    std::fs::remove_file("test_session.jsonl").unwrap();
    std::fs::remove_file("test_session.compact.jsonl").unwrap();
}
