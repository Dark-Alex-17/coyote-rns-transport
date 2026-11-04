//! The attempt counter a failed-dial log line ends in; shared by the integration tests that
//! read the dial log cadence.

/// The `N` of a failed-dial line's trailing `(attempt N)`.
pub fn attempt_number(message: &str) -> u64 {
    let (_, tail) = message.rsplit_once(" (attempt ").expect("the line ends in its attempt");
    tail.strip_suffix(')')
        .expect("the attempt is parenthesised")
        .parse()
        .unwrap_or_else(|err| panic!("attempt number in {message:?}: {err}"))
}
