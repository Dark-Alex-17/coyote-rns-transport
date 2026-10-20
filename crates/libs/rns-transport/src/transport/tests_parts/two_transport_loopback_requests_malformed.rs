// Loopback coverage for a Request packet whose decrypted body is not a request frame. Python
// unpacks the body inside the packet handler's `try` (`RNS/Link.py:1003-1007`) and a frame
// of the wrong shape fails inside `handle_request` (`RNS/Link.py:804-807`); either way the
// error is logged, no handler runs, nothing is answered, and the link keeps working. The
// responder here must do the same: the broadcast `LinkEvent::Data(Request)` still posts for
// the packet, no Response packet leaves, the registered handler never runs, and the next
// well-formed request on the same link is answered. Helpers come from the sibling
// `tests_parts` files.

/// Bodies that are not a `[requested_at, path_hash, data]` frame: bytes msgpack never
/// produces (0xc1 is the reserved marker), a scalar instead of an array, an array whose
/// path hash is a string, and an array whose path hash has the wrong length.
fn undecodable_request_bodies() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("reserved msgpack marker", vec![0xc1, 0xc1, 0xc1, 0xc1]),
        ("scalar instead of a frame", pack_value(rmpv::Value::from(7))),
        (
            "string path hash",
            pack_value(rmpv::Value::Array(vec![
                rmpv::Value::F64(0.0),
                rmpv::Value::from("/echo"),
                rmpv::Value::Nil,
            ])),
        ),
        (
            "short path hash",
            pack_value(rmpv::Value::Array(vec![
                rmpv::Value::F64(0.0),
                rmpv::Value::Binary(vec![0x01, 0x02, 0x03]),
                rmpv::Value::Nil,
            ])),
        ),
    ]
}

fn pack_value(value: rmpv::Value) -> Vec<u8> {
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, &value).expect("value packs");
    packed
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_undecodable_body_is_ignored_and_link_still_answers() {
    let mut pair = loopback_request_pair("request-malformed").await;
    let calls = Arc::new(AtomicUsize::new(0));
    register_handler(&pair, "/echo", RequestAllow::All, counting_echo_handler(calls.clone())).await;

    for (label, body) in undecodable_request_bodies() {
        let id = send_packed_request(&pair, &body).await;
        // The broadcast contract does not depend on the body decoding: the Data event names
        // the packet by its id and carries the Request context.
        timeout(RESPONSE_TIMEOUT, await_request_data_event(&mut pair, id))
            .await
            .unwrap_or_else(|_| panic!("Data(Request) event posts for the {label} body"));
    }

    assert_no_response(&mut pair).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0, "no handler ran for an undecodable body");

    let id = send_request(&pair, "/echo", rmpv::Value::from("after malformed")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("after malformed"));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the well-formed request ran once");

    timeout(STOP_TIMEOUT, pair.handles.listener.transport.stop())
        .await
        .expect("listener stops after undecodable requests");
    timeout(STOP_TIMEOUT, pair.handles.connector.transport.stop()).await.expect("connector stops");
}
