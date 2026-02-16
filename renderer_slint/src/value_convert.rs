use slint::VecModel;
use slint_interpreter::Value as SlintValue;
use std::rc::Rc;

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

/// Convert JSON to Slint value.
pub(crate) fn json_to_slint_value(
    value: &serde_json::Value,
) -> Result<SlintValue, Box<dyn std::error::Error>> {
    match value {
        serde_json::Value::Null => Ok(SlintValue::Void),
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
            let items: Result<Vec<_>, _> = arr.iter().map(json_to_slint_value).collect();
            Ok(SlintValue::Model(Rc::new(VecModel::from(items?)).into()))
        }
        serde_json::Value::Object(obj) => {
            let fields: Result<Vec<_>, _> = obj
                .iter()
                .map(|(k, v)| json_to_slint_value(v).map(|val| (k.clone(), val)))
                .collect();
            Ok(SlintValue::Struct(
                slint_interpreter::Struct::from_iter(fields?).into(),
            ))
        }
    }
}
