//! Port of `TicketDetail.swift`: `LinkedCI`, `TicketDetail` and `TicketService::detail`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::case_log::{CaseLog, HtmlText};
use crate::client::ITopObject;
use crate::error::ITopError;
use crate::json::JsonValue;
use crate::rest_operation::RestOperation;
use crate::ticket::Ticket;
use crate::ticket_queries::TicketService;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct LinkedCI {
    pub id: String,
    pub name: String,
    /// The CI's real class (NetworkDevice, Server…), for opening it in iTop.
    pub class_name: String,
}

impl LinkedCI {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        class_name: impl Into<String>,
    ) -> Self {
        LinkedCI {
            id: id.into(),
            name: name.into(),
            class_name: class_name.into(),
        }
    }
}

/// What the detail pane adds to a list row: description, both logs and linked CIs.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TicketDetail {
    pub description: String,
    /// The description's HTML, for links and images (`RichText`).
    pub description_html: String,
    pub public_log: Vec<crate::case_log::LogEntry>,
    pub private_log: Vec<crate::case_log::LogEntry>,
    pub linked_cis: Vec<LinkedCI>,
    pub service_name: String,
    pub subcategory_name: String,
    /// Ids behind the names, to prefill Resolve. "0" or empty when not set.
    pub service_id: String,
    pub subcategory_id: String,
}

impl TicketDetail {
    pub const FIELDS: &'static [&'static str] = &[
        "description",
        "public_log",
        "private_log",
        "functionalcis_list",
        "service_id",
        "service_id_friendlyname",
        "servicesubcategory_id",
        "servicesubcategory_id_friendlyname",
        "last_update",
    ];

    /// `TicketDetail(object:)`.
    pub fn from_object(object: &ITopObject) -> TicketDetail {
        let f = &object.fields;
        let text = |k: &str| f.get(k).and_then(JsonValue::string_value);
        let mut cis: Vec<LinkedCI> = Vec::new();
        if let Some(JsonValue::Array(links)) = f.get("functionalcis_list") {
            cis = links
                .iter()
                .filter_map(|link| {
                    let id = link.get("functionalci_id").and_then(JsonValue::int_value)?;
                    let name = link
                        .get("functionalci_id_friendlyname")
                        .and_then(JsonValue::string_value)
                        .or_else(|| {
                            link.get("functionalci_name")
                                .and_then(JsonValue::string_value)
                        })
                        .map_or_else(|| format!("CI {id}"), str::to_string);
                    let class_name = link
                        .get("functionalci_id_finalclass_recall")
                        .and_then(JsonValue::string_value)
                        .unwrap_or("FunctionalCI");
                    Some(LinkedCI::new(id.to_string(), name, class_name))
                })
                .collect();
        }
        // A string id as it is, else a numeric id as digits, else "".
        let ident = |k: &str| {
            text(k).map(str::to_string).unwrap_or_else(|| {
                f.get(k)
                    .and_then(JsonValue::int_value)
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            })
        };
        TicketDetail {
            description: HtmlText::plain(text("description").unwrap_or("")),
            description_html: text("description").unwrap_or("").to_string(),
            public_log: CaseLog::entries(f.get("public_log")),
            private_log: CaseLog::entries(f.get("private_log")),
            linked_cis: cis,
            service_name: text("service_id_friendlyname").unwrap_or("").to_string(),
            subcategory_name: text("servicesubcategory_id_friendlyname")
                .unwrap_or("")
                .to_string(),
            service_id: ident("service_id"),
            subcategory_id: ident("servicesubcategory_id"),
        }
    }
}

impl TicketService {
    /// The full ticket for the detail pane. Read-only.
    pub async fn detail(&self, ticket: &Ticket) -> Result<TicketDetail, ITopError> {
        let response = self
            .client
            .send(RestOperation::get(
                ticket.ticket_class.raw_value(),
                ticket.key.clone(),
                TicketDetail::FIELDS,
            ))
            .await?;
        let Some(object) = response.objects().into_iter().next() else {
            return Err(ITopError::Server {
                code: 0,
                message: format!(
                    "{} wasn't returned. It may have been deleted, or your account can't see it any more.",
                    ticket.r#ref
                ),
            });
        };
        Ok(TicketDetail::from_object(&object))
    }
}
