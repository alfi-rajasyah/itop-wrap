//! Port of `CurrentUser.swift`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::ITopClient;
use crate::error::ITopError;
use crate::json::JsonValue;
use crate::oql;
use crate::rest_operation::RestOperation;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Team {
    pub id: String,
    pub name: String,
}

impl Team {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Team {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Who is signed in: their iTop User, the Person behind it, and their teams.
/// "My tickets" filter on `contact_id`; "needs you" and Team tickets use `teams`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct CurrentUser {
    pub user_id: String,
    pub contact_id: String,
    pub login: String,
    pub display_name: String,
    pub profiles: Vec<String>,
    pub teams: Vec<Team>,
}

impl CurrentUser {
    /// Two letters for the account button, e.g. "Jane van Doe" → "JD".
    pub fn initials(&self) -> String {
        let words: Vec<&str> = self
            .display_name
            .split(' ')
            .filter(|w| w.chars().next().is_some_and(char::is_alphabetic))
            .collect();
        let picked: Vec<&str> = if words.len() > 1 {
            vec![words[0], words[words.len() - 1]]
        } else {
            words.into_iter().take(1).collect()
        };
        let letters: String = picked
            .iter()
            .filter_map(|w| w.chars().next())
            .flat_map(char::to_uppercase)
            .collect();
        if letters.is_empty() {
            self.login
                .chars()
                .take(2)
                .collect::<String>()
                .to_uppercase()
        } else {
            letters
        }
    }

    /// Reads the User with this login, then its Person and team links. Read-only.
    pub async fn load(login: &str, client: &ITopClient) -> Result<CurrentUser, ITopError> {
        let users = client
            .send(RestOperation::get(
                "User",
                format!("SELECT User WHERE login = {}", oql::quote(login)),
                &[
                    "login",
                    "contactid",
                    "first_name",
                    "last_name",
                    "profile_list",
                ],
            ))
            .await?;
        let objects = users.objects();
        let user = objects.first().ok_or(ITopError::InvalidCredentials)?;
        let f = &user.fields;
        let contact_id = f
            .get("contactid")
            .and_then(JsonValue::int_value)
            .map_or_else(|| "0".to_string(), |n| n.to_string());
        let profiles: Vec<String> = match f.get("profile_list") {
            Some(JsonValue::Array(list)) => list
                .iter()
                .filter_map(|p| {
                    p.get("profile")
                        .and_then(JsonValue::string_value)
                        .map(str::to_string)
                })
                .collect(),
            _ => Vec::new(),
        };

        let mut teams: Vec<Team> = Vec::new();
        let mut name = join_names([f.get("first_name"), f.get("last_name")]);
        if contact_id != "0" {
            let people = client
                .send(RestOperation::get(
                    "Person",
                    format!("SELECT Person WHERE id = {contact_id}"),
                    &["name", "first_name", "team_list"],
                ))
                .await?;
            if let Some(person) = people.objects().first() {
                let p = &person.fields;
                let person_name = join_names([p.get("first_name"), p.get("name")]);
                if !person_name.is_empty() {
                    name = person_name;
                }
                if let Some(JsonValue::Array(links)) = p.get("team_list") {
                    teams = links
                        .iter()
                        .filter_map(|link| {
                            let id = link.get("team_id").and_then(JsonValue::int_value)?;
                            let name = link
                                .get("team_name")
                                .and_then(JsonValue::string_value)
                                .map_or_else(|| format!("Team {id}"), str::to_string);
                            Some(Team {
                                id: id.to_string(),
                                name,
                            })
                        })
                        .collect();
                }
            }
        }
        Ok(CurrentUser {
            user_id: user.key.clone(),
            contact_id,
            login: login.to_string(),
            display_name: if name.is_empty() {
                login.to_string()
            } else {
                name
            },
            profiles,
            teams,
        })
    }
}

/// The string values among these, non-empty ones joined with a space.
fn join_names<const N: usize>(values: [Option<&JsonValue>; N]) -> String {
    values
        .iter()
        .filter_map(|v| v.and_then(JsonValue::string_value))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
