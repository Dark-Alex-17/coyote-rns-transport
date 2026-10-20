use core::fmt;

use crate::hash::{address_hash, Hash, ADDRESS_HASH_SIZE};

/// `truncated_hash(path.encode("utf-8"))`: the request path as a request
/// carries it and as a destination registers its handlers
/// (`RNS/Link.py:486`, `RNS/Identity.py:362-369`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathHash([u8; ADDRESS_HASH_SIZE]);

impl PathHash {
    /// Wraps an already computed hash: the 16 truncated-hash bytes a request
    /// carries (`RNS/Link.py:486`).
    pub const fn new(bytes: [u8; ADDRESS_HASH_SIZE]) -> Self {
        Self(bytes)
    }

    /// The hash of `path`: SHA-256 over its UTF-8 bytes, truncated to 16,
    /// `RNS.Identity.truncated_hash(path.encode("utf-8"))`
    /// (`RNS/Link.py:486`, `RNS/Identity.py:362-369`).
    pub fn of(path: &str) -> Self {
        Self(address_hash(path.as_bytes()))
    }

    /// The 16 truncated-hash bytes, as a request packs them at
    /// `unpacked_request[1]` (`RNS/Link.py:486-487`).
    pub const fn as_bytes(&self) -> &[u8; ADDRESS_HASH_SIZE] {
        &self.0
    }

    /// The 16 truncated-hash bytes by value; the owned twin of
    /// [`as_bytes`](Self::as_bytes) (`RNS/Link.py:486-487`).
    pub const fn to_bytes(self) -> [u8; ADDRESS_HASH_SIZE] {
        self.0
    }
}

impl From<[u8; ADDRESS_HASH_SIZE]> for PathHash {
    fn from(bytes: [u8; ADDRESS_HASH_SIZE]) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for PathHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(f, &self.0)
    }
}

/// The id a response answers to. `RNS.Link.request` derives it two ways
/// (`RNS/Link.py:490-504`): a request that fits one link packet is known by
/// that packet's truncated hash, a request sent as a resource by the
/// truncated hash of its packed bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId([u8; ADDRESS_HASH_SIZE]);

impl RequestId {
    /// Wraps an already computed id: the 16 bytes `RequestReceipt.hash`
    /// holds (`RNS/Link.py:1332`, `:1337`).
    pub const fn new(bytes: [u8; ADDRESS_HASH_SIZE]) -> Self {
        Self(bytes)
    }

    /// The packet form: the first 16 bytes of the request packet's hash,
    /// `packet.getTruncatedHash()` (`RNS/Link.py:996`, `RNS/Packet.py:355`).
    pub fn from_packet_hash(hash: &Hash) -> Self {
        let mut id = [0u8; ADDRESS_HASH_SIZE];
        id.copy_from_slice(&hash.as_slice()[..ADDRESS_HASH_SIZE]);
        Self(id)
    }

    /// The resource form: `truncated_hash(packed_request)` over the bytes a
    /// [`RequestFrame`](super::RequestFrame) encodes to (`RNS/Link.py:504`);
    /// the receiver recomputes it from the assembled resource
    /// (`RNS/Link.py:885-889`).
    pub fn of_packed_request(packed: &[u8]) -> Self {
        Self(address_hash(packed))
    }

    /// The 16 id bytes a response names at `unpacked_response[0]`
    /// (`RNS/Link.py:490-504`, `:1014`).
    pub const fn as_bytes(&self) -> &[u8; ADDRESS_HASH_SIZE] {
        &self.0
    }

    /// The 16 id bytes by value; the owned twin of
    /// [`as_bytes`](Self::as_bytes) (`RNS/Link.py:490-504`).
    pub const fn to_bytes(self) -> [u8; ADDRESS_HASH_SIZE] {
        self.0
    }
}

impl From<[u8; ADDRESS_HASH_SIZE]> for RequestId {
    fn from(bytes: [u8; ADDRESS_HASH_SIZE]) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(f, &self.0)
    }
}

fn write_hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(f, "{byte:02x}")?;
    }
    Ok(())
}
