//! Port of `JSONValue.swift`.
//!
//! Choice: a dedicated [`JsonValue`] enum rather than `serde_json::Value`, because the Swift
//! semantics differ from `serde_json::Value` in ways callers depend on:
//!
//! - Every number is a `f64` (Swift decodes numbers as `Double`), so `int_value` is
//!   `Int(exactly:)`: `3.0` is 3, `3.5` is `None`, and integers past 2^53 lose precision the same
//!   way they do in Swift.
//! - Objects are a `BTreeMap`, so encoding writes keys sorted, like `JSONEncoder` with
//!   `.sortedKeys`. Keys sort by bytes; Swift sorts by `String` order, which is the same for the
//!   ASCII keys iTop uses.
//! - Encoding writes an integral number below 1e15 as an integer (`4`, not `4.0`), exactly as the
//!   Swift `encode(to:)` does. Other numbers use serde_json's float form, which matches Swift for
//!   ordinary fractions (`2.5`) but not for huge ones (`1000000000000000.0` here, `1e+15` in
//!   Swift); iTop never needs those.
//! - Slashes are not escaped (`.withoutEscapingSlashes`); `serde_json` never escapes them.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A JSON object, keys sorted.
pub type JsonObject = BTreeMap<String, JsonValue>;

/// Any JSON value, so iTop's loosely typed responses can be decoded and passed around safely.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum JsonValue {
    String(String),
    Number(f64),
    Bool(bool),
    Array(Vec<JsonValue>),
    Object(JsonObject),
    #[default]
    Null,
}

impl JsonValue {
    /// Decodes JSON bytes, like `JSONDecoder().decode(JSONValue.self, from:)`.
    pub fn parse(bytes: &[u8]) -> Result<JsonValue, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Decodes a JSON string.
    pub fn parse_str(text: &str) -> Result<JsonValue, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Compact JSON with sorted keys and unescaped slashes, like `ITopClient.encode` in Swift.
    /// Falls back to `{}` as the Swift code does (encoding cannot actually fail here).
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// The value under `key` when this is an object (Swift's `subscript(key:)`).
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(o) => o.get(key),
            _ => None,
        }
    }

    pub fn string_value(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// iTop sends most numbers as strings ("4"), and some as numbers.
    ///
    /// Numbers must be whole and fit in 64 bits (`Int(exactly:)`); strings are parsed like Swift's
    /// `Int(_:)`: an optional sign and ASCII digits, nothing else (no spaces).
    pub fn int_value(&self) -> Option<i64> {
        match self {
            JsonValue::Number(n) => int_exactly(*n),
            JsonValue::String(s) => swift_int(s),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&JsonObject> {
        match self {
            JsonValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }
}

/// Swift's `Int(exactly: Double)`.
fn int_exactly(n: f64) -> Option<i64> {
    // 2^63 is exactly representable; anything at or above it does not fit.
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    if n.is_finite() && n.fract() == 0.0 && (-LIMIT..LIMIT).contains(&n) {
        Some(n as i64)
    } else {
        None
    }
}

/// Swift's `Int(_ text: String)`: optional `+` or `-`, then one or more ASCII digits.
pub(crate) fn swift_int(s: &str) -> Option<i64> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

impl From<&str> for JsonValue {
    fn from(s: &str) -> Self {
        JsonValue::String(s.to_string())
    }
}

impl From<String> for JsonValue {
    fn from(s: String) -> Self {
        JsonValue::String(s)
    }
}

impl From<bool> for JsonValue {
    fn from(b: bool) -> Self {
        JsonValue::Bool(b)
    }
}

impl From<f64> for JsonValue {
    fn from(n: f64) -> Self {
        JsonValue::Number(n)
    }
}

impl From<i64> for JsonValue {
    fn from(n: i64) -> Self {
        JsonValue::Number(n as f64)
    }
}

impl From<JsonObject> for JsonValue {
    fn from(o: JsonObject) -> Self {
        JsonValue::Object(o)
    }
}

impl From<Vec<JsonValue>> for JsonValue {
    fn from(a: Vec<JsonValue>) -> Self {
        JsonValue::Array(a)
    }
}

impl Serialize for JsonValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            JsonValue::String(s) => serializer.serialize_str(s),
            JsonValue::Number(n) => {
                if n.round() == *n && n.abs() < 1e15 {
                    serializer.serialize_i64(*n as i64)
                } else {
                    serializer.serialize_f64(*n)
                }
            }
            JsonValue::Bool(b) => serializer.serialize_bool(*b),
            JsonValue::Array(a) => a.serialize(serializer),
            JsonValue::Object(o) => o.serialize(serializer),
            JsonValue::Null => serializer.serialize_unit(),
        }
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonValueVisitor)
    }
}

struct JsonValueVisitor;

impl<'de> Visitor<'de> for JsonValueVisitor {
    type Value = JsonValue;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<JsonValue, E> {
        Ok(JsonValue::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(v as f64))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(v as f64))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<JsonValue, E> {
        Ok(JsonValue::String(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<JsonValue, E> {
        Ok(JsonValue::String(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<JsonValue, E> {
        Ok(JsonValue::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<JsonValue, E> {
        Ok(JsonValue::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<JsonValue, D::Error> {
        Deserialize::deserialize(d)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsonValue, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = seq.next_element()? {
            out.push(v);
        }
        Ok(JsonValue::Array(out))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsonValue, A::Error> {
        let mut out = JsonObject::new();
        while let Some((k, v)) = map.next_entry::<String, JsonValue>()? {
            out.insert(k, v);
        }
        Ok(JsonValue::Object(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_value_follows_swift() {
        assert_eq!(JsonValue::from("4").int_value(), Some(4));
        assert_eq!(JsonValue::from("+4").int_value(), Some(4));
        assert_eq!(JsonValue::from("-4").int_value(), Some(-4));
        assert_eq!(JsonValue::from(" 4").int_value(), None);
        assert_eq!(JsonValue::from("").int_value(), None);
        assert_eq!(JsonValue::from("4.0").int_value(), None);
        assert_eq!(JsonValue::Number(3.0).int_value(), Some(3));
        assert_eq!(JsonValue::Number(3.5).int_value(), None);
        assert_eq!(JsonValue::Number(1e19).int_value(), None);
        assert_eq!(JsonValue::Bool(true).int_value(), None);
    }

    #[test]
    fn encodes_sorted_with_integers_and_unescaped_slashes() {
        let v = JsonValue::parse_str(r#"{"b":1,"a":"x/y","c":[2.5,-0.25,3.0,-7]}"#).unwrap();
        assert_eq!(
            v.to_json_string(),
            r#"{"a":"x/y","b":1,"c":[2.5,-0.25,3,-7]}"#
        );
    }

    #[test]
    fn bools_are_not_numbers() {
        assert_eq!(JsonValue::parse_str("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(JsonValue::parse_str("1").unwrap(), JsonValue::Number(1.0));
        assert_eq!(JsonValue::parse_str("null").unwrap(), JsonValue::Null);
    }
}
