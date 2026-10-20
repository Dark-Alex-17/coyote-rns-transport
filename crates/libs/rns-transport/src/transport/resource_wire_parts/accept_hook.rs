// The accept-hook consultation of `resource_wire.rs`, `include!`d into that
// module so it stays below the repository's 500-line limit; a file boundary,
// not a privacy one.

use std::panic::{catch_unwind, AssertUnwindSafe};

/// What the installed [`ResourceAcceptHook`] said about a plain advertisement.
pub(super) enum HookVerdict {
    Accepted,
    /// The peer is owed a receiver cancel.
    Rejected,
    /// The hook panicked; the advertisement is dropped unanswered, as
    /// `RNS/Link.py:1073-1074` logs the callback's exception and neither
    /// accepts nor rejects.
    Panicked,
}

/// Consults the accept hook for an advertisement the request and response
/// branches did not claim; without a hook every such advertisement is
/// accepted. The hook is caller code running on the inbound worker, so a
/// panic is caught here instead of unwinding into the worker, whose exit
/// would cancel every other worker of the transport.
pub(super) fn consult_accept_hook(
    handler: &TransportHandler,
    link: &Link,
    advertisement: &ResourceAdvertisement,
) -> HookVerdict {
    let Some(hook) = handler.resource_accept_hook.as_ref() else {
        return HookVerdict::Accepted;
    };
    let verdict = catch_unwind(AssertUnwindSafe(|| {
        hook(advertisement, link.id(), link.identified_peer_identity())
    }));
    match verdict {
        Ok(true) => HookVerdict::Accepted,
        Ok(false) => {
            log::debug!(
                "[resource-diag] reject_advertisement_by_hook link={} hash={} data_size={}",
                link.id(),
                advertisement.hash,
                advertisement.data_size
            );
            HookVerdict::Rejected
        }
        Err(_) => {
            log::error!(
                "[resource-diag] accept_hook_panicked link={} hash={}",
                link.id(),
                advertisement.hash
            );
            HookVerdict::Panicked
        }
    }
}
