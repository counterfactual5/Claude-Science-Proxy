//! Backend JSON payloads for localizable frontend copy (`hint_key` / `i18n` errors).

use serde_json::{json, Value};

pub(crate) fn hint_payload(key: &str, vars: Value) -> Value {
    json!({
        "hint_key": key,
        "hint_vars": vars,
    })
}

pub(crate) fn i18n_err(key: &str, vars: Value) -> String {
    serde_json::to_string(&json!({ "i18n": key, "vars": vars })).unwrap_or_else(|_| key.to_string())
}
