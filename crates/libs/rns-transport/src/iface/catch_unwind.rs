//! Poll-level panic capture for a future awaited inline. A spawned task that
//! panics surfaces as a `JoinError` at its join point; a loop awaited inline
//! unwinds its caller instead, and a driver that owns the interface tx
//! receiver loses it. Wrapping the loop here turns the panic back into an
//! `Err` the driver handles in place.

use std::any::Any;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Resolves to the inner output, or to the panic message when a poll of the
/// inner future panics. The inner future is never polled again after a panic.
pub(crate) struct CatchUnwind<F> {
    future: Pin<Box<F>>,
}

impl<F: Future> CatchUnwind<F> {
    pub(crate) fn new(future: F) -> Self {
        Self { future: Box::pin(future) }
    }
}

impl<F: Future> Future for CatchUnwind<F> {
    type Output = Result<F::Output, String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match catch_unwind(AssertUnwindSafe(|| self.future.as_mut().poll(cx))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
            Err(payload) => Poll::Ready(Err(panic_message(payload.as_ref()))),
        }
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::CatchUnwind;

    #[tokio::test]
    async fn passes_the_output_through_when_nothing_panics() {
        assert_eq!(CatchUnwind::new(async { 7 }).await, Ok(7));
    }

    #[tokio::test]
    async fn reports_a_panic_as_its_message_without_unwinding_the_caller() {
        let result = CatchUnwind::new(async {
            tokio::task::yield_now().await;
            panic!("boom {}", 1);
        })
        .await;
        assert_eq!(result, Err("boom 1".to_string()));
    }
}
