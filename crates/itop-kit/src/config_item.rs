//! Port of `ConfigItem.swift`: `ConfigItem`, `CIRef`, `CIField`, `ConfigItemDetail`,
//! `CMDBQueries` and `CMDBService`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::case_log::HtmlText;
use crate::client::{ITopClient, ITopResponse};
use crate::error::{ITopError, is_swift_whitespace};
use crate::json::{JsonObject, JsonValue, swift_int};
use crate::oql;
use crate::rest_operation::RestOperation;
use crate::ticket::{Ticket, TicketClass};
use crate::ticket_queries::localized_standard_compare;

/// Swift's `Character.isUppercase`: changes when lowercased, not when uppercased.
pub(crate) fn swift_is_uppercase(c: char) -> bool {
    !c.to_lowercase().eq(std::iter::once(c)) && c.to_uppercase().eq(std::iter::once(c))
}

/// Swift's `Character.isLowercase`: changes when uppercased, not when lowercased.
pub(crate) fn swift_is_lowercase(c: char) -> bool {
    !c.to_uppercase().eq(std::iter::once(c)) && c.to_lowercase().eq(std::iter::once(c))
}

/// How many characters a query has, for the 2-character minimum. Swift counts grapheme clusters;
/// this counts code points without combining marks, which agrees for all but exotic clusters
/// (emoji sequences, Hangul jamo).
pub(crate) fn character_count(s: &str) -> usize {
    s.chars()
        .filter(|c| {
            !matches!(*c as u32,
                0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF
                | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x200D)
        })
        .count()
}

/// A CI in search results. `class_name` is the real class (Server, NetworkDevice…), not FunctionalCI.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ConfigItem {
    pub key: String,
    pub class_name: String,
    pub name: String,
    pub org_name: String,
}

impl ConfigItem {
    pub fn new(
        key: impl Into<String>,
        class_name: impl Into<String>,
        name: impl Into<String>,
        org_name: impl Into<String>,
    ) -> Self {
        ConfigItem {
            key: key.into(),
            class_name: class_name.into(),
            name: name.into(),
            org_name: org_name.into(),
        }
    }

    /// `Identifiable.id`: "Server::5".
    pub fn id(&self) -> String {
        format!("{}::{}", self.class_name, self.key)
    }

    /// "ApplicationSolution" → "Application solution", "PC" stays "PC".
    pub fn class_label(&self) -> String {
        Self::label_for_class(&self.class_name)
    }

    pub fn label_for_class(name: &str) -> String {
        let mut words: Vec<String> = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = name.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            let next_is_lower = chars.get(i + 1).is_some_and(|n| swift_is_lowercase(*n));
            if swift_is_uppercase(c)
                && let Some(last) = current.chars().last()
                && (swift_is_lowercase(last) || next_is_lower)
            {
                words.push(std::mem::take(&mut current));
            }
            current.push(c);
        }
        if !current.is_empty() {
            words.push(current);
        }
        words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                if i == 0 || w.chars().all(swift_is_uppercase) {
                    w.clone()
                } else {
                    w.to_lowercase()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A CI on the other end of a relationship.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct CIRef {
    pub id: String,
    pub name: String,
    pub class_name: String,
}

impl CIRef {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        class_name: impl Into<String>,
    ) -> Self {
        CIRef {
            id: id.into(),
            name: name.into(),
            class_name: class_name.into(),
        }
    }

    pub fn item(&self) -> ConfigItem {
        ConfigItem::new(
            self.id.clone(),
            self.class_name.clone(),
            self.name.clone(),
            "",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct CIField {
    pub label: String,
    pub value: String,
}

impl CIField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        CIField {
            label: label.into(),
            value: value.into(),
        }
    }

    /// `Identifiable.id`.
    pub fn id(&self) -> &str {
        &self.label
    }
}

/// The CI detail pane: main fields, relationships and open tickets (DESIGN.md, Config item detail).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ConfigItemDetail {
    pub item: ConfigItem,
    pub fields: Vec<CIField>,
    pub description: String,
    pub depends_on: Vec<CIRef>,
    pub used_by: Vec<CIRef>,
    pub open_tickets: Vec<Ticket>,
}

struct Shown {
    keys: &'static [&'static str],
    label: &'static str,
    humanize: bool,
}

impl ConfigItemDetail {
    /// The fields worth showing, in this order, when the class has them and they're filled in.
    const SHOWN: &'static [Shown] = &[
        Shown {
            keys: &["status"],
            label: "Status",
            humanize: true,
        },
        Shown {
            keys: &["org_id_friendlyname", "organization_name"],
            label: "Organization",
            humanize: false,
        },
        Shown {
            keys: &["business_criticity"],
            label: "Business criticality",
            humanize: true,
        },
        Shown {
            keys: &["location_id_friendlyname", "location_name"],
            label: "Location",
            humanize: false,
        },
        Shown {
            keys: &["brand_id_friendlyname", "brand_name"],
            label: "Brand",
            humanize: false,
        },
        Shown {
            keys: &["model_id_friendlyname", "model_name"],
            label: "Model",
            humanize: false,
        },
        Shown {
            keys: &["osfamily_id_friendlyname", "osfamily_name"],
            label: "OS family",
            humanize: false,
        },
        Shown {
            keys: &["osversion_id_friendlyname", "osversion_name"],
            label: "OS version",
            humanize: false,
        },
        Shown {
            keys: &["serialnumber"],
            label: "Serial number",
            humanize: false,
        },
        Shown {
            keys: &["asset_number"],
            label: "Asset number",
            humanize: false,
        },
        Shown {
            keys: &["managementip"],
            label: "Management IP",
            humanize: false,
        },
        Shown {
            keys: &["cpu"],
            label: "CPU",
            humanize: false,
        },
        Shown {
            keys: &["ram"],
            label: "RAM",
            humanize: false,
        },
        Shown {
            keys: &["move2production"],
            label: "In production since",
            humanize: false,
        },
    ];

    pub fn fields(f: &JsonObject) -> Vec<CIField> {
        let mut out: Vec<CIField> = Self::SHOWN
            .iter()
            .filter_map(|entry| {
                let value = entry
                    .keys
                    .iter()
                    .filter_map(|k| f.get(*k).and_then(JsonValue::string_value))
                    .find(|v| !v.is_empty() && *v != "0")?;
                Some(CIField::new(
                    entry.label,
                    if entry.humanize {
                        Self::humanize(value)
                    } else {
                        value.to_string()
                    },
                ))
            })
            .collect();
        if let Some(JsonValue::Array(contacts)) = f.get("contacts_list") {
            let names: Vec<&str> = contacts
                .iter()
                .filter_map(|c| {
                    c.get("contact_id_friendlyname")
                        .and_then(JsonValue::string_value)
                })
                .filter(|n| !n.is_empty())
                .collect();
            if !names.is_empty() {
                out.push(CIField::new("Contacts", names.join(", ")));
            }
        }
        out
    }

    /// iTop codes as words: "not_applicable" → "Not applicable".
    pub fn humanize(code: &str) -> String {
        let s = code.replace('_', " ");
        let mut chars = s.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => String::new(),
        }
    }
}

pub struct CMDBQueries;

impl CMDBQueries {
    pub const SEARCH_LIMIT: i64 = 50;

    /// CIs whose name contains the text, or `None` for fewer than 2 characters.
    pub fn search(text: &str) -> Option<String> {
        let q = text.trim_matches(is_swift_whitespace);
        if character_count(q) < 2 {
            return None;
        }
        Some(format!(
            "SELECT FunctionalCI WHERE name LIKE {}",
            oql::quote(&format!("%{q}%"))
        ))
    }

    /// All CIs of one organization.
    pub fn in_org(org_id: &str) -> Option<String> {
        swift_int(org_id).map(|n| format!("SELECT FunctionalCI WHERE org_id = {n}"))
    }

    /// Open tickets linked to a CI through lnkFunctionalCIToTicket.
    pub fn open_tickets(cls: TicketClass, ci_id: &str) -> Option<String> {
        let id = swift_int(ci_id)?;
        Some(format!(
            "SELECT {} AS t JOIN lnkFunctionalCIToTicket AS l ON l.ticket_id = t.id WHERE l.functionalci_id = {id} AND t.status NOT IN ('resolved','closed')",
            cls.raw_value()
        ))
    }
}

/// Swift's `CMDBService.Direction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn raw_value(self) -> &'static str {
        match self {
            Direction::Up => "up",
            Direction::Down => "down",
        }
    }
}

/// CMDB reads (PLAN.md Task 5). Read-only.
#[derive(Clone, Debug)]
pub struct CMDBService {
    client: ITopClient,
}

impl CMDBService {
    pub fn new(client: ITopClient) -> Self {
        CMDBService { client }
    }

    pub async fn search(&self, text: &str) -> Result<Vec<ConfigItem>, ITopError> {
        let Some(oql) = CMDBQueries::search(text) else {
            return Ok(Vec::new());
        };
        self.items(oql).await
    }

    /// The CIs of one organization (first 50), for the org view's Config items link.
    pub async fn items_in_org(&self, org_id: &str) -> Result<Vec<ConfigItem>, ITopError> {
        let Some(oql) = CMDBQueries::in_org(org_id) else {
            return Ok(Vec::new());
        };
        self.items(oql).await
    }

    async fn items(&self, oql: String) -> Result<Vec<ConfigItem>, ITopError> {
        let response = self
            .client
            .send(RestOperation::Get {
                class_name: "FunctionalCI".into(),
                key: oql,
                output_fields: ["name", "finalclass", "org_id_friendlyname"]
                    .map(String::from)
                    .to_vec(),
                limit: Some(CMDBQueries::SEARCH_LIMIT),
            })
            .await?;
        let mut items: Vec<ConfigItem> = response
            .objects()
            .iter()
            .take(CMDBQueries::SEARCH_LIMIT as usize)
            .map(|o| {
                let text = |k: &str| o.fields.get(k).and_then(JsonValue::string_value);
                ConfigItem::new(
                    o.key.clone(),
                    text("finalclass").unwrap_or(&o.class_name),
                    text("name").unwrap_or(""),
                    text("org_id_friendlyname").unwrap_or(""),
                )
            })
            .collect();
        items.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(items)
    }

    /// The CI with its fields, both directions of "impacts" and its open tickets, loaded together.
    /// The relationships are best effort (a failure shows none); the fields and tickets aren't.
    pub async fn detail(&self, item: &ConfigItem) -> Result<ConfigItemDetail, ITopError> {
        let (full, up, down, tickets) = tokio::join!(
            self.client.send(RestOperation::get(
                item.class_name.clone(),
                item.key.clone(),
                &["*"]
            )),
            self.related(&item.class_name, &item.key, Direction::Up),
            self.related(&item.class_name, &item.key, Direction::Down),
            self.open_tickets(&item.key),
        );
        let objects = full?.objects();
        let empty = JsonObject::new();
        let f = objects.first().map_or(&empty, |o| &o.fields);
        let text = |k: &str| f.get(k).and_then(JsonValue::string_value);
        let resolved = ConfigItem::new(
            item.key.clone(),
            item.class_name.clone(),
            text("name").unwrap_or(&item.name),
            text("org_id_friendlyname").unwrap_or(&item.org_name),
        );
        Ok(ConfigItemDetail {
            item: resolved,
            fields: ConfigItemDetail::fields(f),
            description: HtmlText::plain(text("description").unwrap_or("")),
            depends_on: up.unwrap_or_default(),
            used_by: down.unwrap_or_default(),
            open_tickets: tickets?,
        })
    }

    /// Direct "impacts" neighbours: `Up` gives what this CI depends on, `Down` what uses it.
    pub async fn related(
        &self,
        class_name: &str,
        key: &str,
        direction: Direction,
    ) -> Result<Vec<CIRef>, ITopError> {
        let response = self
            .client
            .send(RestOperation::GetRelated {
                class_name: class_name.to_string(),
                key: key.to_string(),
                relation: "impacts".into(),
                depth: 1,
                direction: direction.raw_value().into(),
            })
            .await?;
        Ok(Self::related_items(&response, class_name, key))
    }

    const NOT_CIS: &'static [&'static str] = &[
        "Person",
        "Team",
        "Contact",
        "Organization",
        "Location",
        "UserRequest",
        "Incident",
        "Problem",
        "Change",
        "NormalChange",
        "RoutineChange",
        "EmergencyChange",
        "Document",
    ];

    pub fn related_items(response: &ITopResponse, class_name: &str, key: &str) -> Vec<CIRef> {
        let mut refs: Vec<CIRef> = response
            .objects()
            .iter()
            .filter(|o| {
                let is_self = o.class_name == class_name && o.key == key;
                !(is_self
                    || Self::NOT_CIS.contains(&o.class_name.as_str())
                    || o.class_name.starts_with("Document"))
            })
            .map(|o| {
                let text = |k: &str| o.fields.get(k).and_then(JsonValue::string_value);
                CIRef::new(
                    o.key.clone(),
                    text("friendlyname")
                        .or_else(|| text("name"))
                        .map_or_else(|| format!("{} {}", o.class_name, o.key), str::to_string),
                    o.class_name.clone(),
                )
            })
            .collect();
        refs.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        refs
    }

    pub async fn open_tickets(&self, ci_id: &str) -> Result<Vec<Ticket>, ITopError> {
        let mut all: Vec<Ticket> = Vec::new();
        for cls in TicketClass::ALL {
            let Some(oql) = CMDBQueries::open_tickets(cls, ci_id) else {
                continue;
            };
            let response = self
                .client
                .send(RestOperation::get(
                    cls.raw_value(),
                    oql,
                    Ticket::LIST_FIELDS,
                ))
                .await?;
            all.extend(response.objects().iter().filter_map(Ticket::from_object));
        }
        Ok(all)
    }
}
