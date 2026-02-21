//! Canonical runtime value and change types for OSV v2.
//!
//! `Sthithi` replaces `serde_json::Value` as the single representation of settled state.
//! `Parivarta` replaces `JsonOp` as the uniform change description.

use std::fmt;

use base64::Engine;
use loro::LoroValue;
use serde::{Deserialize, Serialize};

use super::layer::JsonOp;

// ---------------------------------------------------------------------------
// Sthithi — canonical runtime value
// ---------------------------------------------------------------------------

/// Canonical runtime value representation.
///
/// **Context**: Replaces `serde_json::Value` as the single representation of settled state.
/// Carries distinct Int/Float, native Bytes, and ordered Map — fixing lossy conversions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Sthithi {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Sthithi>),
    Map(Vec<(String, Sthithi)>),
}

// ---------------------------------------------------------------------------
// OpKind + Parivarta — canonical change representation
// ---------------------------------------------------------------------------

/// Operation kind for a change event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    Insert,
    Update,
    Delete,
    Set,
}

impl fmt::Display for OpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpKind::Insert => write!(f, "insert"),
            OpKind::Update => write!(f, "update"),
            OpKind::Delete => write!(f, "delete"),
            OpKind::Set => write!(f, "set"),
        }
    }
}

/// Canonical change representation.
///
/// **Context**: Replaces `JsonOp` as the uniform change description.
/// Carries `intent` for state transitions and `from_peer` for attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parivarta {
    /// Layer this change targets
    pub layer: String,
    /// Operation kind
    pub op: OpKind,
    /// Path within the layer (e.g., "root/items")
    pub path: String,
    /// Key for map operations
    pub key: Option<String>,
    /// Index for list operations
    pub index: Option<usize>,
    /// New value
    pub value: Option<Sthithi>,
    /// Previous value (for updates, looked up from Sthithi)
    pub old_value: Option<Sthithi>,
    /// Intent tag for state transitions (e.g., "confirm", "cancel")
    pub intent: Option<String>,
    /// Source peer (user_did, device_id). None = local.
    pub from_peer: Option<(String, String)>,
}

// ---------------------------------------------------------------------------
// LoroValue ↔ Sthithi conversions
// ---------------------------------------------------------------------------

impl From<LoroValue> for Sthithi {
    fn from(v: LoroValue) -> Self {
        match v {
            LoroValue::Null => Sthithi::Null,
            LoroValue::Bool(b) => Sthithi::Bool(b),
            LoroValue::I64(n) => Sthithi::Int(n),
            LoroValue::Double(f) => Sthithi::Float(f),
            LoroValue::String(s) => Sthithi::Str(s.to_string()),
            LoroValue::Binary(b) => Sthithi::Bytes(b.to_vec()),
            LoroValue::List(list) => {
                Sthithi::List(list.iter().cloned().map(Sthithi::from).collect())
            }
            LoroValue::Map(map) => Sthithi::Map(
                map.iter()
                    .map(|(k, v)| (k.clone(), Sthithi::from(v.clone())))
                    .collect(),
            ),
            // Container IDs are opaque references — resolve via get_deep_value before conversion
            LoroValue::Container(_) => Sthithi::Null,
        }
    }
}

impl From<&Sthithi> for LoroValue {
    fn from(s: &Sthithi) -> Self {
        match s {
            Sthithi::Null => LoroValue::Null,
            Sthithi::Bool(b) => LoroValue::Bool(*b),
            Sthithi::Int(n) => LoroValue::I64(*n),
            Sthithi::Float(f) => LoroValue::Double(*f),
            Sthithi::Str(s) => LoroValue::String(s.as_str().into()),
            Sthithi::Bytes(b) => LoroValue::Binary(b.clone().into()),
            Sthithi::List(items) => {
                let values: Vec<LoroValue> = items.iter().map(LoroValue::from).collect();
                LoroValue::List(values.into())
            }
            Sthithi::Map(entries) => {
                let pairs: Vec<(String, LoroValue)> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), LoroValue::from(v)))
                    .collect();
                LoroValue::Map(pairs.into())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// serde_json::Value ↔ Sthithi conversions (backward compatibility)
// ---------------------------------------------------------------------------

impl From<serde_json::Value> for Sthithi {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Sthithi::Null,
            serde_json::Value::Bool(b) => Sthithi::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Sthithi::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Sthithi::Float(f)
                } else {
                    Sthithi::Null
                }
            }
            serde_json::Value::String(s) => Sthithi::Str(s),
            serde_json::Value::Array(arr) => {
                Sthithi::List(arr.into_iter().map(Sthithi::from).collect())
            }
            serde_json::Value::Object(obj) => Sthithi::Map(
                obj.into_iter()
                    .map(|(k, v)| (k, Sthithi::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<&Sthithi> for serde_json::Value {
    fn from(s: &Sthithi) -> Self {
        match s {
            Sthithi::Null => serde_json::Value::Null,
            Sthithi::Bool(b) => serde_json::Value::Bool(*b),
            Sthithi::Int(n) => serde_json::json!(*n),
            Sthithi::Float(f) => serde_json::json!(*f),
            Sthithi::Str(s) => serde_json::Value::String(s.clone()),
            Sthithi::Bytes(b) => {
                // Lossy: JSON has no binary type, encode as base64 string
                let encoded = base64::engine::general_purpose::STANDARD.encode(b);
                serde_json::Value::String(encoded)
            }
            Sthithi::List(items) => {
                serde_json::Value::Array(items.iter().map(serde_json::Value::from).collect())
            }
            Sthithi::Map(entries) => {
                let map: serde_json::Map<String, serde_json::Value> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::from(v)))
                    .collect();
                serde_json::Value::Object(map)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JsonOp → Parivarta conversion (backward compatibility)
// ---------------------------------------------------------------------------

impl From<&JsonOp> for Parivarta {
    fn from(op: &JsonOp) -> Self {
        let kind = match op.op.as_str() {
            "insert" => OpKind::Insert,
            "update" | "set" => OpKind::Set,
            "delete" => OpKind::Delete,
            _ => OpKind::Set,
        };

        Parivarta {
            layer: String::new(),
            op: kind,
            path: op.path.clone(),
            key: op.key.clone(),
            index: op.index,
            value: op.value.as_ref().map(|v| Sthithi::from(v.clone())),
            old_value: op.old_value.as_ref().map(|v| Sthithi::from(v.clone())),
            intent: None,
            from_peer: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: LoroValue → Sthithi → LoroValue for each variant
    #[test]
    fn loro_round_trip_null() {
        let original = LoroValue::Null;
        let sth = Sthithi::from(original.clone());
        let back = LoroValue::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn loro_round_trip_bool() {
        let original = LoroValue::Bool(true);
        let sth = Sthithi::from(original.clone());
        let back = LoroValue::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn loro_round_trip_i64() {
        let original = LoroValue::I64(42);
        let sth = Sthithi::from(original.clone());
        assert_eq!(sth, Sthithi::Int(42));
        let back = LoroValue::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn loro_round_trip_double() {
        let original = LoroValue::Double(3.14);
        let sth = Sthithi::from(original.clone());
        assert_eq!(sth, Sthithi::Float(3.14));
        let back = LoroValue::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn loro_round_trip_string() {
        let original = LoroValue::String("hello".into());
        let sth = Sthithi::from(original.clone());
        assert_eq!(sth, Sthithi::Str("hello".to_string()));
        let back = LoroValue::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn loro_round_trip_binary() {
        let original = LoroValue::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF].into());
        let sth = Sthithi::from(original.clone());
        assert_eq!(sth, Sthithi::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]));
        let back = LoroValue::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn loro_round_trip_list() {
        let original = LoroValue::List(vec![LoroValue::I64(1), LoroValue::Bool(false)].into());
        let sth = Sthithi::from(original.clone());
        let back = LoroValue::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn loro_round_trip_map() {
        let original = LoroValue::Map(vec![("key".to_string(), LoroValue::I64(99))].into());
        let sth = Sthithi::from(original.clone());
        let back = LoroValue::from(&sth);
        // Map round-trip: order may differ in FxHashMap, compare structurally
        match (&original, &back) {
            (LoroValue::Map(a), LoroValue::Map(b)) => {
                assert_eq!(a.len(), b.len());
                for (k, v) in a.iter() {
                    assert_eq!(b.get(k), Some(v));
                }
            }
            _ => panic!("Expected Map variants"),
        }
    }

    #[test]
    fn loro_container_becomes_null() {
        let container = LoroValue::Container(loro::ContainerID::new_root(
            "test",
            loro::ContainerType::Map,
        ));
        let sth = Sthithi::from(container);
        assert_eq!(sth, Sthithi::Null);
    }

    /// Round-trip: serde_json::Value → Sthithi → serde_json::Value for each variant
    #[test]
    fn json_round_trip_null() {
        let original = serde_json::Value::Null;
        let sth = Sthithi::from(original.clone());
        let back = serde_json::Value::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn json_round_trip_bool() {
        let original = serde_json::json!(true);
        let sth = Sthithi::from(original.clone());
        let back = serde_json::Value::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn json_round_trip_int() {
        let original = serde_json::json!(42);
        let sth = Sthithi::from(original.clone());
        assert_eq!(sth, Sthithi::Int(42));
        let back = serde_json::Value::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn json_round_trip_float() {
        let original = serde_json::json!(3.14);
        let sth = Sthithi::from(original.clone());
        assert_eq!(sth, Sthithi::Float(3.14));
        let back = serde_json::Value::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn json_round_trip_string() {
        let original = serde_json::json!("hello");
        let sth = Sthithi::from(original.clone());
        let back = serde_json::Value::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn json_round_trip_array() {
        let original = serde_json::json!([1, "two", null]);
        let sth = Sthithi::from(original.clone());
        let back = serde_json::Value::from(&sth);
        assert_eq!(original, back);
    }

    #[test]
    fn json_round_trip_object() {
        let original = serde_json::json!({"a": 1, "b": "two"});
        let sth = Sthithi::from(original.clone());
        let back = serde_json::Value::from(&sth);
        assert_eq!(original, back);
    }

    /// Bytes → JSON → back produces base64 string (lossy, expected)
    #[test]
    fn bytes_to_json_is_base64_lossy() {
        let bytes = Sthithi::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let json = serde_json::Value::from(&bytes);
        let expected_b64 =
            base64::engine::general_purpose::STANDARD.encode([0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(json, serde_json::Value::String(expected_b64.clone()));

        // Converting back from JSON gives us a Str, not Bytes (lossy)
        let back = Sthithi::from(json);
        assert_eq!(back, Sthithi::Str(expected_b64));
    }

    /// JsonOp → Parivarta conversion
    #[test]
    fn json_op_to_parivarta() {
        let json_op = JsonOp {
            op: "insert".to_string(),
            path: "root/items".to_string(),
            key: Some("name".to_string()),
            index: None,
            value: Some(serde_json::json!("Alice")),
            old_value: None,
        };

        let p = Parivarta::from(&json_op);
        assert_eq!(p.op, OpKind::Insert);
        assert_eq!(p.path, "root/items");
        assert_eq!(p.key, Some("name".to_string()));
        assert_eq!(p.index, None);
        assert_eq!(p.value, Some(Sthithi::Str("Alice".to_string())));
        assert_eq!(p.old_value, None);
        assert_eq!(p.layer, "");
        assert_eq!(p.intent, None);
        assert_eq!(p.from_peer, None);
    }

    #[test]
    fn json_op_update_maps_to_set() {
        let json_op = JsonOp {
            op: "update".to_string(),
            path: "root".to_string(),
            key: None,
            index: Some(0),
            value: Some(serde_json::json!(42)),
            old_value: Some(serde_json::json!(10)),
        };

        let p = Parivarta::from(&json_op);
        assert_eq!(p.op, OpKind::Set);
        assert_eq!(p.value, Some(Sthithi::Int(42)));
        assert_eq!(p.old_value, Some(Sthithi::Int(10)));
    }

    #[test]
    fn json_op_delete() {
        let json_op = JsonOp {
            op: "delete".to_string(),
            path: "root/items".to_string(),
            key: Some("obsolete".to_string()),
            index: None,
            value: None,
            old_value: Some(serde_json::json!("old")),
        };

        let p = Parivarta::from(&json_op);
        assert_eq!(p.op, OpKind::Delete);
        assert_eq!(p.value, None);
        assert_eq!(p.old_value, Some(Sthithi::Str("old".to_string())));
    }

    #[test]
    fn json_op_unknown_op_maps_to_set() {
        let json_op = JsonOp {
            op: "unknown_op".to_string(),
            path: "root".to_string(),
            key: None,
            index: None,
            value: None,
            old_value: None,
        };

        let p = Parivarta::from(&json_op);
        assert_eq!(p.op, OpKind::Set);
    }

    /// Nested structures: Map containing List containing Int
    #[test]
    fn nested_structures() {
        let nested = Sthithi::Map(vec![
            (
                "numbers".to_string(),
                Sthithi::List(vec![Sthithi::Int(1), Sthithi::Int(2), Sthithi::Int(3)]),
            ),
            (
                "metadata".to_string(),
                Sthithi::Map(vec![
                    ("count".to_string(), Sthithi::Int(3)),
                    ("label".to_string(), Sthithi::Str("test".to_string())),
                ]),
            ),
        ]);

        // LoroValue round-trip
        let loro_val = LoroValue::from(&nested);
        let back = Sthithi::from(loro_val.clone());
        // Map key order may differ through LoroValue (HashMap), compare inner values
        match &back {
            Sthithi::Map(entries) => {
                assert_eq!(entries.len(), 2);
                let as_map: std::collections::HashMap<&str, &Sthithi> =
                    entries.iter().map(|(k, v)| (k.as_str(), v)).collect();
                match as_map.get("numbers") {
                    Some(Sthithi::List(items)) => {
                        assert_eq!(items.len(), 3);
                        assert_eq!(items[0], Sthithi::Int(1));
                    }
                    other => panic!("Expected List for 'numbers', got {:?}", other),
                }
                match as_map.get("metadata") {
                    Some(Sthithi::Map(inner)) => {
                        assert_eq!(inner.len(), 2);
                    }
                    other => panic!("Expected Map for 'metadata', got {:?}", other),
                }
            }
            _ => panic!("Expected Map"),
        }

        // JSON round-trip (serde_json::Map uses BTreeMap — keys are sorted alphabetically)
        let json_val = serde_json::Value::from(&nested);
        let back_json = Sthithi::from(json_val);
        match &back_json {
            Sthithi::Map(entries) => {
                assert_eq!(entries.len(), 2);
                let as_map: std::collections::HashMap<&str, &Sthithi> =
                    entries.iter().map(|(k, v)| (k.as_str(), v)).collect();
                assert!(matches!(as_map.get("numbers"), Some(Sthithi::List(_))));
                match as_map.get("metadata") {
                    Some(Sthithi::Map(inner)) => {
                        let inner_map: std::collections::HashMap<&str, &Sthithi> =
                            inner.iter().map(|(k, v)| (k.as_str(), v)).collect();
                        assert_eq!(inner_map.get("count"), Some(&&Sthithi::Int(3)));
                        assert_eq!(
                            inner_map.get("label"),
                            Some(&&Sthithi::Str("test".to_string()))
                        );
                    }
                    other => panic!("Expected Map for 'metadata', got {:?}", other),
                }
            }
            _ => panic!("Expected Map from JSON round-trip"),
        }
    }

    /// OpKind Display
    #[test]
    fn op_kind_display() {
        assert_eq!(OpKind::Insert.to_string(), "insert");
        assert_eq!(OpKind::Update.to_string(), "update");
        assert_eq!(OpKind::Delete.to_string(), "delete");
        assert_eq!(OpKind::Set.to_string(), "set");
    }
}
