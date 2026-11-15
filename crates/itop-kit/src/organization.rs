//! Port of `Organization.swift`: `Organization`, `Person`, `OrgQueries` and `OrgService`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{ITopClient, ITopObject};
use crate::config_item::{CMDBQueries, character_count};
use crate::current_user::Team;
use crate::error::{ITopError, is_swift_whitespace};
use crate::json::{JsonValue, swift_int};
use crate::oql;
use crate::rest_operation::RestOperation;
use crate::ticket::{Ticket, TicketClass};
use crate::ticket_queries::{TicketQueries, TicketScope, localized_standard_compare};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub code: String,
    /// Open tickets you can see for this org.
    #[ts(type = "number")]
    pub open_count: i64,
}

impl Organization {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        code: impl Into<String>,
        open_count: i64,
    ) -> Self {
        Organization {
            id: id.into(),
            name: name.into(),
            code: code.into(),
            open_count,
        }
    }
}

/// A Person from iTop. Someone in at least one team counts as an agent (DESIGN.md: People).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Person {
    pub id: String,
    pub display_name: String,
    pub org_id: String,
    pub org_name: String,
    pub email: String,
    pub phone: String,
    pub function: String,
    pub teams: Vec<Team>,
}

impl Person {
    pub const FIELDS: &'static [&'static str] = &[
        "name",
        "first_name",
        "org_id",
        "org_id_friendlyname",
        "email",
        "phone",
        "function",
        "team_list",
    ];

    pub fn is_agent(&self) -> bool {
        !self.teams.is_empty()
    }

    /// "Agent · Network, Service desk" or "Caller · Org A".
    pub fn subtitle(&self) -> String {
        if self.is_agent() {
            format!(
                "Agent · {}",
                self.teams
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else if self.org_name.is_empty() {
            "Caller".to_string()
        } else {
            format!("Caller · {}", self.org_name)
        }
    }

    /// The first letters of the first two words, uppercased.
    pub fn initials(&self) -> String {
        self.display_name
            .split(' ')
            .filter(|w| !w.is_empty())
            .take(2)
            .filter_map(|w| w.chars().next())
            .flat_map(char::to_uppercase)
            .collect()
    }

    /// `Person(object:)`.
    pub fn from_object(object: &ITopObject) -> Person {
        let f = &object.fields;
        let text = |k: &str| {
            f.get(k)
                .and_then(JsonValue::string_value)
                .unwrap_or("")
                .to_string()
        };
        let mut teams: Vec<Team> = Vec::new();
        if let Some(JsonValue::Array(links)) = f.get("team_list") {
            teams = links
                .iter()
                .filter_map(|l| {
                    let id = l.get("team_id").and_then(JsonValue::int_value)?;
                    let name = l
                        .get("team_name")
                        .and_then(JsonValue::string_value)
                        .map_or_else(|| format!("Team {id}"), str::to_string);
                    Some(Team::new(id.to_string(), name))
                })
                .collect();
        }
        let name = [text("first_name"), text("name")]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        Person {
            id: object.key.clone(),
            display_name: if name.is_empty() {
                format!("Person {}", object.key)
            } else {
                name
            },
            org_id: f
                .get("org_id")
                .and_then(JsonValue::int_value)
                .map_or_else(|| "0".to_string(), |n| n.to_string()),
            org_name: text("org_id_friendlyname"),
            email: text("email"),
            phone: text("phone"),
            function: text("function"),
            teams,
        }
    }
}

/// The OQL behind Organizations and People (PLAN.md Task 5b).
pub struct OrgQueries;

impl OrgQueries {
    pub const ORGANIZATIONS: &'static str = "SELECT Organization WHERE status = 'active'";

    pub fn open_tickets(cls: TicketClass, org_id: &str, scope: &TicketScope) -> Option<String> {
        swift_int(org_id).map(|n| {
            format!(
                "SELECT {} WHERE {} AND org_id = {n}",
                cls.raw_value(),
                scope.clause()
            )
        })
    }

    pub fn people(org_id: &str) -> Option<String> {
        swift_int(org_id).map(|n| format!("SELECT Person WHERE org_id = {n}"))
    }

    pub fn config_items(org_id: &str) -> Option<String> {
        CMDBQueries::in_org(org_id)
    }

    /// Every word must appear in the first or last name; `None` under 2 characters.
    pub fn people_search(text: &str) -> Option<String> {
        let q = text.trim_matches(is_swift_whitespace);
        if character_count(q) < 2 {
            return None;
        }
        let clauses: Vec<String> = q
            .split(' ')
            .filter(|w| !w.is_empty())
            .map(|word| {
                let w = oql::quote(&format!("%{word}%"));
                format!("(name LIKE {w} OR first_name LIKE {w})")
            })
            .collect();
        Some(format!("SELECT Person WHERE {}", clauses.join(" AND ")))
    }

    pub fn assigned_to(cls: TicketClass, person_id: &str, scope: &TicketScope) -> Option<String> {
        swift_int(person_id).map(|n| {
            format!(
                "SELECT {} WHERE {} AND agent_id = {n}",
                cls.raw_value(),
                scope.clause()
            )
        })
    }

    pub fn reported_by(cls: TicketClass, person_id: &str, scope: &TicketScope) -> Option<String> {
        swift_int(person_id).map(|n| {
            format!(
                "SELECT {} WHERE {} AND caller_id = {n}",
                cls.raw_value(),
                scope.clause()
            )
        })
    }
}

/// How many people and CIs an org has (Swift's `(people:configItems:)` tuple).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct OrgCounts {
    #[ts(type = "number")]
    pub people: i64,
    #[ts(type = "number")]
    pub config_items: i64,
}

/// The first people found and how many matched (Swift's `(people:total:)` tuple).
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PeoplePage {
    pub people: Vec<Person>,
    #[ts(type = "number")]
    pub total: i64,
}

/// Organizations and People reads. Read-only.
#[derive(Clone, Debug)]
pub struct OrgService {
    client: ITopClient,
}

impl OrgService {
    pub const PEOPLE_LIMIT: i64 = 100;

    pub fn new(client: ITopClient) -> Self {
        OrgService { client }
    }

    /// The orgs your account can see, with open-ticket counts. iTop's API can't count, so this loads
    /// every open ticket you can see with only its org id: one light query per ticket class.
    ///
    /// The org list loads alongside the counts; a failed count is reported before a failed list,
    /// as Swift awaits the counts first.
    pub async fn organizations(&self) -> Result<Vec<Organization>, ITopError> {
        let orgs = self.client.send(RestOperation::get(
            "Organization",
            OrgQueries::ORGANIZATIONS,
            &["name", "code"],
        ));
        let counts = async {
            let mut counts: HashMap<String, i64> = HashMap::new();
            for cls in TicketClass::ALL {
                let r = self
                    .client
                    .send(RestOperation::get(
                        cls.raw_value(),
                        format!("SELECT {} WHERE {}", cls.raw_value(), TicketQueries::OPEN),
                        &["org_id"],
                    ))
                    .await?;
                for o in r.objects() {
                    let org = o
                        .fields
                        .get("org_id")
                        .and_then(JsonValue::int_value)
                        .map_or_else(|| "0".to_string(), |n| n.to_string());
                    *counts.entry(org).or_insert(0) += 1;
                }
            }
            Ok::<_, ITopError>(counts)
        };
        let (orgs, counts) = tokio::join!(orgs, counts);
        let counts = counts?;
        let mut list: Vec<Organization> = orgs?
            .objects()
            .iter()
            .map(|o| {
                Organization::new(
                    o.key.clone(),
                    o.fields
                        .get("name")
                        .and_then(JsonValue::string_value)
                        .map_or_else(|| format!("Organization {}", o.key), str::to_string),
                    o.fields
                        .get("code")
                        .and_then(JsonValue::string_value)
                        .unwrap_or(""),
                    counts.get(&o.key).copied().unwrap_or(0),
                )
            })
            .collect();
        list.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(list)
    }

    /// An org's open tickets from every agent.
    pub async fn open_tickets(
        &self,
        org_id: &str,
        scope: &TicketScope,
    ) -> Result<Vec<Ticket>, ITopError> {
        self.tickets(
            TicketClass::ALL
                .into_iter()
                .filter_map(|cls| OrgQueries::open_tickets(cls, org_id, scope).map(|q| (cls, q)))
                .collect(),
        )
        .await
    }

    /// How many people and CIs an org has, for the org header's links.
    pub async fn counts(&self, org_id: &str) -> Result<OrgCounts, ITopError> {
        let (Some(p), Some(c)) = (OrgQueries::people(org_id), OrgQueries::config_items(org_id))
        else {
            return Ok(OrgCounts::default());
        };
        let (people, cis) = tokio::join!(
            self.client.send(RestOperation::get("Person", p, &["id"])),
            self.client
                .send(RestOperation::get("FunctionalCI", c, &["id"])),
        );
        let people = people?.objects().len() as i64;
        let config_items = cis?.objects().len() as i64;
        Ok(OrgCounts {
            people,
            config_items,
        })
    }

    pub async fn people_matching(&self, text: &str) -> Result<Vec<Person>, ITopError> {
        Ok(self.people_page_matching(text).await?.people)
    }

    /// The first people found (up to 100) and how many matched in total, so the app can say
    /// "Showing 100 of 256". iTop can't order them, so a narrower search is the way to the rest.
    pub async fn people_page_matching(&self, text: &str) -> Result<PeoplePage, ITopError> {
        let Some(oql) = OrgQueries::people_search(text) else {
            return Ok(PeoplePage::default());
        };
        self.people_page(oql).await
    }

    pub async fn people_page_in_org(&self, org_id: &str) -> Result<PeoplePage, ITopError> {
        let Some(oql) = OrgQueries::people(org_id) else {
            return Ok(PeoplePage::default());
        };
        self.people_page(oql).await
    }

    pub async fn people_in_org(&self, org_id: &str) -> Result<Vec<Person>, ITopError> {
        let Some(oql) = OrgQueries::people(org_id) else {
            return Ok(Vec::new());
        };
        Ok(self.people_page(oql).await?.people)
    }

    pub async fn tickets_assigned_to(
        &self,
        person_id: &str,
        scope: &TicketScope,
    ) -> Result<Vec<Ticket>, ITopError> {
        self.tickets(
            TicketClass::ALL
                .into_iter()
                .filter_map(|cls| OrgQueries::assigned_to(cls, person_id, scope).map(|q| (cls, q)))
                .collect(),
        )
        .await
    }

    pub async fn tickets_reported_by(
        &self,
        person_id: &str,
        scope: &TicketScope,
    ) -> Result<Vec<Ticket>, ITopError> {
        self.tickets(
            TicketClass::ALL
                .into_iter()
                .filter_map(|cls| OrgQueries::reported_by(cls, person_id, scope).map(|q| (cls, q)))
                .collect(),
        )
        .await
    }

    async fn people_page(&self, oql: String) -> Result<PeoplePage, ITopError> {
        let r = self
            .client
            .send(RestOperation::Get {
                class_name: "Person".into(),
                key: oql,
                output_fields: Person::FIELDS.iter().map(|s| s.to_string()).collect(),
                limit: Some(Self::PEOPLE_LIMIT),
            })
            .await?;
        let mut people: Vec<Person> = r
            .objects()
            .iter()
            .take(Self::PEOPLE_LIMIT as usize)
            .map(Person::from_object)
            .collect();
        people.sort_by(|a, b| localized_standard_compare(&a.display_name, &b.display_name));
        let count = people.len() as i64;
        Ok(PeoplePage {
            people,
            total: r.found().unwrap_or(count).max(count),
        })
    }

    async fn tickets(&self, queries: Vec<(TicketClass, String)>) -> Result<Vec<Ticket>, ITopError> {
        let mut all: Vec<Ticket> = Vec::new();
        for (cls, oql) in queries {
            let r = self
                .client
                .send(RestOperation::get(
                    cls.raw_value(),
                    oql,
                    Ticket::LIST_FIELDS,
                ))
                .await?;
            all.extend(r.objects().iter().filter_map(Ticket::from_object));
        }
        Ok(all)
    }
}
