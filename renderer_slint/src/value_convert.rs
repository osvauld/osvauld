use domains::Sthithi;
use slint::VecModel;
use slint_interpreter::Value as SlintValue;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Sthithi <-> Slint conversions (canonical path — no serde_json hop)
// ---------------------------------------------------------------------------

/// Convert Sthithi value to Slint value (direct, no JSON hop).
///
/// **Context**: Replaces `json_to_slint_value` as the canonical conversion path.
/// Preserves Int vs Float (both become Slint Number). Handles Bytes and image fields.
pub(crate) fn sthithi_to_slint_value(
    val: &Sthithi,
) -> Result<SlintValue, Box<dyn std::error::Error>> {
    sthithi_to_slint_inner(val, None)
}

fn sthithi_to_slint_inner(
    val: &Sthithi,
    field_name: Option<&str>,
) -> Result<SlintValue, Box<dyn std::error::Error>> {
    match val {
        Sthithi::Null => {
            // Image-typed fields must never be Void — Slint panics on .width/.height access.
            if field_name.map_or(false, is_image_field) {
                Ok(SlintValue::Image(slint::Image::default()))
            } else {
                Ok(SlintValue::Void)
            }
        }
        Sthithi::Bool(b) => Ok(SlintValue::Bool(*b)),
        Sthithi::Int(n) => Ok(SlintValue::Number(*n as f64)),
        Sthithi::Float(f) => Ok(SlintValue::Number(*f)),
        Sthithi::Str(s) => Ok(SlintValue::String(slint::SharedString::from(s.as_str()))),
        Sthithi::Bytes(_) => {
            // Slint has no binary type — represent as empty string
            Ok(SlintValue::String(slint::SharedString::default()))
        }
        Sthithi::List(items) => {
            let slint_items: Result<Vec<_>, _> = items
                .iter()
                .map(|v| sthithi_to_slint_inner(v, None))
                .collect();
            Ok(SlintValue::Model(
                Rc::new(VecModel::from(slint_items?)).into(),
            ))
        }
        Sthithi::Map(entries) => {
            let fields: Result<Vec<_>, _> = entries
                .iter()
                .map(|(k, v)| {
                    sthithi_to_slint_inner(v, Some(k.as_str())).map(|val| (k.clone(), val))
                })
                .collect();
            Ok(SlintValue::Struct(
                slint_interpreter::Struct::from_iter(fields?).into(),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// JSON <-> Slint conversions (legacy — kept during migration)
// ---------------------------------------------------------------------------

/// Convert Slint value to JSON for passing to Lua.
pub(crate) fn slint_value_to_json(value: &SlintValue) -> serde_json::Value {
    match value {
        SlintValue::String(s) => serde_json::Value::String(s.to_string()),
        SlintValue::Number(n) => serde_json::json!(n),
        SlintValue::Bool(b) => serde_json::Value::Bool(*b),
        SlintValue::Void => serde_json::Value::Null,
        SlintValue::Image(_) => serde_json::Value::Null,
        SlintValue::Model(_) => serde_json::Value::Null,
        SlintValue::Struct(s) => {
            let obj: serde_json::Map<String, serde_json::Value> = s
                .iter()
                .map(|(k, v)| (k.to_string(), slint_value_to_json(&v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        SlintValue::Brush(_) => serde_json::Value::Null,
        _ => serde_json::Value::Null,
    }
}

/// Check whether a struct field name indicates an `image`-typed Slint property.
///
/// **Context**: The Slint interpreter panics (`"First argument not an image"`) when
/// built-in functions like `ImageSize` evaluate a `Void` value where `Image` is expected.
/// JSON has no image type, so `null` → `Void` for these fields triggers the panic.
/// We detect image fields by naming convention and substitute a default (0×0) `Image`.
fn is_image_field(field_name: &str) -> bool {
    field_name.ends_with("_image") || field_name == "image" || field_name == "source"
}

/// Convert JSON to Slint value (type-unaware, top-level).
pub(crate) fn json_to_slint_value(
    value: &serde_json::Value,
) -> Result<SlintValue, Box<dyn std::error::Error>> {
    json_to_slint_value_inner(value, None)
}

/// Convert JSON to Slint value with optional field-name context.
///
/// When `field_name` is provided and matches an image-field naming convention,
/// `null` values produce `SlintValue::Image(Image::default())` (0×0 empty image)
/// instead of `SlintValue::Void`, preventing Slint interpreter panics.
fn json_to_slint_value_inner(
    value: &serde_json::Value,
    field_name: Option<&str>,
) -> Result<SlintValue, Box<dyn std::error::Error>> {
    match value {
        serde_json::Value::Null => {
            // Image-typed fields must never be Void — Slint panics on .width/.height access.
            if field_name.map_or(false, is_image_field) {
                Ok(SlintValue::Image(slint::Image::default()))
            } else {
                Ok(SlintValue::Void)
            }
        }
        serde_json::Value::Bool(b) => Ok(SlintValue::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Ok(SlintValue::Number(f))
            } else {
                Err(format!("Invalid number: {}", n).into())
            }
        }
        serde_json::Value::String(s) => {
            Ok(SlintValue::String(slint::SharedString::from(s.as_str())))
        }
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<_>, _> = arr
                .iter()
                .map(|v| json_to_slint_value_inner(v, None))
                .collect();
            Ok(SlintValue::Model(Rc::new(VecModel::from(items?)).into()))
        }
        serde_json::Value::Object(obj) => {
            let fields: Result<Vec<_>, _> = obj
                .iter()
                .map(|(k, v)| {
                    json_to_slint_value_inner(v, Some(k.as_str())).map(|val| (k.clone(), val))
                })
                .collect();
            Ok(SlintValue::Struct(
                slint_interpreter::Struct::from_iter(fields?).into(),
            ))
        }
    }
}
