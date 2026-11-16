//! Port of `RESTOperation.swift`.

use crate::json::{JsonObject, JsonValue};

/// One iTop REST operation, encoded as the `json_data` form field.
#[derive(Clone, PartialEq)]
pub enum RestOperation {
    ListOperations,
    CheckCredentials {
        user: String,
        password: String,
    },
    /// `key` is an OQL query, an object id, or a search criteria string.
    Get {
        class_name: String,
        key: String,
        output_fields: Vec<String>,
        limit: Option<i64>,
    },
    /// Objects linked through a relation, e.g. "impacts" up (what this depends on) or down (what uses it).
    GetRelated {
        class_name: String,
        key: String,
        relation: String,
        depth: i64,
        direction: String,
    },
    /// `output_fields` are the fields iTop sends back for the new object (an InlineImage's `secret`).
    Create {
        class_name: String,
        fields: JsonObject,
        comment: String,
        output_fields: Vec<String>,
    },
    Update {
        class_name: String,
        key: String,
        fields: JsonObject,
        comment: String,
    },
    ApplyStimulus {
        class_name: String,
        key: String,
        stimulus: String,
        fields: JsonObject,
        comment: String,
    },
}

/// Written by hand so the password in `CheckCredentials` can never be printed.
impl std::fmt::Debug for RestOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestOperation::ListOperations => f.write_str("ListOperations"),
            RestOperation::CheckCredentials { user, .. } => f
                .debug_struct("CheckCredentials")
                .field("user", user)
                .field("password", &"<redacted>")
                .finish(),
            RestOperation::Get {
                class_name,
                key,
                output_fields,
                limit,
            } => f
                .debug_struct("Get")
                .field("class_name", class_name)
                .field("key", key)
                .field("output_fields", output_fields)
                .field("limit", limit)
                .finish(),
            RestOperation::GetRelated {
                class_name,
                key,
                relation,
                depth,
                direction,
            } => f
                .debug_struct("GetRelated")
                .field("class_name", class_name)
                .field("key", key)
                .field("relation", relation)
                .field("depth", depth)
                .field("direction", direction)
                .finish(),
            RestOperation::Create {
                class_name,
                fields,
                comment,
                output_fields,
            } => f
                .debug_struct("Create")
                .field("class_name", class_name)
                .field("fields", fields)
                .field("comment", comment)
                .field("output_fields", output_fields)
                .finish(),
            RestOperation::Update {
                class_name,
                key,
                fields,
                comment,
            } => f
                .debug_struct("Update")
                .field("class_name", class_name)
                .field("key", key)
                .field("fields", fields)
                .field("comment", comment)
                .finish(),
            RestOperation::ApplyStimulus {
                class_name,
                key,
                stimulus,
                fields,
                comment,
            } => f
                .debug_struct("ApplyStimulus")
                .field("class_name", class_name)
                .field("key", key)
                .field("stimulus", stimulus)
                .field("fields", fields)
                .field("comment", comment)
                .finish(),
        }
    }
}

impl RestOperation {
    /// `.get(className:key:outputFields:)` with no limit.
    pub fn get(
        class_name: impl Into<String>,
        key: impl Into<String>,
        output_fields: &[&str],
    ) -> Self {
        RestOperation::Get {
            class_name: class_name.into(),
            key: key.into(),
            output_fields: output_fields.iter().map(|s| s.to_string()).collect(),
            limit: None,
        }
    }

    /// `.create(className:fields:comment:)` with the default `outputFields: ["id"]`.
    pub fn create(
        class_name: impl Into<String>,
        fields: JsonObject,
        comment: impl Into<String>,
    ) -> Self {
        RestOperation::Create {
            class_name: class_name.into(),
            fields,
            comment: comment.into(),
            output_fields: vec!["id".to_string()],
        }
    }

    /// Writes are refused by `ITopClient` unless it was created with `allows_writes: true`.
    pub fn is_write(&self) -> bool {
        match self {
            RestOperation::ListOperations
            | RestOperation::CheckCredentials { .. }
            | RestOperation::Get { .. }
            | RestOperation::GetRelated { .. } => false,
            RestOperation::Create { .. }
            | RestOperation::Update { .. }
            | RestOperation::ApplyStimulus { .. } => true,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            RestOperation::ListOperations => "list_operations",
            RestOperation::CheckCredentials { .. } => "core/check_credentials",
            RestOperation::Get { .. } => "core/get",
            RestOperation::GetRelated { .. } => "core/get_related",
            RestOperation::Create { .. } => "core/create",
            RestOperation::Update { .. } => "core/update",
            RestOperation::ApplyStimulus { .. } => "core/apply_stimulus",
        }
    }

    /// The `json_data` value.
    pub fn json(&self) -> JsonValue {
        let mut o = JsonObject::new();
        o.insert("operation".into(), self.name().into());
        let s = |v: &str| JsonValue::String(v.to_string());
        match self {
            RestOperation::ListOperations => {}
            RestOperation::CheckCredentials { user, password } => {
                o.insert("user".into(), s(user));
                o.insert("password".into(), s(password));
            }
            RestOperation::Get {
                class_name,
                key,
                output_fields,
                limit,
            } => {
                o.insert("class".into(), s(class_name));
                o.insert("key".into(), s(key));
                o.insert("output_fields".into(), s(&output_fields.join(",")));
                if let Some(limit) = limit {
                    o.insert("limit".into(), JsonValue::String(limit.to_string()));
                }
            }
            RestOperation::GetRelated {
                class_name,
                key,
                relation,
                depth,
                direction,
            } => {
                o.insert("class".into(), s(class_name));
                o.insert("key".into(), s(key));
                o.insert("relation".into(), s(relation));
                o.insert("depth".into(), JsonValue::Number(*depth as f64));
                o.insert("direction".into(), s(direction));
                o.insert("redundancy".into(), JsonValue::Bool(false));
            }
            RestOperation::Create {
                class_name,
                fields,
                comment,
                output_fields,
            } => {
                o.insert("class".into(), s(class_name));
                o.insert("fields".into(), JsonValue::Object(fields.clone()));
                o.insert("comment".into(), s(comment));
                o.insert("output_fields".into(), s(&output_fields.join(",")));
            }
            RestOperation::Update {
                class_name,
                key,
                fields,
                comment,
            } => {
                o.insert("class".into(), s(class_name));
                o.insert("key".into(), s(key));
                o.insert("fields".into(), JsonValue::Object(fields.clone()));
                o.insert("comment".into(), s(comment));
                o.insert("output_fields".into(), s("id"));
            }
            RestOperation::ApplyStimulus {
                class_name,
                key,
                stimulus,
                fields,
                comment,
            } => {
                o.insert("class".into(), s(class_name));
                o.insert("key".into(), s(key));
                o.insert("stimulus".into(), s(stimulus));
                o.insert("fields".into(), JsonValue::Object(fields.clone()));
                o.insert("comment".into(), s(comment));
                o.insert("output_fields".into(), s("id"));
            }
        }
        JsonValue::Object(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_credentials_debug_hides_the_password() {
        let op = RestOperation::CheckCredentials {
            user: "agent".into(),
            password: "hunter2".into(),
        };
        let text = format!("{op:?}");
        assert!(text.contains("agent"), "{text}");
        assert!(!text.contains("hunter2"), "{text}");
    }

    #[test]
    fn keychain_errors_read_like_the_swift_app() {
        let e = crate::KeychainError { status: -25293 };
        assert_eq!(
            e.to_string(),
            "The operation couldn\u{2019}t be completed. (ITopKit.KeychainError error 1.)"
        );
    }
}
