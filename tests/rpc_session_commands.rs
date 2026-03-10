use pi_protocol::rpc::ClientRequest;
use pi_protocol::{parse_client_request, protocol_version};

#[test]
fn parse_session_navigation_requests() {
    let select = parse_client_request(&format!(
        r#"{{"v":"{}","type":"select_session_path","id":"1","path":".pi/alt.jsonl"}}"#,
        protocol_version()
    ))
    .expect("parse select_session_path");
    assert!(matches!(
        select,
        ClientRequest::SelectSessionPath { path, .. } if path == ".pi/alt.jsonl"
    ));

    let fork = parse_client_request(&format!(
        r#"{{"v":"{}","type":"fork_session","id":"2","from_turn_id":"abc"}}"#,
        protocol_version()
    ))
    .expect("parse fork_session");
    assert!(matches!(
        fork,
        ClientRequest::ForkSession { from_turn_id, .. } if from_turn_id == "abc"
    ));

    let checkout = parse_client_request(&format!(
        r#"{{"v":"{}","type":"checkout_branch_head","id":"3","from_turn_id":"abc"}}"#,
        protocol_version()
    ))
    .expect("parse checkout_branch_head");
    assert!(matches!(
        checkout,
        ClientRequest::CheckoutBranchHead { from_turn_id: Some(id), .. } if id == "abc"
    ));
}
