// The shared-instance RPC client's failure kinds under contention, on the
// rnsd fixture of `python_shared_instance_rpc.rs`.
//
// rnsd serves RPC dials serially on `rpc_loop` (`RNS/Reticulum.py:1274-1334`)
// behind a `Listener` with `backlog=1` (`RNS/Reticulum.py:364`). A dial that
// lands while another is being served is queued, dropped or reset depending
// on the kernel, so three concurrent dials make the overflow likely. What a
// caller must see from an overflowed dial is never `Protocol` (the bytes it
// did read were fine) but a retryable kind (`ConnectionReset`,
// `ConnectionAborted`, `BrokenPipe`, `UnexpectedEof`), which `io_kind()`
// exposes, or `Timeout`. Where the reset lands depends on the kernel: a
// reset once `connect()` has returned is `Io`, while macOS resets the
// overflowed dial before `connect()` completes, so there it is
// `Connect` with the `ConnectionReset` kind; `io_kind()` reads both, which
// is why callers match on the kind and not the variant. A refused dial
// (`ConnectionRefused`) is still a listener that is not there and fails the
// round. A dropped SYN is retransmitted by the kernel and shows up as a slow
// `Ok` or a `Timeout`, so a round with no failure at all is a pass: the race
// is opportunistic.

const SHARED_INSTANCE_RPC_IO_ROUNDS: usize = 8;

/// `Timeout`, or the variant and kind of a retryable failure; `None` for
/// anything a caller must not retry.
fn shared_instance_rpc_io_failure_kind(err: &RpcError) -> Option<String> {
    match (err, err.io_kind()) {
        (RpcError::Timeout, _) => Some("Timeout".to_string()),
        (RpcError::Connect(_), Some(kind)) if shared_instance_rpc_retryable(err) => {
            Some(format!("Connect({kind:?})"))
        }
        (RpcError::Io(_), Some(kind)) if shared_instance_rpc_retryable(err) => {
            Some(format!("Io({kind:?})"))
        }
        _ => None,
    }
}

/// Three concurrent `interface_stats` dials per round, eight rounds: every
/// round gets at least one reply, every reply carries the fixture's entries, and
/// every failure is `Timeout`, or `Io` or `Connect` whose `io_kind()` is a
/// retryable kind.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_rpc_concurrent_dials_fail_only_with_retryable_io_kinds() {
    let _interop_guard = python_interop_guard().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &live_tcp_shared_instance(),
        control_port,
        &temp.path().join("python-rnsd-shared-instance-rpc-io-rnsd"),
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;

    // The listener is bound once this answers, so no later refusal is a
    // not-yet-bound listener.
    rnsd.poll_interface_stats(&endpoint, &key).await.expect("the first interface_stats reply");

    let mut failure_kinds = Vec::new();
    for round in 1..=SHARED_INSTANCE_RPC_IO_ROUNDS {
        let results = tokio::join!(
            interface_stats(&endpoint, &key, SHARED_INSTANCE_RPC_CALL),
            interface_stats(&endpoint, &key, SHARED_INSTANCE_RPC_CALL),
            interface_stats(&endpoint, &key, SHARED_INSTANCE_RPC_CALL),
        );
        let results = [results.0, results.1, results.2];
        if results.iter().all(Result::is_err) {
            rnsd.assert_running(&format!("round {round}: no dial was answered: {results:?}"));
            panic!("round {round}: no dial was answered: {results:?}");
        }
        for stats in results.iter().filter_map(|result| result.as_ref().ok()) {
            assert_shared_instance_rpc_entries(stats);
        }
        for err in results.iter().filter_map(|result| result.as_ref().err()) {
            match shared_instance_rpc_io_failure_kind(err) {
                Some(kind) => failure_kinds.push(kind),
                None => {
                    rnsd.assert_running(&format!("round {round}: {err:?}"));
                    panic!("round {round}: not a retryable failure: {err:?}");
                }
            }
        }
    }
    eprintln!(
        "rnsd concurrent dials: rounds={SHARED_INSTANCE_RPC_IO_ROUNDS} failures={} kinds={:?}",
        failure_kinds.len(),
        failure_kinds
    );
}
