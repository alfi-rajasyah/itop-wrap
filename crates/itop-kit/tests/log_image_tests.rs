//! Port of Tests/ITopKitTests/LogImageTests.swift.
//!
//! Shapes from fixtures/log-images-probe.json: a web-pasted log image is an InlineImage tied to
//! the ticket (item_class, item_id, item_org_id) with a 32-hex secret, and the entry's HTML has
//! <img src="…/pages/ajax.document.php?operation=download_inlineimage&id=…&s=…" data-img-id data-img-secret>.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use itop_kit::log_image::Failure as ImageFailure;
use itop_kit::stub::{RecordedRequest, StubReply, StubServer};
use itop_kit::ticket_writes::Failure;
use itop_kit::{
    Credentials, ITopClient, ITopError, LogImage, ServerAddress, Ticket, TicketClass, TicketWrites,
    WriteError,
};

fn ticket_with(key: &str) -> Ticket {
    Ticket {
        key: key.into(),
        ticket_class: TicketClass::UserRequest,
        r#ref: format!("R-00000{key}"),
        title: "t".into(),
        status: "assigned".into(),
        priority: 3,
        caller_name: String::new(),
        org_name: String::new(),
        org_id: "3".into(),
        team_id: "0".into(),
        team_name: String::new(),
        agent_id: "2".into(),
        agent_name: String::new(),
        start_date: None,
        last_update: None,
    }
}

fn ticket() -> Ticket {
    ticket_with("7")
}

/// A 1×1 PNG.
fn png() -> Vec<u8> {
    STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
        .unwrap()
}

fn image(name: &str) -> LogImage {
    LogImage::new(png(), name).unwrap()
}

fn client(stub: &StubServer, writes: bool, allow: Option<&[&str]>) -> ITopClient {
    ITopClient::with_writes(
        ServerAddress::new("itop.example.com/itop").unwrap(),
        Credentials::new("a", "p"),
        stub.transport(),
        writes,
        allow.map(|keys| keys.iter().map(|k| k.to_string()).collect()),
        false,
    )
}

fn writes(stub: &StubServer) -> TicketWrites {
    TicketWrites::new(client(stub, true, None))
}

const CREATED: &str = r#"{"code":0,"message":"","objects":{"InlineImage::55":{"class":"InlineImage","key":"55","code":0,"message":"created","fields":{"id":"55","secret":"0123456789abcdef0123456789abcdef"}}}}"#;
const UPDATED: &str = r#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"updated","fields":{"id":"7"}}}}"#;

/// Answers creates with `create` and updates with `UPDATED`.
fn server(create: &'static str) -> StubServer {
    StubServer::with_handler(move |request| {
        StubReply::json(if request.operation().as_deref() == Some("core/create") {
            create
        } else {
            UPDATED
        })
    })
}

fn add_item(r: &RecordedRequest, field: &str) -> serde_json::Value {
    r.json_data()["fields"][field]["add_item"].clone()
}

fn message(r: &RecordedRequest, field: &str) -> String {
    add_item(r, field)["message"].as_str().unwrap().to_string()
}

mod log_image_tests {
    use super::*;

    /// The image is uploaded as an InlineImage on the ticket, then the entry is added as HTML that shows it
    #[tokio::test]
    async fn upload_then_entry() {
        let stub = server(CREATED);
        let image = LogImage::new(png(), "screenshot.png").unwrap();
        writes(&stub)
            .add_log_entry_with_images(&ticket(), "See the error:", &[image], false)
            .await
            .unwrap();

        let requests = stub.requests();
        assert_eq!(requests.len(), 2);
        let create = requests[0].json_data();
        assert_eq!(create["operation"], "core/create");
        assert_eq!(create["class"], "InlineImage");
        assert_eq!(create["comment"], TicketWrites::COMMENT);
        assert!(
            create["output_fields"]
                .as_str()
                .is_some_and(|f| f.split(',').any(|x| x == "secret"))
        );
        let fields = &create["fields"];
        assert_eq!(fields["item_class"], "UserRequest");
        assert_eq!(fields["item_id"], "7");
        assert_eq!(fields["item_org_id"], "3");
        let contents = &fields["contents"];
        assert_eq!(contents["mimetype"], "image/png");
        assert_eq!(contents["filename"], "screenshot.png");
        assert_eq!(contents["data"], STANDARD.encode(png()).as_str());

        let update = &requests[1];
        assert_eq!(update.operation().as_deref(), Some("core/update"));
        let add = add_item(update, "public_log");
        assert_eq!(add["format"], "html");
        let html = add["message"].as_str().unwrap();
        assert!(html.starts_with("<p>See the error:</p>"));
        assert!(html.contains(r#"src="https://itop.example.com/itop/pages/ajax.document.php?operation=download_inlineimage&amp;id=55&amp;s=0123456789abcdef0123456789abcdef""#), "{html}");
        assert!(html.contains(r#"data-img-id="55""#));
        assert!(html.contains(r#"data-img-secret="0123456789abcdef0123456789abcdef""#));
        assert!(html.contains(r#"width="1""#) && html.contains(r#"height="1""#));
    }

    // NOT PORTED: roundTrip ("The entry iTop Wrap sends reads back as the same text and the
    // uploaded image"). It parses the sent HTML with `RichText.parse` and `HTMLText.plain`
    // (RichText.swift), which the reads module ports; it belongs with RichText's tests once both
    // halves are merged. The exact HTML is pinned here by upload_then_entry and escaping.

    /// Text is escaped and keeps its line breaks
    #[test]
    fn escaping() {
        for (text, expected) in [
            (
                "<b>bold</b> & \"q\" 'a'",
                "<p>&lt;b&gt;bold&lt;/b&gt; &amp; &quot;q&quot; &#39;a&#39;</p>",
            ),
            ("Line one\nLine two", "<p>Line one<br>Line two</p>"),
            ("Para one\n\nPara two", "<p>Para one<br><br>Para two</p>"),
        ] {
            assert_eq!(TicketWrites::html(text), expected);
        }
    }

    /// An entry with only an image is fine
    #[tokio::test]
    async fn image_only() {
        let stub = server(CREATED);
        writes(&stub)
            .add_log_entry_with_images(&ticket(), "  \n", &[image("a.png")], false)
            .await
            .unwrap();
        assert!(message(&stub.requests()[1], "public_log").starts_with("<p><img "));
    }

    /// Several images are uploaded in order and shown in that order
    #[tokio::test]
    async fn several() {
        let counter = Arc::new(AtomicUsize::new(0));
        let stub = StubServer::with_handler(move |request| {
            if request.operation().as_deref() != Some("core/create") {
                return StubReply::json(UPDATED);
            }
            let n = counter.fetch_add(1, Ordering::SeqCst) + 60;
            StubReply::json(&format!(
                r#"{{"code":0,"message":"","objects":{{"InlineImage::{n}":{{"class":"InlineImage","key":"{n}","code":0,"message":"created","fields":{{"id":"{n}","secret":"s{n}"}}}}}}}}"#
            ))
        });
        let images: Vec<LogImage> = (0..3).map(|i| image(&format!("{i}.png"))).collect();
        writes(&stub)
            .add_log_entry_with_images(&ticket(), "x", &images, false)
            .await
            .unwrap();
        let html = message(&stub.requests()[3], "public_log");
        let ids: Vec<&str> = html
            .split("data-img-id=\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect();
        assert_eq!(ids, ["60", "61", "62"]);
    }

    /// Without a secret in iTop's answer, the one iTop Wrap sent is used
    #[tokio::test]
    async fn secret_fallback() {
        let stub = server(
            r#"{"code":0,"message":"","objects":{"InlineImage::55":{"class":"InlineImage","key":"55","code":0,"message":"created","fields":{"id":"55"}}}}"#,
        );
        writes(&stub)
            .add_log_entry_with_images(&ticket(), "x", &[image("a.png")], false)
            .await
            .unwrap();
        let requests = stub.requests();
        let sent = requests[0].json_data()["fields"]["secret"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(sent.len() == 32 && sent.chars().all(|c| c.is_ascii_hexdigit()));
        let html = message(&requests[1], "public_log");
        assert!(html.contains(&format!(r#"data-img-secret="{sent}""#)));
    }

    /// Only PNG, JPEG and GIF are accepted, judged by the bytes, not the name
    #[test]
    fn types() {
        assert_eq!(LogImage::new(png(), "x.jpg").unwrap().mimetype, "image/png");
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        jpeg.extend([0u8; 20]);
        assert_eq!(LogImage::new(jpeg, "a").unwrap().mimetype, "image/jpeg");
        let mut gif = b"GIF89a".to_vec();
        gif.extend([0u8; 20]);
        assert_eq!(LogImage::new(gif, "a").unwrap().mimetype, "image/gif");
        assert_eq!(
            LogImage::new(b"hello".to_vec(), "notes.png"),
            Err(ImageFailure::UnsupportedType {
                filename: "notes.png".into()
            })
        );
    }

    /// An image over the size limit is refused before anything is sent
    #[test]
    fn too_large() {
        let mut big = png();
        big.extend(vec![0u8; LogImage::MAX_BYTES]);
        assert_eq!(
            LogImage::new(big, "big.png"),
            Err(ImageFailure::TooLarge {
                filename: "big.png".into(),
                max_mb: (LogImage::MAX_BYTES / 1_048_576) as i64
            })
        );
    }

    /// More images than the limit are refused before anything is sent
    #[tokio::test]
    async fn too_many() {
        let stub = server(CREATED);
        let images: Vec<LogImage> = (0..=LogImage::MAX_PER_ENTRY)
            .map(|i| image(&format!("{i}.png")))
            .collect();
        assert_eq!(
            writes(&stub)
                .add_log_entry_with_images(&ticket(), "x", &images, false)
                .await,
            Err(WriteError::Failure(Failure::TooManyImages {
                max: LogImage::MAX_PER_ENTRY as i64
            }))
        );
        assert!(stub.requests().is_empty());
    }

    /// When iTop refuses the image, its message comes back and no entry is added
    #[tokio::test]
    async fn image_refused() {
        let stub = server(r#"{"code":100,"message":"Error: contents: the file is too big"}"#);
        assert_eq!(
            writes(&stub)
                .add_log_entry_with_images(&ticket(), "x", &[image("a.png")], false)
                .await,
            Err(WriteError::ITop(ITopError::Server {
                code: 100,
                message: "Error: contents: the file is too big".into()
            }))
        );
        assert_eq!(stub.requests().len(), 1);
    }

    /// A per-object refusal of the image also stops before the entry
    #[tokio::test]
    async fn image_refused_per_object() {
        let stub = server(
            r#"{"code":0,"message":"","objects":{"InlineImage::0":{"class":"InlineImage","key":"0","code":100,"message":"Error: not allowed to create InlineImage"}}}"#,
        );
        assert_eq!(
            writes(&stub)
                .add_log_entry_with_images(&ticket(), "x", &[image("a.png")], false)
                .await,
            Err(WriteError::ITop(ITopError::Server {
                code: 100,
                message: "Error: not allowed to create InlineImage".into()
            }))
        );
        assert_eq!(stub.requests().len(), 1);
    }

    /// With writes off nothing is uploaded
    #[tokio::test]
    async fn writes_off() {
        let stub = server(CREATED);
        assert_eq!(
            TicketWrites::new(client(&stub, false, None))
                .add_log_entry_with_images(&ticket(), "x", &[image("a.png")], false)
                .await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert!(stub.requests().is_empty());
    }

    /// The write allowlist lets the upload through for an allowed ticket only
    #[tokio::test]
    async fn allowlist() {
        let stub = server(CREATED);
        let writes = TicketWrites::new(client(&stub, true, Some(&["UserRequest::7"])));
        writes
            .add_log_entry_with_images(&ticket(), "x", &[image("a.png")], false)
            .await
            .unwrap();
        assert_eq!(stub.requests().len(), 2);

        let other = ticket_with("8");
        assert_eq!(
            writes
                .add_log_entry_with_images(&other, "x", &[image("a.png")], false)
                .await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert_eq!(stub.requests().len(), 2);
    }

    /// Without images the entry is still sent as plain text
    #[tokio::test]
    async fn no_images() {
        let stub = server(CREATED);
        writes(&stub)
            .add_log_entry_with_images(&ticket(), "Hello", &[], false)
            .await
            .unwrap();
        let requests = stub.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(add_item(&requests[0], "public_log")["format"], "text");
    }
}
