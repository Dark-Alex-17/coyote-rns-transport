//! Checks the error `delivery::await_link_activation` returns on a closed link; shared by
//! the integration tests that close a pending link under a wait.

use std::io;

/// Asserts `error` is `ConnectionAborted` with the text
/// `link closed before activation: <reason>`, `reason` spelt as the contract spells it.
pub fn assert_closed_before_activation(error: &io::Error, reason: &str) {
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted, "unexpected error: {error}");
    assert_eq!(
        error.to_string(),
        format!("link closed before activation: {reason}"),
        "the error text names the close reason exactly as the contract spells it"
    );
}
