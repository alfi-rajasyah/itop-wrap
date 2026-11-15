//! Port of `RichText.swift`: `RichRun`, `RichImage`, `RichBlock`, the `RichText` parser,
//! `AttachmentRef`, `AttachmentFile` and `InlineImageLoader`.
//!
//! URLs are kept as strings (what Swift's `URL.absoluteString` gives), so they serialise to the
//! UI as they are. Swift resolves relative references with Foundation's RFC 3986 rules and this
//! port with the `url` crate (WHATWG); they agree on the links iTop writes (absolute http(s),
//! mailto, paths relative to the iTop folder, `//host/…`). They can differ on malformed input:
//! a backslash in a relative path (WHATWG reads it as `/`), or an absolute URL the `url` crate
//! refuses (an invalid host), which loses its link here.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use url::Url;

use crate::case_log::{HtmlText, SCRIPT_STYLE, regex, replace_all};
use crate::client::ITopClient;
use crate::error::{ITopError, is_swift_whitespace};
use crate::json::{JsonValue, swift_int};
use crate::oql;
use crate::rest_operation::RestOperation;
use crate::ticket::Ticket;
use crate::ticket_queries::localized_standard_compare;

/// `Data(base64Encoded:)`: the standard alphabet, padding required, no whitespace. Foundation
/// ignores non-zero bits in the last character, so they are allowed here too.
pub(crate) fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const ENGINE: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::RequireCanonical)
            .with_decode_allow_trailing_bits(true),
    );
    ENGINE.decode(text).ok()
}

/// Bytes as a base64 string in JSON, so the UI can build a `data:` URL.
pub(crate) mod base64_bytes {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}

/// A piece of text, with a link when it came from an `<a>`. Images from other hosts are never
/// loaded and stay in the line: a small one (a signature icon) as an icon, a larger one as a
/// chip labelled with its host that opens it in the browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct RichRun {
    pub text: String,
    pub link: Option<String>,
    pub icon: Option<String>,
    /// True for the "🖼 host ↗" chip of a larger outside image; `link` opens it.
    pub is_outside_image: bool,
}

impl RichRun {
    /// `RichRun(text:)`.
    pub fn text(text: impl Into<String>) -> Self {
        RichRun {
            text: text.into(),
            link: None,
            icon: None,
            is_outside_image: false,
        }
    }

    /// `RichRun(text:link:)`.
    pub fn link(text: impl Into<String>, link: impl Into<String>) -> Self {
        RichRun {
            text: text.into(),
            link: Some(link.into()),
            icon: None,
            is_outside_image: false,
        }
    }

    /// `RichRun(icon:)`.
    pub fn icon(url: impl Into<String>) -> Self {
        RichRun {
            text: String::new(),
            link: None,
            icon: Some(url.into()),
            is_outside_image: false,
        }
    }

    /// `RichRun(outsideImage:)`: labelled with the URL's host ("another site" without one).
    pub fn outside_image(url: impl Into<String>) -> Self {
        let url = url.into();
        let host = Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "another site".to_string());
        RichRun {
            text: host,
            link: Some(url),
            icon: None,
            is_outside_image: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum RichImage {
    /// An image pasted into iTop: loaded by id through REST, never through the web link.
    Inline { id: String },
    /// A `data:` image: already in the HTML, no network needed (base64 in JSON).
    Data(
        #[serde(with = "base64_bytes")]
        #[ts(type = "string")]
        Vec<u8>,
    ),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum RichBlock {
    Text(Vec<RichRun>),
    Image(RichImage),
}

/// HTML from iTop as text blocks with links, and images in between (PLAN.md Task 5c).
/// It follows `HtmlText::plain`'s rules, so the text reads the same as the plain version.
pub struct RichText;

// Link boundaries and icons survive whitespace clean-up as private-use characters,
// then a scanner turns them into runs.
const START: char = '\u{E000}';
const MID: char = '\u{E001}';
const END: char = '\u{E002}';
const ICON_MARK: char = '\u{E003}';

/// One of the private-use characters above.
fn is_mark(c: char) -> bool {
    ('\u{E000}'..='\u{E003}').contains(&c)
}

struct Icon {
    url: String,
    small: bool,
}

impl RichText {
    /// `base` is the server's base URL (`ServerAddress::base_url`).
    pub fn parse(html: &str, base: &str) -> Vec<RichBlock> {
        let source = replace_all(regex!(SCRIPT_STYLE), html, "");

        let mut blocks: Vec<RichBlock> = Vec::new();
        let mut buffer = String::new();
        let mut links: Vec<String> = Vec::new();
        let mut icons: Vec<Icon> = Vec::new();
        let mut in_link = false;

        fn flush(
            buffer: &mut String,
            blocks: &mut Vec<RichBlock>,
            links: &[String],
            icons: &[Icon],
        ) {
            let text = HtmlText::normalize(buffer);
            buffer.clear();
            let runs = runs(&text, links, icons);
            if !runs.is_empty() {
                blocks.push(RichBlock::Text(runs));
            }
        }

        let Some(tokens) = regex!(r"<[^>]*>|[^<]+") else {
            return blocks;
        };
        let tag_name = regex!(r"^<\s*/?\s*([a-z0-9]+)");
        for token in tokens.find_iter(&source) {
            let piece = token.as_str();
            if !piece.starts_with('<') {
                // The marks are the parser's own: text (or `&#xE000;`) can't forge a link or icon.
                let text = HtmlText::decode_entities(piece);
                buffer.extend(text.chars().filter(|c| !is_mark(*c)));
                continue;
            }
            let tag = piece.to_lowercase();
            let name = tag_name
                .and_then(|re| re.captures(&tag))
                .and_then(|c| c.get(1))
                .map_or("", |m| m.as_str());
            let closing = tag.starts_with("</");
            match name {
                "img" => {
                    if let Some(url) = outside_image(piece, base) {
                        // From another host: stays in the line, never loaded (icon, or a host chip).
                        if in_link {
                            buffer.push(END);
                            in_link = false;
                        }
                        icons.push(Icon {
                            url,
                            small: is_small(piece),
                        });
                        buffer.push(ICON_MARK);
                        buffer.push_str(&(icons.len() - 1).to_string());
                        buffer.push(MID);
                    } else if let Some(image) = image(piece, base) {
                        if in_link {
                            buffer.push(END);
                            in_link = false;
                        }
                        flush(&mut buffer, &mut blocks, &links, &icons);
                        blocks.push(RichBlock::Image(image));
                    }
                }
                "br" => buffer.push('\n'),
                "li" => {
                    if !closing {
                        buffer.push_str("\n• ");
                    }
                }
                "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "figure" | "ul"
                | "ol" | "table" | "pre" => buffer.push_str("\n\n"),
                "div" | "tr" | "section" | "article" | "header" | "footer" => buffer.push('\n'),
                "td" | "th" => buffer.push(' '),
                "a" => {
                    if closing {
                        if in_link {
                            buffer.push(END);
                            in_link = false;
                        }
                    } else if let Some(url) = link(piece, base) {
                        if in_link {
                            buffer.push(END);
                        }
                        links.push(url);
                        buffer.push(START);
                        buffer.push_str(&(links.len() - 1).to_string());
                        buffer.push(MID);
                        in_link = true;
                    }
                }
                _ => {}
            }
        }
        if in_link {
            buffer.push(END);
        }
        flush(&mut buffer, &mut blocks, &links, &icons);
        blocks
    }
}

/// Reads the marks back into runs. Swift walks `Character`s and this walks `char`s; they only
/// differ if a combining mark follows a mark character, which iTop's HTML never has.
fn runs(text: &str, links: &[String], icons: &[Icon]) -> Vec<RichRun> {
    let mut out: Vec<RichRun> = Vec::new();
    let mut plain = String::new();
    let mut label = String::new();
    let mut link: Option<String> = None;
    // (reading an icon mark, digits so far)
    let mut reading: Option<(bool, String)> = None;

    fn emit_plain(out: &mut Vec<RichRun>, plain: &mut String) {
        if !plain.is_empty() {
            out.push(RichRun::text(std::mem::take(plain)));
        }
    }

    for ch in text.chars() {
        if let Some((is_icon, mut digits)) = reading.take() {
            if ch == MID {
                // `Int(digits) ?? -1`: anything that isn't a plain number points nowhere.
                let index = swift_int(&digits)
                    .and_then(|i| usize::try_from(i).ok())
                    .unwrap_or(usize::MAX);
                if is_icon {
                    if let Some(icon) = icons.get(index) {
                        emit_plain(&mut out, &mut plain);
                        out.push(if icon.small {
                            RichRun::icon(icon.url.clone())
                        } else {
                            RichRun::outside_image(icon.url.clone())
                        });
                    }
                } else if let Some(l) = links.get(index) {
                    emit_plain(&mut out, &mut plain);
                    link = Some(l.clone());
                    label.clear();
                }
            } else if ch.is_numeric() {
                // Swift's `Character.isNumber`; any non-ASCII digit makes `Int` fail above.
                digits.push(ch);
                reading = Some((is_icon, digits));
            }
            continue;
        }
        match ch {
            START => reading = Some((false, String::new())),
            ICON_MARK => reading = Some((true, String::new())),
            END => {
                if let Some(l) = link.take()
                    && !label.is_empty()
                {
                    out.push(RichRun::link(std::mem::take(&mut label), l));
                }
                label.clear();
            }
            MID => {}
            _ => {
                if link.is_some() {
                    label.push(ch);
                } else {
                    plain.push(ch);
                }
            }
        }
    }
    match link {
        Some(l) if !label.is_empty() => out.push(RichRun::link(label, l)),
        _ => plain.push_str(&label),
    }
    emit_plain(&mut out, &mut plain);
    out
}

/// The attribute patterns, one per name the parser reads (the `regex` crate compiles each once).
fn attribute_regex(name: &str) -> Option<&'static regex::Regex> {
    macro_rules! attr {
        ($name:literal) => {
            regex!(concat!(r"(?i)\b", $name, r#"\s*=\s*("([^"]*)"|'([^']*)')"#))
        };
    }
    match name {
        "src" => attr!("src"),
        "href" => attr!("href"),
        "data-img-id" => attr!("data-img-id"),
        "style" => attr!("style"),
        "width" => attr!("width"),
        "height" => attr!("height"),
        _ => None,
    }
}

/// A quoted attribute's value, entities decoded. Unquoted values aren't read (as in Swift).
fn attribute(name: &str, tag: &str) -> Option<String> {
    let caps = attribute_regex(name)?.captures(tag)?;
    let value = caps.get(2).or_else(|| caps.get(3))?;
    Some(HtmlText::decode_entities(value.as_str()))
}

/// Signature-icon size: the width and height it declares (attributes or inline style) are at
/// most 40 px.
pub(crate) fn is_small(tag: &str) -> bool {
    fn px(name: &str, tag: &str) -> Option<i64> {
        if let Some(v) = attribute(name, tag)
            && let Some(n) = swift_int(&v.trim_matches(is_swift_whitespace).replace("px", ""))
        {
            return Some(n);
        }
        let style = attribute("style", tag)?;
        let re = match name {
            "width" => regex!(r"(?i)(?:^|;)\s*width\s*:\s*(\d+)\s*px"),
            "height" => regex!(r"(?i)(?:^|;)\s*height\s*:\s*(\d+)\s*px"),
            _ => None,
        }?;
        let digits = re.captures(&style)?.get(1)?;
        swift_int(digits.as_str())
    }
    let sizes: Vec<i64> = [px("width", tag), px("height", tag)]
        .into_iter()
        .flatten()
        .collect();
    !sizes.is_empty() && sizes.iter().all(|s| *s <= 40)
}

/// Whether the text starts with a URL scheme (`scheme:`), so it isn't a relative reference.
fn has_scheme(text: &str) -> bool {
    let Some((scheme, _)) = text.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `URL(string: text, relativeTo: base folder)?.absoluteURL`, as its absolute string.
pub(crate) fn resolve(text: &str, base: &str) -> Option<String> {
    let t = text.trim_matches(is_swift_whitespace);
    // `URL(string: "")` is nil, where joining an empty reference would give the folder.
    if t.is_empty() {
        return None;
    }
    if has_scheme(t) {
        let parsed = Url::parse(t).ok()?;
        // An absolute URL is its own absolute form in Foundation; keep its spelling (the `url`
        // crate would add a "/" to "https://host") unless it has characters Foundation encodes.
        let clean = t
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"\"<>\\^`{|}".contains(&b));
        return Some(if clean {
            t.to_string()
        } else {
            parsed.to_string()
        });
    }
    let folder = if base.ends_with('/') {
        base.to_string()
    } else {
        format!("{base}/")
    };
    Some(Url::parse(&folder).ok()?.join(t).ok()?.to_string())
}

/// `URLComponents.queryItems`: names and values percent-decoded, `+` left alone, a value only
/// when there is an `=`.
fn query_items(url: &str) -> Vec<(String, Option<String>)> {
    let Some(query) = Url::parse(url)
        .ok()
        .and_then(|u| u.query().map(str::to_string))
    else {
        return Vec::new();
    };
    query
        .split('&')
        .map(|item| match item.split_once('=') {
            Some((n, v)) => (percent_decode(n), Some(percent_decode(v))),
            None => (percent_decode(item), None),
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

fn is_inline_image_link(items: &[(String, Option<String>)]) -> bool {
    items
        .iter()
        .any(|(n, v)| n == "operation" && v.as_deref() == Some("download_inlineimage"))
}

fn scheme_of(url: &str) -> Option<String> {
    Url::parse(url).ok().map(|u| u.scheme().to_lowercase())
}

/// Only web and email links: `javascript:`, `file:` and the like keep their text but lose the link.
pub(crate) fn link(tag: &str, base: &str) -> Option<String> {
    let href = attribute("href", tag)?;
    if href.is_empty() {
        return None;
    }
    let url = resolve(&href, base)?;
    let scheme = scheme_of(&url)?;
    ["http", "https", "mailto"]
        .contains(&scheme.as_str())
        .then_some(url)
}

pub(crate) fn image(tag: &str, base: &str) -> Option<RichImage> {
    let src = attribute("src", tag)?;
    if src.to_lowercase().starts_with("data:image/")
        && let Some(comma) = src.find(',')
        && src[..comma].ends_with(";base64")
    {
        return decode_base64(&src[comma + 1..]).map(RichImage::Data);
    }
    let url = resolve(&src, base)?;
    let items = query_items(&url);
    if is_inline_image_link(&items) {
        // `attribute(...) ?? query id`: a present but non-numeric data-img-id doesn't fall back.
        let id = attribute("data-img-id", tag).or_else(|| {
            items
                .iter()
                .find(|(n, _)| n == "id")
                .and_then(|(_, v)| v.clone())
        });
        return id
            .and_then(|i| swift_int(&i))
            .map(|n| RichImage::Inline { id: n.to_string() });
    }
    None
}

/// An http(s) image that isn't a pasted iTop image: from another host, so never loaded.
pub(crate) fn outside_image(tag: &str, base: &str) -> Option<String> {
    let src = attribute("src", tag)?;
    if src.to_lowercase().starts_with("data:") {
        return None;
    }
    let url = resolve(&src, base)?;
    let scheme = scheme_of(&url)?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    if is_inline_image_link(&query_items(&url)) {
        None
    } else {
        Some(url)
    }
}

// MARK: - Attachments

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AttachmentRef {
    pub id: String,
    pub name: String,
}

impl AttachmentRef {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        AttachmentRef {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AttachmentFile {
    #[serde(with = "base64_bytes")]
    #[ts(type = "string")]
    pub data: Vec<u8>,
    pub filename: String,
    pub mimetype: String,
}

/// Swift's `InlineImageLoader.Image`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct InlineImage {
    #[serde(with = "base64_bytes")]
    #[ts(type = "string")]
    pub data: Vec<u8>,
    pub mimetype: String,
}

/// Swift's `InlineImageLoader.Failure`.
///
/// It has no `LocalizedError` text in Swift, so what the user sees is Foundation's generic
/// description with the case number; `Display` gives that same text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum InlineImageFailure {
    #[error("The operation couldn’t be completed. (ITopKit.InlineImageLoader.Failure error 0.)")]
    BadId,
    #[error("The operation couldn’t be completed. (ITopKit.InlineImageLoader.Failure error 1.)")]
    NotFound,
    #[error("The operation couldn’t be completed. (ITopKit.InlineImageLoader.Failure error 2.)")]
    Undecodable,
}

/// What the loader can throw: its own failures, or iTop's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum InlineImageError {
    #[error(transparent)]
    Failure(#[from] InlineImageFailure),
    #[error(transparent)]
    ITop(#[from] ITopError),
}

#[derive(Default)]
struct ImageCache {
    images: HashMap<String, InlineImage>,
    order: VecDeque<String>,
}

/// Pasted images and attachments, read through REST with the app's own login. Your iTop
/// refuses the web image links with HTTP Basic auth (403 in the Task 5c probe), so they're
/// never used. Images are kept in memory (the last 60).
///
/// Swift's actor becomes a mutex held only between awaits. As with the actor, two concurrent
/// loads of the same id both reach iTop and both record the id in the eviction order.
pub struct InlineImageLoader {
    client: ITopClient,
    cache: Mutex<ImageCache>,
}

impl InlineImageLoader {
    const CACHE_SIZE: usize = 60;

    pub fn new(client: ITopClient) -> Self {
        InlineImageLoader {
            client,
            cache: Mutex::new(ImageCache::default()),
        }
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, ImageCache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub async fn image(&self, id: &str) -> Result<InlineImage, InlineImageError> {
        if let Some(hit) = self.cache().images.get(id) {
            return Ok(hit.clone());
        }
        let n = swift_int(id).ok_or(InlineImageFailure::BadId)?;
        let r = self
            .client
            .send(RestOperation::get(
                "InlineImage",
                format!("SELECT InlineImage WHERE id = {n}"),
                &["contents"],
            ))
            .await?;
        let objects = r.objects();
        let contents = objects
            .first()
            .and_then(|o| o.fields.get("contents"))
            .ok_or(InlineImageFailure::NotFound)?;
        let data = contents
            .get("data")
            .and_then(JsonValue::string_value)
            .and_then(decode_base64)
            .ok_or(InlineImageFailure::Undecodable)?;
        let image = InlineImage {
            data,
            mimetype: contents
                .get("mimetype")
                .and_then(JsonValue::string_value)
                .unwrap_or("")
                .to_string(),
        };
        let mut cache = self.cache();
        cache.images.insert(id.to_string(), image.clone());
        cache.order.push_back(id.to_string());
        if cache.order.len() > Self::CACHE_SIZE
            && let Some(oldest) = cache.order.pop_front()
        {
            cache.images.remove(&oldest);
        }
        Ok(image)
    }

    /// The ticket's attachments by name. The file contents are only fetched on download.
    pub async fn attachments(&self, ticket: &Ticket) -> Result<Vec<AttachmentRef>, ITopError> {
        let Some(key) = swift_int(&ticket.key) else {
            return Ok(Vec::new());
        };
        let r = self
            .client
            .send(RestOperation::get(
                "Attachment",
                format!(
                    "SELECT Attachment WHERE item_class = {} AND item_id = {key}",
                    oql::quote(ticket.ticket_class.raw_value())
                ),
                &["friendlyname"],
            ))
            .await?;
        let mut list: Vec<AttachmentRef> = r
            .objects()
            .iter()
            .map(|o| {
                AttachmentRef::new(
                    o.key.clone(),
                    o.fields
                        .get("friendlyname")
                        .and_then(JsonValue::string_value)
                        .map_or_else(|| format!("Attachment {}", o.key), str::to_string),
                )
            })
            .collect();
        list.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(list)
    }

    pub async fn download(
        &self,
        attachment: &AttachmentRef,
    ) -> Result<AttachmentFile, InlineImageError> {
        let n = swift_int(&attachment.id).ok_or(InlineImageFailure::BadId)?;
        let r = self
            .client
            .send(RestOperation::get(
                "Attachment",
                format!("SELECT Attachment WHERE id = {n}"),
                &["contents"],
            ))
            .await?;
        let objects = r.objects();
        let contents = objects
            .first()
            .and_then(|o| o.fields.get("contents"))
            .ok_or(InlineImageFailure::NotFound)?;
        let data = contents
            .get("data")
            .and_then(JsonValue::string_value)
            .and_then(decode_base64)
            .ok_or(InlineImageFailure::Undecodable)?;
        let text = |k: &str| contents.get(k).and_then(JsonValue::string_value);
        Ok(AttachmentFile {
            data,
            filename: text("filename").unwrap_or(&attachment.name).to_string(),
            mimetype: text("mimetype").unwrap_or("").to_string(),
        })
    }
}
