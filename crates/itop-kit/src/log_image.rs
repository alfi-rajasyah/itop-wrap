//! Port of `LogImage.swift`: `LogImage` (type sniffing, size limit, pixel size), the upload as an
//! `InlineImage`, the entry HTML, and the InlineImage secret.
//!
//! Converting other image types (HEIC, TIFF, …) is not here: the Swift app does it with AppKit in
//! the app target (`ImageIntake` in ComposerTextView.swift), before a `LogImage` is made.
//!
//! The pixel size comes from ImageIO in Swift (`kCGImagePropertyPixelWidth/Height`, the stored
//! size, before any EXIF rotation). Here it is read from the file's own header: PNG `IHDR`, the
//! GIF logical screen, the JPEG `SOFn` frame. Files ImageIO can't decode but whose header is
//! intact may get a size here where Swift had none; the only effect is the `<img>` size
//! attributes.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Serialize, Serializer};
use ts_rs::TS;

use crate::error::ITopError;
use crate::json::{JsonObject, JsonValue};
use crate::rest_operation::RestOperation;
use crate::ticket::Ticket;
use crate::ticket_writes::{COMMENT, Failure as WriteFailure, TicketWrites, WriteError, log_item};

/// Why an image can't be added (Swift's `LogImage.Failure`). The app words these itself
/// (parity checklist LOG-19); `Display` is for logs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
// Two Swift types are called `Failure`; the TypeScript names keep them apart.
#[ts(
    rename = "LogImageFailure",
    export,
    export_to = "../../../ui/src/bindings/"
)]
pub enum Failure {
    #[error("{filename} is not a PNG, JPEG or GIF")]
    #[serde(rename_all = "camelCase")]
    UnsupportedType { filename: String },
    #[error("{filename} is larger than {max_mb} MB")]
    #[serde(rename_all = "camelCase")]
    TooLarge {
        filename: String,
        #[ts(type = "number")]
        max_mb: i64,
    },
}

/// An image to add to a log entry, checked before anything is sent: a PNG, JPEG or GIF (judged by
/// its bytes, not its name) of at most `MAX_BYTES`.
///
/// It's uploaded as an `InlineImage` on the ticket, the way the iTop web UI stores a pasted image
/// (fixtures/log-images-probe.json), so it shows inside the entry on the web and in the app.
///
/// Serialised for the UI with `data` as base64. There is no `Deserialize`: an image only comes
/// into being through [`LogImage::new`], which checks it.
#[derive(Clone, Debug, PartialEq, Eq, TS)]
#[ts(
    rename_all = "camelCase",
    export,
    export_to = "../../../ui/src/bindings/"
)]
pub struct LogImage {
    /// A fresh UUID string per image, like Swift's `UUID()` (so equal bytes are still two images).
    pub id: String,
    /// Base64 in the serde form.
    #[ts(type = "string")]
    pub data: Vec<u8>,
    pub filename: String,
    pub mimetype: String,
    #[ts(type = "number | null")]
    pub pixel_width: Option<i64>,
    #[ts(type = "number | null")]
    pub pixel_height: Option<i64>,
}

/// Written by hand so `data` goes out as base64 (a derive would write an array of numbers).
impl Serialize for LogImage {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Out<'a> {
            id: &'a str,
            data: String,
            filename: &'a str,
            mimetype: &'a str,
            pixel_width: Option<i64>,
            pixel_height: Option<i64>,
        }
        Out {
            id: &self.id,
            data: STANDARD.encode(&self.data),
            filename: &self.filename,
            mimetype: &self.mimetype,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
        }
        .serialize(s)
    }
}

impl LogImage {
    /// 4 MB. The upload is sent base64 in a form field, which makes it about 1.4 times bigger;
    /// this keeps it under PHP's default 8 MB `post_max_size`.
    pub const MAX_BYTES: usize = 4 * 1_048_576;
    pub const MAX_PER_ENTRY: usize = 5;
    pub(crate) const MAX_SHOWN_WIDTH: i64 = 800;

    pub fn new(data: Vec<u8>, filename: &str) -> Result<LogImage, Failure> {
        let name = filename.trim();
        let Some((mimetype, ext)) = Self::sniff(&data) else {
            return Err(Failure::UnsupportedType {
                filename: name.to_string(),
            });
        };
        if data.len() > Self::MAX_BYTES {
            return Err(Failure::TooLarge {
                filename: name.to_string(),
                max_mb: (Self::MAX_BYTES / 1_048_576) as i64,
            });
        }
        let size = Self::pixel_size(&data);
        Ok(LogImage {
            id: new_uuid(),
            filename: if name.is_empty() {
                format!("image.{ext}")
            } else {
                name.to_string()
            },
            mimetype: mimetype.to_string(),
            pixel_width: size.map(|s| s.0),
            pixel_height: size.map(|s| s.1),
            data,
        })
    }

    /// The type from the file's first bytes: (mimetype, extension).
    pub fn sniff(data: &[u8]) -> Option<(&'static str, &'static str)> {
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Some(("image/png", "png"));
        }
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(("image/jpeg", "jpg"));
        }
        if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            return Some(("image/gif", "gif"));
        }
        None
    }

    /// Width and height in pixels, both above zero, or `None`.
    pub fn pixel_size(data: &[u8]) -> Option<(i64, i64)> {
        let (w, h) = match Self::sniff(data)?.0 {
            "image/png" => png_size(data)?,
            "image/gif" => gif_size(data)?,
            _ => jpeg_size(data)?,
        };
        (w > 0 && h > 0).then_some((w, h))
    }

    /// The size the entry shows the image at: its own, narrowed to `MAX_SHOWN_WIDTH` like a web
    /// paste.
    pub fn shown_size(&self) -> Option<(i64, i64)> {
        let (w, h) = (self.pixel_width?, self.pixel_height?);
        if w <= Self::MAX_SHOWN_WIDTH {
            return Some((w, h));
        }
        // `.rounded()` is half away from zero, like `f64::round`.
        let scaled = (h as f64 * Self::MAX_SHOWN_WIDTH as f64 / w as f64).round() as i64;
        Some((Self::MAX_SHOWN_WIDTH, scaled.max(1)))
    }
}

fn be16(d: &[u8], at: usize) -> Option<i64> {
    let b = d.get(at..at + 2)?;
    Some(i64::from(u16::from_be_bytes([b[0], b[1]])))
}

fn png_size(d: &[u8]) -> Option<(i64, i64)> {
    // Signature (8), chunk length (4), "IHDR", then width and height as big-endian u32.
    if d.get(12..16)? != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(d.get(16..20)?.try_into().ok()?);
    let h = u32::from_be_bytes(d.get(20..24)?.try_into().ok()?);
    Some((i64::from(w), i64::from(h)))
}

fn gif_size(d: &[u8]) -> Option<(i64, i64)> {
    // The logical screen after the 6-byte header, little-endian.
    let b = d.get(6..10)?;
    Some((
        i64::from(u16::from_le_bytes([b[0], b[1]])),
        i64::from(u16::from_le_bytes([b[2], b[3]])),
    ))
}

fn jpeg_size(d: &[u8]) -> Option<(i64, i64)> {
    let mut i = 2; // after SOI
    loop {
        // Markers may be padded with extra 0xFF bytes.
        if *d.get(i)? != 0xFF {
            return None;
        }
        while *d.get(i)? == 0xFF {
            i += 1;
        }
        let marker = *d.get(i)?;
        i += 1;
        match marker {
            // Standalone markers without a length.
            0x01 | 0xD0..=0xD7 => continue,
            // End of image or start of scan before any frame header: no size.
            0xD9 | 0xDA => return None,
            _ => {}
        }
        let len = usize::try_from(be16(d, i)?).ok()?;
        if len < 2 {
            return None;
        }
        // SOF0–SOF15, except DHT (C4), JPG (C8) and DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            // length (2), precision (1), height (2), width (2).
            let h = be16(d, i + 3)?;
            let w = be16(d, i + 5)?;
            return Some((w, h));
        }
        i += len;
    }
}

/// A random (version 4) UUID in Foundation's `uuidString` form: uppercase, 8-4-4-4-12.
fn new_uuid() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02X}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// An image already stored in iTop as an InlineImage (Swift's `TicketWrites.UploadedImage`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct UploadedImage {
    pub id: String,
    pub secret: String,
    pub image: LogImage,
}

impl UploadedImage {
    pub fn new(id: impl Into<String>, secret: impl Into<String>, image: LogImage) -> Self {
        UploadedImage {
            id: id.into(),
            secret: secret.into(),
            image,
        }
    }
}

impl TicketWrites {
    /// Uploads the images as InlineImages on the ticket, then appends one HTML entry with the
    /// text (escaped, line breaks kept) followed by the images. Without images, the text goes as
    /// before. Everything is checked before the first upload; if iTop refuses an image, no entry
    /// is added.
    ///
    /// Swift's `addLogEntry(to:text:images:isPrivate:)` overload.
    pub async fn add_log_entry_with_images(
        &self,
        ticket: &Ticket,
        text: &str,
        images: &[LogImage],
        is_private: bool,
    ) -> Result<(), WriteError> {
        if images.is_empty() {
            return self.add_log_entry(ticket, text, is_private).await;
        }
        if images.len() > LogImage::MAX_PER_ENTRY {
            return Err(WriteFailure::TooManyImages {
                max: LogImage::MAX_PER_ENTRY as i64,
            }
            .into());
        }
        if !self
            .client
            .can_write(ticket.ticket_class.raw_value(), &ticket.key)
        {
            return Err(ITopError::WritesDisabled.into());
        }

        let mut uploaded = Vec::with_capacity(images.len());
        for image in images {
            let (id, secret) = self.upload(image, ticket).await?;
            uploaded.push(UploadedImage::new(id, secret, image.clone()));
        }
        let message = Self::entry_html(text, &uploaded, self.client.server.base_url());
        let field = if is_private {
            "private_log"
        } else {
            "public_log"
        };
        self.update(ticket, log_item(field, &message, "html"))
            .await?;
        Ok(())
    }

    /// The new InlineImage's id and secret. iTop Wrap sends a secret of its own and uses iTop's
    /// when the answer has one.
    pub(crate) async fn upload(
        &self,
        image: &LogImage,
        ticket: &Ticket,
    ) -> Result<(String, String), ITopError> {
        let own_secret = Self::new_secret();
        let mut contents = JsonObject::new();
        contents.insert("data".into(), STANDARD.encode(&image.data).into());
        contents.insert("mimetype".into(), image.mimetype.as_str().into());
        contents.insert("filename".into(), image.filename.as_str().into());
        let mut fields = JsonObject::new();
        fields.insert("item_class".into(), ticket.ticket_class.raw_value().into());
        fields.insert("item_id".into(), ticket.key.as_str().into());
        fields.insert("item_org_id".into(), ticket.org_id.as_str().into());
        fields.insert("secret".into(), own_secret.as_str().into());
        fields.insert("contents".into(), JsonValue::Object(contents));
        let response = self
            .client
            .send_on_ticket(
                RestOperation::Create {
                    class_name: "InlineImage".into(),
                    fields,
                    comment: COMMENT.into(),
                    output_fields: vec!["id".into(), "secret".into()],
                },
                Some(&Self::allowlist_key(ticket)),
            )
            .await?;
        Self::check(&response)?;
        let objects = response.objects();
        let object = objects.first().ok_or(ITopError::UnexpectedResponse)?;
        let id = object
            .fields
            .get("id")
            .and_then(JsonValue::string_value)
            .unwrap_or(&object.key)
            .to_string();
        if !Self::is_id(&id) || id == "0" {
            return Err(ITopError::UnexpectedResponse);
        }
        let secret = object
            .fields
            .get("secret")
            .and_then(JsonValue::string_value)
            .filter(|s| !s.is_empty())
            .map_or(own_secret, str::to_string);
        Ok((id, secret))
    }

    /// 32 hex characters, the shape iTop's own secrets have.
    pub fn new_secret() -> String {
        let bytes: [u8; 16] = rand::random();
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The entry's HTML: the text, then each image the way a web paste shows it. `base` is the
    /// server's base URL (`ServerAddress::base_url`).
    pub fn entry_html(text: &str, images: &[UploadedImage], base: &str) -> String {
        let mut out = Self::html(text);
        for u in images {
            out += &Self::image_html(&u.id, &u.secret, &u.image, base);
        }
        out
    }

    /// The text as one HTML paragraph, escaped, with `<br>` for each line break. Empty for blank
    /// text.
    pub fn html(text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = trimmed
            .replace("\r\n", "\n")
            .split('\n')
            .map(Self::escape)
            .collect();
        format!("<p>{}</p>", lines.join("<br>"))
    }

    /// The `<img>` the web UI writes for a pasted image (fixtures/log-images-probe.json).
    pub fn image_html(id: &str, secret: &str, image: &LogImage, base: &str) -> String {
        let src = format!(
            "{base}/pages/ajax.document.php?operation=download_inlineimage&id={id}&s={secret}"
        );
        let mut attrs = vec![format!("src=\"{}\"", Self::escape(&src))];
        if let Some((w, h)) = image.shown_size() {
            attrs.push(format!(
                "width=\"{w}\" height=\"{h}\" style=\"aspect-ratio:{w}/{h}\""
            ));
        }
        attrs.push(format!(
            "data-img-id=\"{}\" data-img-secret=\"{}\"",
            Self::escape(id),
            Self::escape(secret)
        ));
        format!("<p><img {}></p>", attrs.join(" "))
    }

    pub fn escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&#39;"),
                other => out.push(other),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_from_headers() {
        // 1×1 PNG.
        let png = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
            .unwrap();
        assert_eq!(LogImage::pixel_size(&png), Some((1, 1)));
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0x40, 0x01, 0xF0, 0x00]);
        assert_eq!(LogImage::pixel_size(&gif), Some((320, 240)));
        // SOI, an APP0 of length 4, then SOF0 with height 600, width 1600.
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x02,
            0x58, 0x06, 0x40, 0x03,
        ];
        assert_eq!(LogImage::pixel_size(&jpeg), Some((1600, 600)));
        let image = LogImage::new(jpeg.to_vec(), "a.jpg").unwrap();
        assert_eq!(image.shown_size(), Some((800, 300)));
        assert_eq!(LogImage::pixel_size(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]), None);
    }

    #[test]
    fn uuid_shape() {
        let id = new_uuid();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
    }
}
