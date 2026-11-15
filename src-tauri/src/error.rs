//! What `get_snapshot` and `dispatch` reject with (contract.md §7). Everything iTop or validation
//! can say is a business result inside a snapshot or an intent's return value, never one of these.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error, ts_rs::TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/")]
pub enum AppError {
    /// A workspace model is addressed while signed out.
    #[error("{message}")]
    NotSignedIn { message: String },
    /// The model id isn't one the contract lists.
    #[error("{message}")]
    UnknownModel { message: String },
    /// The intent doesn't deserialize, or names something the model doesn't have.
    #[error("{message}")]
    InvalidIntent { message: String },
    /// A bug; the UI logs it.
    #[error("{message}")]
    Internal { message: String },
}

impl AppError {
    pub fn not_signed_in() -> Self {
        Self::NotSignedIn {
            message: "Not signed in.".into(),
        }
    }
    pub fn unknown_model(id: &str) -> Self {
        Self::UnknownModel {
            message: format!("No model named {id}."),
        }
    }
    pub fn invalid_intent(why: impl std::fmt::Display) -> Self {
        Self::InvalidIntent {
            message: why.to_string(),
        }
    }
    pub fn internal(why: impl std::fmt::Display) -> Self {
        Self::Internal {
            message: why.to_string(),
        }
    }
}
