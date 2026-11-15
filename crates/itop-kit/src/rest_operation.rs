//! Port of `RESTOperation.swift`.

use crate::json::{JsonObject, JsonValue};

/// One iTop REST operation, encoded as the `json_data` form field.
#[derive(Clone, Debug, PartialEq)]
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
