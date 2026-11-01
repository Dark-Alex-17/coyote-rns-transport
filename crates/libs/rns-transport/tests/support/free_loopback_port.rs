//! A port the loopback stack has just released; shared by the integration tests that bind
//! a listener.

#[deny(clippy::await_holding_invalid_type)]
pub async fn free_loopback_port() -> u16 {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("probe address").port();
    drop(probe);
    port
}
