//! Port of `OQL.swift`.

/// A string literal for an OQL query: backslashes and single quotes are escaped.
pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}
