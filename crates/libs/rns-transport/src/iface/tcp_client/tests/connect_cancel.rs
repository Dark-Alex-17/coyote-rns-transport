use super::*;
use crate::iface::tcp_client::TcpRuntimeStatusHandle;
use crate::iface::InterfaceContext;

// 192.0.2.1 is TEST-NET-1: never routed, so the SYN is black-holed and the
// connect only ends by timeout or cancellation.
const BLACK_HOLE_ENDPOINT: &str = "192.0.2.1:1";

fn black_hole_client_context(
    manager: &mut InterfaceManager,
) -> (InterfaceContext<TcpClient>, TcpRuntimeStatusHandle) {
    let client = TcpClient::new(BLACK_HOLE_ENDPOINT.to_string())
        .with_connect_timeout(Duration::from_secs(30));
    let runtime_status = client.runtime_status_handle();
    (manager.new_context(client), runtime_status)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_cancel_interrupts_an_in_flight_connect() {
    let mut manager = InterfaceManager::new(8);
    let (context, runtime_status) = black_hole_client_context(&mut manager);
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(runtime_status.to_json()["stream_state"].as_str(), Some("connecting"));
    cancel.cancel();

    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("tcp client should stop while a connect is in flight")
        .expect("tcp client task should not panic");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_iface_stop_interrupts_an_in_flight_connect() {
    let mut manager = InterfaceManager::new(8);
    let (context, runtime_status) = black_hole_client_context(&mut manager);
    let iface_stop = context.channel.stop.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(runtime_status.to_json()["stream_state"].as_str(), Some("connecting"));
    iface_stop.cancel();

    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("tcp client should stop while a connect is in flight")
        .expect("tcp client task should not panic");
}
