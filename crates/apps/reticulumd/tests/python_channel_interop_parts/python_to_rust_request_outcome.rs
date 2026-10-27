// `InboundRequest::track_outcome` against a Python RNS 1.5.2 requester: a
// reply above the ~16 KB TCP link MDU leaves as a response Resource and
// resolves `Resource(Complete)` once Python proves it; a short reply resolves
// `Packet`.
//
// Reuses `RequestHandlerFixture` / `RequestHandlerRecord` and the
// `request_forms_matrix_*` helpers from the sibling include!d files.

use rns_transport::destination::request::{ResponseOutcome, ResponseOutcomeReceiver};
use rns_transport::resource::OutboundResult;

/// `RequestHandlerRecord` whose handler also keeps the receiver from
/// `track_outcome`.
#[derive(Default)]
struct TrackedHandlerRecord {
    record: RequestHandlerRecord,
    receiver: Arc<StdMutex<Option<ResponseOutcomeReceiver>>>,
}

impl TrackedHandlerRecord {
    fn handler(
        &self,
        respond: fn(&rmpv::Value) -> Option<RequestResponse>,
    ) -> impl RequestHandler + 'static {
        let seen = self.record.seen.clone();
        let calls = self.record.calls.clone();
        let receiver = self.receiver.clone();
        move |mut request: InboundRequest| {
            calls.fetch_add(1, Ordering::SeqCst);
            *receiver.lock().expect("receiver lock") = Some(request.track_outcome());
            let response = respond(request.data());
            let request_id = request.request_id();
            let remote = request.remote_identity().map(|identity| identity.address_hash);
            seen.lock().expect("seen lock").push((request_id, remote, request.into_data()));
            std::future::ready(response)
        }
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn outcome(&self) -> ResponseOutcome {
        let receiver = self
            .receiver
            .lock()
            .expect("receiver lock")
            .take()
            .expect("handler tracked the outcome");
        tokio::time::timeout(Duration::from_secs(10), receiver).await.expect("outcome resolves")
    }
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_tracked_resource_response_resolves_complete() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_tracked_resource_response_resolves_complete().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_tracked_resource_response_resolves_complete() {
    let fixture = RequestHandlerFixture::start("python-rns-request-outcome-resource").await;
    let tracked = TrackedHandlerRecord::default();
    fixture.register(tracked.handler(request_forms_matrix_large_bytes), RequestAllow::All).await;

    let mut client = fixture.connect(30.0, &[]).await;
    let lines = client.json_lines().await;

    let (request_id, _, data) = tracked.record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(
        &lines,
        request_id,
        &request_forms_matrix_body(REQUEST_FORMS_MATRIX_LARGE),
        "packet",
    );

    assert_eq!(tracked.outcome().await, ResponseOutcome::Resource(OutboundResult::Complete));
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_tracked_packet_response_resolves_packet() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_tracked_packet_response_resolves_packet().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_tracked_packet_response_resolves_packet() {
    let fixture = RequestHandlerFixture::start("python-rns-request-outcome-packet").await;
    let tracked = TrackedHandlerRecord::default();
    fixture.register(tracked.handler(request_handler_echo), RequestAllow::All).await;

    let mut client = fixture.connect(8.0, &[]).await;
    let lines = client.json_lines().await;

    let (request_id, _, data) = tracked.record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(&lines, request_id, b"reply:hello-rust", "packet");

    assert_eq!(tracked.outcome().await, ResponseOutcome::Packet);
}
