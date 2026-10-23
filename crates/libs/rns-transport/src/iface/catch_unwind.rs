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
/// inner future panics. The inner future is dropped on the panic and never
/// polled again; polling this wrapper after that is a caller bug and panics.
pub(crate) struct CatchUnwind<F> {
    future: Option<Pin<Box<F>>>,
}

impl<F: Future> CatchUnwind<F> {
    pub(crate) fn new(future: F) -> Self {
        Self { future: Some(Box::pin(future)) }
    }
}

impl<F: Future> Future for CatchUnwind<F> {
    type Output = Result<F::Output, String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(future) = self.future.as_mut() else {
            panic!("CatchUnwind polled after its inner future panicked");
        };
        match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
            Err(payload) => {
                self.future.take();
                Poll::Ready(Err(panic_message(payload.as_ref())))
            }
        }
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic (non-string payload)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::CatchUnwind;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

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

    #[tokio::test]
    async fn reports_a_literal_panic_message_verbatim() {
        let result = CatchUnwind::new(async { panic!("boom") }).await;
        assert_eq!(result, Err("boom".to_string()));
    }

    #[test]
    #[should_panic(expected = "CatchUnwind polled after its inner future panicked")]
    fn a_poll_after_the_caught_panic_does_not_reach_the_inner_future() {
        // An async block resumed after a panic reports "resumed after
        // panicking"; the expected message pins which future was polled.
        let mut wrapped = pin!(CatchUnwind::new(async { panic!("first poll") }));
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(wrapped.as_mut().poll(&mut cx), Poll::Ready(Err("first poll".to_string())));
        let _ = wrapped.as_mut().poll(&mut cx);
    }
}
