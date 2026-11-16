//! Editing a ticket's own fields (iTop Wrap addition, not in the Swift app): the same fields as
//! New ticket, prefilled from the ticket, saved with one `core/update` of only what changed.
//!
//! iTop stores the description as HTML. A description that is just paragraphs and line breaks
//! (what New ticket writes, and what iTop's editor makes from plain typing) is edited as text. One
//! with anything else in it (images, links, formatting) is never rewritten here: the app shows it
//! read-only, so saving can't drop a pasted screenshot.

use crate::json::JsonObject;
use crate::new_ticket::NewTicket;
use crate::rest_operation::RestOperation;
use crate::ticket::Ticket;
use crate::ticket_writes::{Failure, TicketWrites, WriteError};

/// A ticket's editable fields as they are in iTop now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableTicket {
    /// Ids, impact, urgency, title, and the description as text (empty when it's formatted).
    pub values: NewTicket,
    pub org_name: String,
    pub caller_name: String,
    pub service_name: String,
    pub subcategory_name: String,
    /// `None` when the description has images or formatting: it can't be edited here.
    pub description_text: Option<String>,
}

impl EditableTicket {
    pub fn description_locked(&self) -> bool {
        self.description_text.is_none()
    }
}

impl TicketWrites {
    pub const EDIT_FIELDS: [&'static str; 13] = [
        "org_id",
        "org_id_friendlyname",
        "caller_id",
        "caller_id_friendlyname",
        "service_id",
        "service_id_friendlyname",
        "servicesubcategory_id",
        "servicesubcategory_id_friendlyname",
        "impact",
        "urgency",
        "title",
        "description",
        "status",
    ];

    /// Reads the ticket's editable fields fresh from iTop.
    pub async fn editable(&self, ticket: &Ticket) -> Result<EditableTicket, WriteError> {
        let cls = ticket.ticket_class.raw_value();
        let response = self
            .client
            .send(RestOperation::get(
                cls,
                format!("SELECT {cls} WHERE id = {}", ticket.key),
                &Self::EDIT_FIELDS,
            ))
            .await?;
        let Some(object) = response.objects().first().cloned() else {
            return Err(crate::error::ITopError::Server {
                code: 0,
                message: format!("{} isn't in iTop any more.", ticket.r#ref),
            }
            .into());
        };
        let text = |key: &str| field_text(&object.fields, key);
        let id = |key: &str| {
            let v = text(key);
            if Self::is_id(&v) && v != "0" {
                v
            } else {
                String::new()
            }
        };
        let description_html = text("description");
        let description_text = plain_text(&description_html);
        let mut values = NewTicket::new(ticket.ticket_class);
        values.org_id = id("org_id");
        values.caller_id = id("caller_id");
        values.service_id = id("service_id");
        values.subcategory_id = id("servicesubcategory_id");
        values.impact = text("impact");
        values.urgency = text("urgency");
        values.title = text("title");
        values.description = description_text.clone().unwrap_or_default();
        Ok(EditableTicket {
            values,
            org_name: text("org_id_friendlyname"),
            caller_name: text("caller_id_friendlyname"),
            service_name: text("service_id_friendlyname"),
            subcategory_name: text("servicesubcategory_id_friendlyname"),
            description_text,
        })
    }

    /// Saves what changed between `before` (as read by [`TicketWrites::editable`]) and `after`.
    /// Required fields are checked first, so nothing is sent for an edit iTop would refuse; a
    /// locked description is never sent. Returns the iTop field codes that were sent (empty when
    /// nothing changed, and then nothing is sent).
    pub async fn save_edit(
        &self,
        ticket: &Ticket,
        before: &NewTicket,
        after: &NewTicket,
        description_locked: bool,
    ) -> Result<Vec<String>, WriteError> {
        if ticket.status == "closed" {
            return Err(Failure::NotAllowed {
                status: ticket.status.clone(),
            }
            .into());
        }
        for (value, name) in [
            (&after.org_id, "organization"),
            (&after.caller_id, "caller"),
            (&after.service_id, "service"),
            (&after.subcategory_id, "subcategory"),
        ] {
            if !Self::is_id(value) {
                return Err(Failure::Missing(name.into()).into());
            }
        }
        if after.title.trim().is_empty() {
            return Err(Failure::Missing("title".into()).into());
        }
        if !description_locked && after.description.trim().is_empty() {
            return Err(Failure::Missing("description".into()).into());
        }
        let fields = edit_changes(before, after, description_locked);
        let sent: Vec<String> = fields.keys().cloned().collect();
        if fields.is_empty() {
            return Ok(sent);
        }
        self.update(ticket, fields).await?;
        Ok(sent)
    }
}

/// A field as text: iTop sends ids and enums as strings or numbers.
fn field_text(fields: &JsonObject, key: &str) -> String {
    match fields.get(key) {
        Some(v) => v
            .string_value()
            .map(str::to_owned)
            .or_else(|| v.int_value().map(|n| n.to_string()))
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// The `core/update` fields for what changed; the description only when it isn't locked.
pub fn edit_changes(before: &NewTicket, after: &NewTicket, description_locked: bool) -> JsonObject {
    let mut fields = JsonObject::new();
    for (code, b, a) in [
        ("org_id", &before.org_id, &after.org_id),
        ("caller_id", &before.caller_id, &after.caller_id),
        ("service_id", &before.service_id, &after.service_id),
        (
            "servicesubcategory_id",
            &before.subcategory_id,
            &after.subcategory_id,
        ),
        ("impact", &before.impact, &after.impact),
        ("urgency", &before.urgency, &after.urgency),
    ] {
        if a != b {
            fields.insert(code.into(), a.as_str().into());
        }
    }
    if after.title.trim() != before.title.trim() {
        fields.insert("title".into(), after.title.trim().into());
    }
    if !description_locked && after.description.trim() != before.description.trim() {
        fields.insert(
            "description".into(),
            NewTicket::html(after.description.trim()).into(),
        );
    }
    fields
}

/// A description as text, if it's plain: only `<p>`, `</p>` and `<br>` tags (no attributes)
/// and character references. Anything else (an image, a link, bold, a list…) → `None`.
pub fn plain_text(html: &str) -> Option<String> {
    if !html.contains('<') {
        return Some(decode_entities(html)?.trim().to_owned());
    }
    // Source line breaks between tags mean nothing in HTML.
    let html: String = html.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    let mut out = String::new();
    let mut rest = html.as_str();
    let mut paragraphs = 0;
    while let Some(start) = rest.find('<') {
        out.push_str(&decode_entities(&rest[..start])?);
        let end = rest[start..].find('>')? + start;
        let tag = rest[start + 1..end].trim().to_ascii_lowercase();
        match tag.as_str() {
            "p" => {
                if paragraphs > 0 {
                    out.push_str("\n\n");
                }
                paragraphs += 1;
            }
            "/p" => {}
            "br" | "br/" | "br /" => out.push('\n'),
            _ => return None,
        }
        rest = &rest[end + 1..];
    }
    out.push_str(&decode_entities(rest)?);
    Some(out.trim().to_owned())
}

/// `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`/`&apos;`, `&nbsp;` and numeric references; an
/// unknown named reference means the text isn't plain.
fn decode_entities(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = after;
            continue;
        };
        let name = &after[..end];
        let ch = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" | "#39" => '\'',
            "nbsp" => ' ',
            n if n.starts_with("#x") || n.starts_with("#X") => {
                char::from_u32(u32::from_str_radix(&n[2..], 16).ok()?)?
            }
            n if n.starts_with('#') => char::from_u32(n[1..].parse().ok()?)?,
            _ => return None,
        };
        out.push(ch);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ticket::TicketClass;

    #[test]
    fn plain_descriptions_become_text() {
        assert_eq!(plain_text("<p>VPN drops</p>").as_deref(), Some("VPN drops"));
        assert_eq!(
            plain_text("<p>One</p>\n<p>Two<br>three</p>").as_deref(),
            Some("One\n\nTwo\nthree")
        );
        assert_eq!(
            plain_text("<p>a &amp; b &lt;c&gt; &quot;d&quot; &#39;e&#39; &#233;</p>").as_deref(),
            Some("a & b <c> \"d\" 'e' é")
        );
        assert_eq!(plain_text("just text").as_deref(), Some("just text"));
        assert_eq!(plain_text("").as_deref(), Some(""));
        assert_eq!(plain_text("<P>Up</P><BR/>").as_deref(), Some("Up"));
    }

    #[test]
    fn formatted_descriptions_are_locked() {
        for html in [
            r#"<p>See <img src="data:image/png;base64,AA=="></p>"#,
            r#"<p><a href="https://x">link</a></p>"#,
            "<p><strong>bold</strong></p>",
            "<ul><li>one</li></ul>",
            r#"<p style="color:red">red</p>"#,
            "<div>block</div>",
            "<p>&weird;</p>",
            "<p>unclosed <b</p>",
        ] {
            assert_eq!(plain_text(html), None, "{html}");
        }
    }

    #[test]
    fn new_ticket_html_round_trips() {
        let text = "Line one\nline two\n\nSecond <para> & \"quotes\"";
        assert_eq!(plain_text(&NewTicket::html(text)).as_deref(), Some(text));
    }

    fn values() -> NewTicket {
        let mut v = NewTicket::new(TicketClass::UserRequest);
        v.org_id = "3".into();
        v.caller_id = "7".into();
        v.service_id = "5".into();
        v.subcategory_id = "6".into();
        v.title = "VPN drops".into();
        v.description = "Every hour".into();
        v
    }

    #[test]
    fn only_changed_fields_are_sent() {
        let before = values();
        assert!(edit_changes(&before, &before, false).is_empty());
        let mut after = before.clone();
        after.title = "  VPN drops every hour ".into();
        after.urgency = "2".into();
        let fields = edit_changes(&before, &after, false);
        let keys: Vec<&String> = fields.keys().collect();
        assert_eq!(keys, ["title", "urgency"]);
        assert_eq!(fields["title"].string_value(), Some("VPN drops every hour"));
    }

    #[test]
    fn a_locked_description_is_never_sent() {
        let before = values();
        let mut after = before.clone();
        after.description = "rewritten".into();
        assert!(edit_changes(&before, &after, true).is_empty());
        let fields = edit_changes(&before, &after, false);
        assert_eq!(
            fields["description"].string_value(),
            Some("<p>rewritten</p>")
        );
    }
}
