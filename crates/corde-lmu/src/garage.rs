//! Bounds of the setup values, read from the game's garage data.
//!
//! `GET /rest/garage/getPlayerGarageData` describes the setup loaded in the
//! garage. According to the community it lists `VM_*` objects with the
//! current value, its label and the minimum / maximum index — **the exact
//! shape has not been seen yet** (no real answer available when this was
//! written; `corde-server` saves the raw answers in
//! `<data>/diagnostics/lmu-api/garage/` so that it can be checked on the
//! driver's PC).
//!
//! [`extract_bounds`] therefore accepts several plausible shapes and never
//! fails: whatever it does not recognise is simply absent from the result,
//! and the caller treats a missing bound as unknown.
//!
//! Recognised (anywhere in the document, at any depth):
//! * an object keyed by name: `{"VM_REAR_WING": {"minValue": 0, "maxValue": 20, "value": 5, "stringValue": "P6"}}`;
//! * an array of objects naming themselves: `[{"key": "RWSetting", "min": 0, "max": 20, "value": 5, "display": "P6"}]`;
//! * numbers written as strings (`"maxValue": "20"`).

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// What the garage says about one setting (every field optional).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyBound {
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Current index.
    pub value: Option<f64>,
    /// Label shown by the game for the current value.
    pub label: Option<String>,
}

impl KeyBound {
    /// `true` when the game says the value cannot move (`min == max`).
    pub fn is_fixed(&self) -> bool {
        matches!((self.min, self.max), (Some(a), Some(b)) if (a - b).abs() < 1e-9)
    }
}

/// Setting name (`VM_REAR_WING`, `RWSetting`…) → what the garage says.
pub type GarageBounds = BTreeMap<String, KeyBound>;

const MIN_KEYS: &[&str] = &[
    "minValue",
    "min",
    "minimum",
    "min_value",
    "minIndex",
    "rangeMin",
    "lo",
    "low",
];
const MAX_KEYS: &[&str] = &[
    "maxValue",
    "max",
    "maximum",
    "max_value",
    "maxIndex",
    "rangeMax",
    "hi",
    "high",
];
const VALUE_KEYS: &[&str] = &[
    "value",
    "currentValue",
    "current",
    "index",
    "val",
    "setting",
];
const LABEL_KEYS: &[&str] = &[
    "stringValue",
    "valueString",
    "displayValue",
    "display",
    "text",
    "label",
    "string",
    "formatted",
    "valueText",
];
const NAME_KEYS: &[&str] = &["key", "name", "id", "settingName", "svmKey", "vm"];

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn field<'a>(obj: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| {
        obj.get(*k).or_else(|| {
            obj.iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(k))
                .map(|(_, v)| v)
        })
    })
}

fn looks_like_name(s: &str) -> bool {
    s.starts_with("VM_") || s.ends_with("Setting")
}

fn walk(v: &Value, hint: Option<&str>, out: &mut GarageBounds) {
    match v {
        Value::Object(obj) => {
            let min = field(obj, MIN_KEYS).and_then(number);
            let max = field(obj, MAX_KEYS).and_then(number);
            let value = field(obj, VALUE_KEYS).and_then(number);
            let label = field(obj, LABEL_KEYS)
                .and_then(Value::as_str)
                .map(str::to_string);
            let own_name = field(obj, NAME_KEYS)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            let name = own_name.or(hint);
            let is_setting = min.is_some()
                || max.is_some()
                || (value.is_some() && name.is_some_and(looks_like_name));
            if let (true, Some(name)) = (is_setting, name) {
                let entry = out.entry(name.to_string()).or_default();
                entry.min = entry.min.or(min);
                entry.max = entry.max.or(max);
                entry.value = entry.value.or(value);
                entry.label = entry.label.take().or(label);
            }
            for (k, child) in obj {
                if child.is_object() || child.is_array() {
                    walk(child, Some(k), out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, None, out);
            }
        }
        _ => {}
    }
}

/// Every setting with a bound, a value or a label found in a garage document.
pub fn extract_bounds(doc: &Value) -> GarageBounds {
    let mut out = GarageBounds::new();
    walk(doc, None, &mut out);
    out
}

/// The bound of a setting by its `.svm` key, else by its `VM_*` name (both
/// compared exactly, then case-insensitively). `None` = unknown.
pub fn bound_for<'a>(
    bounds: &'a GarageBounds,
    svm_key: &str,
    rest_key: Option<&str>,
) -> Option<&'a KeyBound> {
    let exact = |k: &str| bounds.get(k);
    let loose = |k: &str| {
        bounds
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(k))
            .map(|(_, b)| b)
    };
    exact(svm_key)
        .or_else(|| rest_key.and_then(exact))
        .or_else(|| loose(svm_key))
        .or_else(|| rest_key.and_then(loose))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Every document below is an ASSUMED shape ("forme supposée"): no real
    // answer of /rest/garage/getPlayerGarageData was available when this was
    // written. They cover the shapes the community describes and a few
    // plausible variants; the real one is captured on the driver's PC.

    #[test]
    fn object_keyed_by_vm_names() {
        // Forme supposée n°1 : objets VM_* indexés par leur nom.
        let doc = json!({
            "VM_REAR_WING": {"minValue": 0, "maxValue": 20, "value": 5, "stringValue": "P6"},
            "VM_BRAKE_BALANCE": {"minValue": "0", "maxValue": "60", "value": "22", "stringValue": "51.5:48.5"},
            "VM_STEER_LOCK": {"value": 3, "stringValue": "540"}
        });
        let b = extract_bounds(&doc);
        let rw = &b["VM_REAR_WING"];
        assert_eq!(
            (rw.min, rw.max, rw.value),
            (Some(0.0), Some(20.0), Some(5.0))
        );
        assert_eq!(rw.label.as_deref(), Some("P6"));
        assert_eq!(b["VM_BRAKE_BALANCE"].max, Some(60.0));
        // A value without bounds is kept (the label is useful), bounds unknown.
        assert_eq!(b["VM_STEER_LOCK"].max, None);
        assert_eq!(b["VM_STEER_LOCK"].value, Some(3.0));
    }

    #[test]
    fn nested_arrays_of_named_settings() {
        // Forme supposée n°2 : tableau d'objets qui portent leur nom, imbriqué.
        let doc = json!({
            "garage": {"settings": [
                {"key": "RWSetting", "min": 0, "max": 15, "value": 8, "display": "+2"},
                {"name": "RearAntiSwaySetting", "minimum": 0, "maximum": 5, "current": 2},
                {"name": "Fixed", "min": 0, "max": 0, "value": 0},
                {"something": "else"}
            ]}
        });
        let b = extract_bounds(&doc);
        assert_eq!(b["RWSetting"].max, Some(15.0));
        assert_eq!(b["RearAntiSwaySetting"].max, Some(5.0));
        assert!(b["Fixed"].is_fixed());
        assert!(!b["RWSetting"].is_fixed());
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn unknown_shapes_give_nothing_instead_of_failing() {
        for doc in [
            json!(null),
            json!("text"),
            json!([1, 2, 3]),
            json!({"VM_X": 3}),
            json!({"a": {"b": {"c": true}}}),
        ] {
            assert!(extract_bounds(&doc).is_empty(), "{doc}");
        }
    }

    #[test]
    fn lookup_by_svm_key_then_rest_key() {
        let doc = json!({
            "VM_BRAKE_BALANCE": {"minValue": 0, "maxValue": 60},
            "rwsetting": {"min": 0, "max": 12}
        });
        let b = extract_bounds(&doc);
        assert_eq!(
            bound_for(&b, "RearBrakeSetting", Some("VM_BRAKE_BALANCE")).and_then(|k| k.max),
            Some(60.0)
        );
        assert_eq!(
            bound_for(&b, "RWSetting", None).and_then(|k| k.max),
            Some(12.0)
        );
        assert!(bound_for(&b, "CamberSetting", Some("VM_CAMBER")).is_none());
    }
}
