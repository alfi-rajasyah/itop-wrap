//! Port of Tests/ITopKitTests/RichTextTests.swift. One module per Swift suite; Swift
//! parameterised tests loop over the same arguments. URLs are compared as their absolute strings.

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{
    AttachmentRef, Credentials, HtmlText, ITopClient, InlineImageError, InlineImageLoader,
    RichBlock, RichImage, RichRun, RichText, ServerAddress, Ticket, TicketClass,
};

const BASE: &str = "https://itop.example.com/itop";

fn client(stub: &StubServer) -> ITopClient {
    ITopClient::new(
        ServerAddress::new("https://itop.example.com/itop").unwrap(),
        Credentials::new("a", "p"),
        stub.transport(),
    )
}

mod rich_text_tests {
    use super::*;

    /// Links become runs with a URL; the rest is plain text
    #[test]
    fn links() {
        let blocks = RichText::parse(
            r#"<p>See <a href="https://kb.example.com/1">the KB article</a>.</p>"#,
            BASE,
        );
        assert_eq!(
            blocks,
            [RichBlock::Text(vec![
                RichRun::text("See "),
                RichRun::link("the KB article", "https://kb.example.com/1"),
                RichRun::text("."),
            ])]
        );
    }

    /// Only web and email links are kept; the text of others stays
    #[test]
    fn unsafe_links() {
        for html in [
            r#"<a href="javascript:alert(1)">click</a>"#,
            r#"<a href="file:///etc/passwd">click</a>"#,
            r#"<a>click</a>"#,
        ] {
            assert_eq!(
                RichText::parse(html, BASE),
                [RichBlock::Text(vec![RichRun::text("click")])],
                "{html}"
            );
        }
    }

    /// Email links are kept
    #[test]
    fn mailto() {
        assert_eq!(
            RichText::parse(r#"<a href="mailto:helpdesk@example.com">mail us</a>"#, BASE),
            [RichBlock::Text(vec![RichRun::link(
                "mail us",
                "mailto:helpdesk@example.com"
            )])]
        );
    }

    /// Relative links resolve against your iTop
    #[test]
    fn relative() {
        let blocks = RichText::parse(
            r#"<a href="pages/UI.php?operation=details&amp;id=5">R-5</a>"#,
            BASE,
        );
        assert_eq!(
            blocks,
            [RichBlock::Text(vec![RichRun::link(
                "R-5",
                "https://itop.example.com/itop/pages/UI.php?operation=details&id=5"
            )])]
        );
    }

    /// A pasted iTop image becomes an inline image by id
    #[test]
    fn inline_image() {
        for html in [
            r#"<img src="https://itop.example.com/itop/pages/ajax.document.php?operation=download_inlineimage&amp;id=42&amp;s=abc" data-img-id="42">"#,
            r#"<img src="pages/ajax.document.php?operation=download_inlineimage&amp;id=42&amp;s=abc">"#,
        ] {
            assert_eq!(
                RichText::parse(html, BASE),
                [RichBlock::Image(RichImage::Inline { id: "42".into() })],
                "{html}"
            );
        }
    }

    /// An image from another host is never loaded: it's an inline chip with its host and a link
    #[test]
    fn external() {
        let blocks = RichText::parse(
            r#"<img src="https://cdn.example.net/pic.png" alt="diagram">"#,
            BASE,
        );
        assert_eq!(
            blocks,
            [RichBlock::Text(vec![RichRun::outside_image(
                "https://cdn.example.net/pic.png"
            )])]
        );
    }

    /// A small image from another host (a signature icon) stays in the line as an icon, still not loaded
    #[test]
    fn small_external_is_inline() {
        let url = "https://lh5.googleusercontent.com/phone";
        for html in [
            r#"<p><img src="https://lh5.googleusercontent.com/phone" width="16" height="16"> : +62 878</p>"#,
            r#"<p><img src="https://lh5.googleusercontent.com/phone" style="width: 12px; height:12px"> : +62 878</p>"#,
        ] {
            assert_eq!(
                RichText::parse(html, BASE),
                [RichBlock::Text(vec![
                    RichRun::icon(url),
                    RichRun::text(" : +62 878")
                ])],
                "{html}"
            );
        }
    }

    /// A large or unsized outside image (a signature logo) stays in the text as a chip, not a block
    #[test]
    fn large_external_is_inline() {
        for html in [
            r#"<p><a href="https://www.example.com">www.example.com</a><br><img src="https://lh3.googleusercontent.com/logo" width="600" height="120"></p>"#,
            r#"<p><a href="https://www.example.com">www.example.com</a><br><img src="https://lh3.googleusercontent.com/logo"></p>"#,
        ] {
            assert_eq!(
                RichText::parse(html, BASE),
                [RichBlock::Text(vec![
                    RichRun::link("www.example.com", "https://www.example.com"),
                    RichRun::text("\n"),
                    RichRun::outside_image("https://lh3.googleusercontent.com/logo"),
                ])],
                "{html}"
            );
        }
    }

    /// The chip is labelled with the host
    #[test]
    fn chip_label() {
        assert_eq!(
            RichRun::outside_image("https://lh6.googleusercontent.com/x").text,
            "lh6.googleusercontent.com"
        );
    }

    /// A data: image is decoded in place, with no network
    #[test]
    fn data_image() {
        let blocks = RichText::parse(r#"<img src="data:image/png;base64,QUJD">"#, BASE);
        assert_eq!(blocks, [RichBlock::Image(RichImage::Data(b"ABC".to_vec()))]);
    }

    /// Text around an image stays in order, trimmed, with empty text dropped
    #[test]
    fn order() {
        let blocks = RichText::parse(
            r#"<p>Before</p><figure><img src="pages/ajax.document.php?operation=download_inlineimage&amp;id=7"></figure><p>After</p>"#,
            BASE,
        );
        assert_eq!(
            blocks,
            [
                RichBlock::Text(vec![RichRun::text("Before")]),
                RichBlock::Image(RichImage::Inline { id: "7".into() }),
                RichBlock::Text(vec![RichRun::text("After")]),
            ]
        );
    }

    /// Without images or links it reads the same as the plain-text version
    #[test]
    fn matches_plain() {
        for html in [
            "<p>Hello<br>world</p><p>Second</p>",
            "<ul><li>One</li><li>Two</li></ul>",
            "<p>[text]<br>\u{00A0}</p>",
            "Fish &amp; chips",
        ] {
            let text = RichText::parse(html, BASE)
                .iter()
                .filter_map(|block| match block {
                    RichBlock::Text(runs) => {
                        Some(runs.iter().map(|r| r.text.as_str()).collect::<String>())
                    }
                    RichBlock::Image(_) => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            assert_eq!(text, HtmlText::plain(html), "{html:?}");
        }
    }

    /// Empty HTML gives no blocks
    #[test]
    fn empty() {
        assert!(RichText::parse("", BASE).is_empty());
        assert!(RichText::parse("<p> </p>", BASE).is_empty());
    }
}

mod inline_image_tests {
    use super::*;

    /// An inline image is read through REST by id and decoded
    #[tokio::test]
    async fn load() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"Found: 1","objects":{"InlineImage::42":{"class":"InlineImage","key":"42","fields":{"contents":{"data":"QUJD","mimetype":"image/png","filename":"x.png"}}}}}"#,
        ));
        let loader = InlineImageLoader::new(client(&stub));

        let image = loader.image("42").await.unwrap();

        assert_eq!(image.data, b"ABC");
        assert_eq!(image.mimetype, "image/png");
        let r = stub.requests()[0].json_data();
        assert_eq!(r["class"].as_str(), Some("InlineImage"));
        assert_eq!(r["key"].as_str(), Some("SELECT InlineImage WHERE id = 42"));
        assert_eq!(r["output_fields"].as_str(), Some("contents"));
    }

    /// A second request for the same image comes from memory
    #[tokio::test]
    async fn cached() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"InlineImage::42":{"class":"InlineImage","key":"42","fields":{"contents":{"data":"QUJD","mimetype":"image/png"}}}}}"#,
        ));
        let loader = InlineImageLoader::new(client(&stub));
        loader.image("42").await.unwrap();
        loader.image("42").await.unwrap();
        assert_eq!(stub.requests().len(), 1);
    }

    /// A missing image or a bad id is an error, not a crash
    #[tokio::test]
    async fn missing() {
        for id in ["42", "4 OR 1=1"] {
            let stub = StubServer::new(StubReply::json(
                r#"{"code":0,"message":"Found: 0","objects":null}"#,
            ));
            let loader = InlineImageLoader::new(client(&stub));
            let result = loader.image(id).await;
            assert!(
                matches!(result, Err(InlineImageError::Failure(_))),
                "{id:?}: {result:?}"
            );
        }
    }

    /// Attachments are listed by name without downloading them
    #[tokio::test]
    async fn list() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"Attachment::3":{"class":"Attachment","key":"3","fields":{"friendlyname":"report.pdf"}}}}"#,
        ));
        let ticket = Ticket {
            key: "7".into(),
            ticket_class: TicketClass::UserRequest,
            r#ref: "R-7".into(),
            title: String::new(),
            status: "assigned".into(),
            priority: 3,
            caller_name: String::new(),
            org_name: String::new(),
            org_id: "0".into(),
            team_id: "0".into(),
            team_name: String::new(),
            agent_id: "0".into(),
            agent_name: String::new(),
            start_date: None,
            last_update: None,
        };

        let list = InlineImageLoader::new(client(&stub))
            .attachments(&ticket)
            .await
            .unwrap();

        assert_eq!(list, [AttachmentRef::new("3", "report.pdf")]);
        let r = stub.requests()[0].json_data();
        assert_eq!(
            r["key"].as_str(),
            Some("SELECT Attachment WHERE item_class = 'UserRequest' AND item_id = 7")
        );
        assert_eq!(
            r["output_fields"].as_str(),
            Some("friendlyname"),
            "the list never asks for file contents"
        );
    }

    /// Downloading an attachment returns its bytes, name and type
    #[tokio::test]
    async fn download() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"Attachment::3":{"class":"Attachment","key":"3","fields":{"contents":{"data":"QUJD","mimetype":"application/pdf","filename":"report.pdf"}}}}}"#,
        ));
        let file = InlineImageLoader::new(client(&stub))
            .download(&AttachmentRef::new("3", "report.pdf"))
            .await
            .unwrap();
        assert!(
            file.data == b"ABC"
                && file.filename == "report.pdf"
                && file.mimetype == "application/pdf"
        );
    }
}
