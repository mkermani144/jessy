use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

// Shared plumbing helpers for platform extraction modules.

/// Serializes selector slices for script-template replacement.
pub fn to_js_array(items: &[&str]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string())
}

/// Parses a JSON value into typed extraction payload.
pub fn parse_from_value<T>(value: Value, parse_context: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).context(parse_context.to_string())
}
