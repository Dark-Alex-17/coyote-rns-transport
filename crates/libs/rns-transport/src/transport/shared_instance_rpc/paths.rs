//! The path calls `rpc_loop` answers (`RNS/Reticulum.py:1274-1334`): the
//! `get` requests `path_table` (`:1283-1285`), `next_hop_if_name` (`:1289`)
//! and `first_hop_timeout` (`:1291`), and the `drop` requests `path`
//! (`:1305`) and `all_via` (`:1306`). Each call is one fresh connection, the
//! handshake, one request and one reply, after which the server closes.
//!
//! Destination hashes travel as msgpack bin: Python's `path_table` dict is
//! keyed by `bytes` (`RNS/Transport.py:2459`), which umsgpack packs as bin
//! (`RNS/vendor/umsgpack.py:505-506`); a str key would miss the entry and
//! `drop path` would answer `False`.

use std::time::Duration;

use rmpv::Value;

use super::{call, path_table::PathRow, RpcEndpoint, RpcError, RpcKey};
use crate::hash::AddressHash;

/// How often [`drop_path_settled`] re-reads the next hop while the cull is
/// pending; `Transport.jobs` runs every `job_interval = 0.250` seconds
/// (`RNS/Transport.py:248`).
const SETTLE_POLL: Duration = Duration::from_millis(50);

/// The shared instance's path table (`RNS/Reticulum.py:1283-1285`,
/// `:1638-1648`), every known destination or, with `max_hops`, only those at
/// most that many hops away. The request always carries the `max_hops` key,
/// nil when `None`: rnsd reads `call["max_hops"]` and a missing key raises a
/// `KeyError` that `rpc_loop` logs at `RNS/Reticulum.py:1333-1334` without
/// answering, so the client would hang until its timeout, or until the next
/// dial lets rnsd release the socket and it reads EOF. The whole call runs
/// under `timeout`; outrunning it is [`RpcError::Timeout`].
pub async fn path_table(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    max_hops: Option<u8>,
    timeout: Duration,
) -> Result<Vec<PathRow>, RpcError> {
    tokio::time::timeout(timeout, async {
        let max_hops = max_hops.map_or(Value::Nil, Value::from);
        let request = request([("get", Value::from("path_table")), ("max_hops", max_hops)])?;
        super::path_table::path_table(call(endpoint, key, &request).await?)
    })
    .await
    .map_err(|_| RpcError::Timeout)?
}

/// The `Type[name]` of the interface the next hop towards `destination` is
/// reached on, `None` without one. rnsd answers
/// `str(Transport.next_hop_interface(d))` (`RNS/Reticulum.py:1715`), and
/// `next_hop_interface` (`RNS/Transport.py:3162-3164`) answers the receiving
/// interface of the path table entry, the shared instance interface for a
/// destination registered inside the rnsd process itself (its
/// `destinations_map`, `RNS/Transport.py:2908-2909`, which keeps such
/// destinations out of the path table, `:2211`), and `None` otherwise, so
/// the four-character str `"None"` reads as no path, as does a msgpack nil.
/// The whole call runs under `timeout`; outrunning it is
/// [`RpcError::Timeout`].
pub async fn next_hop_interface_name(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    destination: &AddressHash,
    timeout: Duration,
) -> Result<Option<String>, RpcError> {
    tokio::time::timeout(timeout, next_hop_interface_name_untimed(endpoint, key, destination))
        .await
        .map_err(|_| RpcError::Timeout)?
}

/// Drops the path to `destination` (`RNS/Reticulum.py:1305`,
/// `RNS/Transport.py:3219-3226`): `true` when there was one. The entry
/// itself lingers until the next cull; see [`drop_path_settled`]. The whole
/// call runs under `timeout`; outrunning it is [`RpcError::Timeout`]. A
/// `Timeout` or [`RpcError::Io`] after the request left the client leaves
/// the drop's effect unknown; repeating the call is safe, since rnsd
/// answers `true` again until the cull pops the entry
/// (`RNS/Transport.py:1088-1090`) and `false` after.
pub async fn drop_path(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    destination: &AddressHash,
    timeout: Duration,
) -> Result<bool, RpcError> {
    tokio::time::timeout(timeout, drop_path_untimed(endpoint, key, destination))
        .await
        .map_err(|_| RpcError::Timeout)?
}

/// Drops every path whose `via` ([`PathRow::via`]) equals `transport_hash`
/// (`RNS/Reticulum.py:1306`, `:1689-1695`) and returns how many were
/// dropped. That slot is the announce's `received_from`: the relaying
/// transport's identity, or the destination hash itself for a path heard
/// directly (`RNS/Transport.py:2177-2178`, `:2202`), so passing a
/// destination hash drops its own direct one-hop path. The request key is
/// `destination_hash` even though the value is usually a transport hash;
/// that is the name `rpc_loop` reads
/// (`RNS/Reticulum.py:1306`). The whole call runs under `timeout`;
/// outrunning it is [`RpcError::Timeout`]. A `Timeout` or [`RpcError::Io`]
/// after the request left the client leaves the drops' effect unknown;
/// repeating the call is safe and re-counts the entries still lingering
/// before the cull (`RNS/Transport.py:1088-1090`).
pub async fn drop_all_via(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    transport_hash: &AddressHash,
    timeout: Duration,
) -> Result<u64, RpcError> {
    tokio::time::timeout(timeout, async {
        let request = hash_request("drop", "all_via", transport_hash)?;
        call(endpoint, key, &request)
            .await?
            .as_u64()
            .ok_or_else(|| RpcError::Decode("reply is not an integer".to_string()))
    })
    .await
    .map_err(|_| RpcError::Timeout)?
}

/// The seconds `Transport.first_hop_timeout(destination)` allows the first
/// hop (`RNS/Reticulum.py:1291`, `RNS/Transport.py:3194-3197`): the float
/// `MTU * latency + DEFAULT_PER_HOP_TIMEOUT` with a path, or the int
/// `DEFAULT_PER_HOP_TIMEOUT` of `6` without one (`RNS/Reticulum.py:93`,
/// `:142`), so both msgpack int and float read as a number. The whole call
/// runs under `timeout`; outrunning it is [`RpcError::Timeout`].
pub async fn first_hop_timeout(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    destination: &AddressHash,
    timeout: Duration,
) -> Result<f64, RpcError> {
    tokio::time::timeout(timeout, async {
        let request = hash_request("get", "first_hop_timeout", destination)?;
        call(endpoint, key, &request)
            .await?
            .as_f64()
            .ok_or_else(|| RpcError::Decode("reply is not a number".to_string()))
    })
    .await
    .map_err(|_| RpcError::Timeout)?
}

/// [`drop_path`] followed by polling [`next_hop_interface_name`] until the
/// entry is really gone. `Ok(false)` when there was no path to drop (no
/// polling), `Ok(true)` once the next hop reads as none.
///
/// The drop window: `expire_path` only zeroes the entry's timestamp and
/// resets `tables_last_culled` (`RNS/Transport.py:3219-3226`); the entry is
/// popped by the cull on the next `Transport.jobs` tick (`job_interval =
/// 0.250`, `RNS/Transport.py:248`; the gate at `:855`, the pop at
/// `:1088-1090`). Until then `path_request` (`RNS/Transport.py:3459`) still
/// answers from the dropped entry's cached announce, so a path request sent
/// right after a bare [`drop_path`] is answered from the stale entry. A
/// caller that wants a fresh path sends one path request after this helper
/// returns `Ok(true)`.
///
/// The polls run every 50 ms, and every dial (the drop and each poll)
/// shares the single `timeout` budget; outrunning it is
/// [`RpcError::Timeout`]. That `Timeout` does not only mean a slow cull: the
/// destination may have been re-learned between the pop and a poll, because
/// a fresh announce arrived, so a caller re-reads [`path_table`] before
/// treating it as a failure; and a destination registered inside the rnsd
/// process never reads as `None` (see [`next_hop_interface_name`]). Like
/// the rest of this library the helper never retries: a poll that fails
/// with a transient error (see [`RpcError::is_transient`]) is returned as
/// it is, budget left or not. The dials are not one transaction: an error
/// after the drop answered `true` leaves the drop in place, and a repeated
/// call then returns `Ok(false)` once the cull has run, so a caller that
/// retries reads that `Ok(false)` as settled rather than as nothing to
/// drop, or re-reads [`path_table`].
pub async fn drop_path_settled(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    destination: &AddressHash,
    timeout: Duration,
) -> Result<bool, RpcError> {
    tokio::time::timeout(timeout, async {
        if !drop_path_untimed(endpoint, key, destination).await? {
            return Ok(false);
        }
        while next_hop_interface_name_untimed(endpoint, key, destination).await?.is_some() {
            tokio::time::sleep(SETTLE_POLL).await;
        }
        Ok(true)
    })
    .await
    .map_err(|_| RpcError::Timeout)?
}

async fn next_hop_interface_name_untimed(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    destination: &AddressHash,
) -> Result<Option<String>, RpcError> {
    let request = hash_request("get", "next_hop_if_name", destination)?;
    // Python's `str()` always packs valid UTF-8; the arm for a str that is
    // not is kept apart from a reply that is no str at all.
    match call(endpoint, key, &request).await? {
        Value::Nil => Ok(None),
        Value::String(name) => match name.as_str() {
            Some("None") => Ok(None),
            Some(name) => Ok(Some(name.to_owned())),
            None => Err(RpcError::Decode("reply str is not utf-8".to_string())),
        },
        _ => Err(RpcError::Decode("reply is not a str".to_string())),
    }
}

async fn drop_path_untimed(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    destination: &AddressHash,
) -> Result<bool, RpcError> {
    let request = hash_request("drop", "path", destination)?;
    call(endpoint, key, &request)
        .await?
        .as_bool()
        .ok_or_else(|| RpcError::Decode("reply is not a bool".to_string()))
}

/// `{verb: name, "destination_hash": <bin>}`, the shape every hash-carrying
/// request shares (`RNS/Reticulum.py:1289`, `:1291`, `:1305-1306`).
fn hash_request(verb: &str, name: &str, hash: &AddressHash) -> Result<Vec<u8>, RpcError> {
    request([
        (verb, Value::from(name)),
        ("destination_hash", Value::Binary(hash.as_slice().to_vec())),
    ])
}

/// A msgpack map with the keys in the given order, as `mp.packb` of a dict
/// literal would emit them. Writing into a `Vec` does not fail in practice;
/// the io error path exists only to keep the builder free of panics, and it
/// reads as [`RpcError::Protocol`], the variant for bytes that are wrong,
/// since no variant names a request that would not encode.
fn request<'a>(pairs: impl IntoIterator<Item = (&'a str, Value)>) -> Result<Vec<u8>, RpcError> {
    let map = Value::Map(pairs.into_iter().map(|(key, value)| (Value::from(key), value)).collect());
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &map)
        .map_err(|err| RpcError::Protocol(err.to_string()))?;
    Ok(bytes)
}
