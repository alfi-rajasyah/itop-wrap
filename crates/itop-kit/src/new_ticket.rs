//! Port of `NewTicket.swift`: `NewTicket`, `TicketWrites::create`, `PickLists` and the
//! `ServiceCatalog::services_for_org` extension.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::ITopClient;
use crate::error::ITopError;
use crate::json::{JsonObject, JsonValue};
use crate::rest_operation::RestOperation;
use crate::ticket::{Ticket, TicketClass};
use crate::ticket_lifecycle::{ServiceCatalog, ServiceRef};
use crate::ticket_writes::{
    COMMENT, Failure, TicketWrites, WriteError, friendly_name, localized_standard_compare,
};

/// What New ticket sends (PLAN.md Task 9; UX-FLOWS F7: organization first).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct NewTicket {
    pub ticket_class: TicketClass,
    pub org_id: String,
    pub caller_id: String,
    pub service_id: String,
    pub subcategory_id: String,
    pub impact: String,
    pub urgency: String,
    pub title: String,
    pub description: String,
    /// Set when the ticket starts from a config item ("New ticket on this CI").
    pub ci_id: Option<String>,
}

impl NewTicket {
    /// Impact and urgency start at iTop's own defaults (1 and 4), like its web form; everything
    /// else empty (Swift's init with its default arguments).
    pub fn new(ticket_class: TicketClass) -> Self {
        NewTicket {
            ticket_class,
            org_id: String::new(),
            caller_id: String::new(),
            service_id: String::new(),
            subcategory_id: String::new(),
            impact: "1".into(),
            urgency: "4".into(),
            title: String::new(),
            description: String::new(),
            ci_id: None,
        }
    }

    /// The description as iTop's HTML: markup escaped, paragraphs kept, line breaks as <br>.
    pub fn html(text: &str) -> String {
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;");
        escaped
            .split("\n\n")
            .map(|p| format!("<p>{}</p>", p.replace('\n', "<br>")))
            .collect()
    }
}

impl TicketWrites {
    /// Creates the ticket and reads it back, so the app can open it.
    pub async fn create(&self, t: &NewTicket) -> Result<Ticket, WriteError> {
        for (value, name) in [
            (&t.org_id, "organization"),
            (&t.caller_id, "caller"),
            (&t.service_id, "service"),
            (&t.subcategory_id, "subcategory"),
        ] {
            if !Self::is_id(value) {
                return Err(Failure::Missing(name.into()).into());
            }
        }
        let title = t.title.trim();
        if title.is_empty() {
            return Err(Failure::Missing("title".into()).into());
        }
        let description = t.description.trim();
        if description.is_empty() {
            return Err(Failure::Missing("description".into()).into());
        }
        let mut fields = JsonObject::new();
        fields.insert("org_id".into(), t.org_id.as_str().into());
        fields.insert("caller_id".into(), t.caller_id.as_str().into());
        fields.insert("service_id".into(), t.service_id.as_str().into());
        fields.insert(
            "servicesubcategory_id".into(),
            t.subcategory_id.as_str().into(),
        );
        fields.insert("impact".into(), t.impact.as_str().into());
        fields.insert("urgency".into(), t.urgency.as_str().into());
        fields.insert("title".into(), title.into());
        fields.insert("description".into(), NewTicket::html(description).into());
        if let Some(ci) = &t.ci_id
            && Self::is_id(ci)
        {
            let mut link = JsonObject::new();
            link.insert("functionalci_id".into(), ci.as_str().into());
            fields.insert(
                "functionalcis_list".into(),
                JsonValue::Array(vec![JsonValue::Object(link)]),
            );
        }
        let cls = t.ticket_class.raw_value();
        let response = self
            .client
            .send(RestOperation::create(cls, fields, COMMENT))
            .await?;
        Self::check(&response)?;
        let key = response.objects().first().map(|o| o.key.clone());
        let Some(key) = key.filter(|k| Self::is_id(k)) else {
            return Err(ITopError::Server {
                code: 0,
                message: "iTop didn't return the new ticket.".into(),
            }
            .into());
        };
        self.client.allow_created_ticket(&format!("{cls}::{key}"));
        let read = self
            .client
            .send(RestOperation::Get {
                class_name: cls.into(),
                key: format!("SELECT {cls} WHERE id = {key}"),
                output_fields: Ticket::LIST_FIELDS.iter().map(|s| s.to_string()).collect(),
                limit: Some(1),
            })
            .await?;
        match read.objects().first().and_then(Ticket::from_object) {
            Some(ticket) => Ok(ticket),
            None => Err(ITopError::Server {
                code: 0,
                message:
                    "The ticket was created, but the app couldn't read it back. Refresh to see it."
                        .into(),
            }
            .into()),
        }
    }
}

/// Lists for New ticket, read from iTop. Impact and urgency can't be read through REST, so they
/// use iTop's standard values; a customised list shows up as iTop refusing the value.
#[derive(Clone, Debug)]
pub struct PickLists {
    client: ITopClient,
}

impl PickLists {
    pub const ORGANIZATIONS_OQL: &'static str = "SELECT Organization WHERE status = 'active'";

    pub fn new(client: ITopClient) -> Self {
        PickLists { client }
    }

    pub fn callers_oql(org_id: &str) -> Option<String> {
        TicketWrites::is_id(org_id)
            .then(|| format!("SELECT Person WHERE org_id = {org_id} AND status = 'active'"))
    }

    pub fn impacts() -> Vec<ServiceRef> {
        vec![
            ServiceRef::new("1", "A department"),
            ServiceRef::new("2", "A service"),
            ServiceRef::new("3", "A person"),
        ]
    }

    pub fn urgencies() -> Vec<ServiceRef> {
        vec![
            ServiceRef::new("1", "Critical"),
            ServiceRef::new("2", "High"),
            ServiceRef::new("3", "Medium"),
            ServiceRef::new("4", "Low"),
        ]
    }

    pub async fn organizations(&self) -> Result<Vec<ServiceRef>, ITopError> {
        self.refs("Organization", Self::ORGANIZATIONS_OQL).await
    }

    pub async fn callers(&self, org_id: &str) -> Result<Vec<ServiceRef>, ITopError> {
        let Some(oql) = Self::callers_oql(org_id) else {
            return Ok(Vec::new());
        };
        self.refs("Person", &oql).await
    }

    async fn refs(&self, class_name: &str, oql: &str) -> Result<Vec<ServiceRef>, ITopError> {
        let response = self
            .client
            .send(RestOperation::get(class_name, oql, &["friendlyname"]))
            .await?;
        let mut refs: Vec<ServiceRef> = response
            .objects()
            .iter()
            .map(|o| ServiceRef::new(o.key.clone(), friendly_name(o, class_name)))
            .collect();
        refs.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(refs)
    }
}

impl ServiceCatalog {
    /// The services an organization's contracts cover, every live service when there are none.
    pub async fn services_for_org(&self, org_id: &str) -> Result<Vec<ServiceRef>, ITopError> {
        // Swift builds an empty probe ticket carrying only the org id; `services` reads nothing else.
        let probe = Ticket {
            key: "0".into(),
            ticket_class: TicketClass::UserRequest,
            r#ref: String::new(),
            title: String::new(),
            status: String::new(),
            priority: 0,
            caller_name: String::new(),
            org_name: String::new(),
            org_id: org_id.into(),
            team_id: "0".into(),
            team_name: String::new(),
            agent_id: "0".into(),
            agent_name: String::new(),
            start_date: None,
            last_update: None,
        };
        self.services(&probe).await
    }
}
