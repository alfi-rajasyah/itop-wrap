//! Port of Tests/ITopKitTests/OrgQueryTests.swift: only the test that needs nothing but
//! `ITopObject` and `Ticket`. The rest need OrgService (Organization.swift) and are ported with it.

use itop_kit::{ITopObject, JsonValue, Ticket};

mod org_filter_tests {
    use super::*;

    /// Tickets now carry their org id, for opening the org view
    #[test]
    fn org_id() {
        let json = JsonValue::parse_str(
            r#"{"class":"UserRequest","key":"5","fields":{"ref":"R-5","org_id":"14","org_id_friendlyname":"Org A"}}"#,
        )
        .unwrap();
        let object = ITopObject::from_json(&json).expect("an object");
        assert_eq!(
            Ticket::from_object(&object).map(|t| t.org_id),
            Some("14".to_string())
        );
        assert!(Ticket::LIST_FIELDS.contains(&"org_id"));
    }
}
