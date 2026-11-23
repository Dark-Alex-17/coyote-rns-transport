//! An `rmpv::Value` map keyed by str, and a value packed to msgpack bytes; shared by the
//! integration tests whose loopback servers answer the rnsd RPC client with canned
//! replies. Every canned reply is built by the one and sent through the other.

use rmpv::Value;

pub fn pack(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, value).expect("pack into a vec");
    bytes
}

pub fn map(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(key, value)| (Value::from(key), value)).collect())
}
