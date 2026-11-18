//! The `interface_stats` reply as `get_interface_stats` builds it
//! (`RNS/Reticulum.py:1405-1628`): a map with an `interfaces` array of
//! per-interface maps. Keys arrive as msgpack str (`RNS.vendor.umsgpack`).

use rmpv::Value;

use super::{InterfaceStat, InterfaceStats, RpcError};

type Map = [(Value, Value)];

pub(super) fn interface_stats(value: Value) -> Result<InterfaceStats, RpcError> {
    let map = value.as_map().ok_or_else(|| RpcError::Decode("reply is not a map".to_string()))?;
    let interfaces = required(map, "interfaces")?
        .as_array()
        .ok_or_else(|| wrong_type("interfaces", "array"))?
        .iter()
        .map(interface_stat)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InterfaceStats { interfaces, raw: value })
}

/// One `ifstats` dict (`RNS/Reticulum.py:1409-1410`, `:1501-1503`,
/// `:1527-1566`).
fn interface_stat(entry: &Value) -> Result<InterfaceStat, RpcError> {
    let map = entry
        .as_map()
        .ok_or_else(|| RpcError::Decode("interfaces entry is not a map".to_string()))?;
    Ok(InterfaceStat {
        name: required_str(map, "name")?,
        short_name: required_str(map, "short_name")?,
        kind: required_str(map, "type")?,
        online: required(map, "status")?.as_bool().ok_or_else(|| wrong_type("status", "bool"))?,
        peers: optional_u64(map, "peers")?,
        clients: optional_u64(map, "clients")?,
        rxb: required_u64(map, "rxb")?,
        txb: required_u64(map, "txb")?,
        raw: entry.clone(),
    })
}

fn field<'a>(map: &'a Map, key: &str) -> Option<&'a Value> {
    map.iter().find(|(name, _)| name.as_str() == Some(key)).map(|(_, value)| value)
}

fn required<'a>(map: &'a Map, key: &str) -> Result<&'a Value, RpcError> {
    field(map, key).ok_or_else(|| RpcError::Decode(format!("missing key {key:?}")))
}

fn required_str(map: &Map, key: &str) -> Result<String, RpcError> {
    required(map, key)?.as_str().map(str::to_owned).ok_or_else(|| wrong_type(key, "str"))
}

fn required_u64(map: &Map, key: &str) -> Result<u64, RpcError> {
    required(map, key)?.as_u64().ok_or_else(|| wrong_type(key, "non-negative integer"))
}

/// Absent and nil both read `None`: Python writes `None` for a `clients`
/// attribute an interface lacks and omits `peers` for one without the
/// attribute (`RNS/Reticulum.py:1409-1410`, `:1501-1503`).
fn optional_u64(map: &Map, key: &str) -> Result<Option<u64>, RpcError> {
    match field(map, key) {
        None | Some(Value::Nil) => Ok(None),
        Some(value) => {
            value.as_u64().map(Some).ok_or_else(|| wrong_type(key, "non-negative integer or nil"))
        }
    }
}

fn wrong_type(key: &str, expected: &str) -> RpcError {
    RpcError::Decode(format!("key {key:?} is not {expected}"))
}
