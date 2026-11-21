//! The `path_table` reply as `get_path_table` builds it
//! (`RNS/Reticulum.py:1638-1648`): an array of one map per known
//! destination, `{"hash": dst_hash, "timestamp": path_table[dst][0], "via":
//! path_table[dst][1], "hops": path_hops, "expires": path_table[dst][3],
//! "interface": str(path_table[dst][5])}`.

use rmpv::Value;

use super::decode::{required, required_str, required_u64, wrong_type, Map};
use super::RpcError;
use crate::hash::AddressHash;

/// One row of the shared instance's path table (`RNS/Reticulum.py:1638-1648`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PathRow {
    /// The destination hash, the `path_table` key (`RNS/Reticulum.py:1641`).
    pub hash: AddressHash,
    /// `path_table[dst][0]`, when the path was learned, as a Unix time
    /// (`RNS/Reticulum.py:1642`). A path dropped through `expire_path` but
    /// not yet culled carries `0` here (`RNS/Transport.py:3219-3226`).
    pub timestamp: f64,
    /// `path_table[dst][1]`, the transport identity hash of the next hop
    /// (`RNS/Reticulum.py:1643`).
    pub via: AddressHash,
    /// `path_table[dst][2]`, the hop count to the destination
    /// (`RNS/Reticulum.py:1644`).
    pub hops: u8,
    /// `path_table[dst][3]`, when the path expires, as a Unix time
    /// (`RNS/Reticulum.py:1645`).
    pub expires: f64,
    /// `str(path_table[dst][5])`, the receiving interface in its
    /// `Type[name]` form (`RNS/Reticulum.py:1646`).
    pub interface: String,
    /// This entry's whole map as received, any key a newer server adds
    /// included.
    pub raw: Value,
}

pub(super) fn path_table(value: Value) -> Result<Vec<PathRow>, RpcError> {
    value
        .as_array()
        .ok_or_else(|| RpcError::Decode("reply is not an array".to_string()))?
        .iter()
        .map(path_row)
        .collect()
}

fn path_row(entry: &Value) -> Result<PathRow, RpcError> {
    let map = entry
        .as_map()
        .ok_or_else(|| RpcError::Decode("path_table entry is not a map".to_string()))?;
    Ok(PathRow {
        hash: required_hash(map, "hash")?,
        timestamp: required_f64(map, "timestamp")?,
        via: required_hash(map, "via")?,
        hops: u8::try_from(required_u64(map, "hops")?)
            .map_err(|_| wrong_type("hops", "a hop count"))?,
        expires: required_f64(map, "expires")?,
        interface: required_str(map, "interface")?,
        raw: entry.clone(),
    })
}

/// Python packs `bytes` as msgpack bin (`RNS/vendor/umsgpack.py:505-506`).
fn required_hash(map: &Map, key: &str) -> Result<AddressHash, RpcError> {
    match required(map, key)? {
        Value::Binary(bytes) => <[u8; 16]>::try_from(bytes.as_slice())
            .map(AddressHash::new)
            .map_err(|_| wrong_type(key, "a 16-byte bin")),
        _ => Err(wrong_type(key, "a 16-byte bin")),
    }
}

/// A Python float arrives as msgpack float, but `expire_path` writes the
/// int `0` into the timestamp slot (`RNS/Transport.py:3219-3226`), so both
/// read as a number.
fn required_f64(map: &Map, key: &str) -> Result<f64, RpcError> {
    required(map, key)?.as_f64().ok_or_else(|| wrong_type(key, "a number"))
}
