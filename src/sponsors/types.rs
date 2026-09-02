use serde::{Deserialize, Serialize};

use crate::links::types::deserialize_d1_bool;

/// Row from the `sponsor_packages` table.
///
/// `rename_all = "camelCase"` exposes canonical JSON shape to API clients;
/// per-field `alias` keeps SQLite snake_case column names deserializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackage {
    pub id: String,
    #[serde(alias = "event_slug")]
    pub event_slug: String,
    pub name: String,
    pub advantage: String,
    /// `digital` or `onsite`; constrained by the table CHECK.
    pub category: String,
    /// Group reference; supersedes category for API/public grouping.
    /// NULL = ungrouped (legacy rows only).
    #[serde(alias = "group_id")]
    pub group_id: Option<String>,
    #[serde(alias = "price_idr")]
    pub price_idr: i64,
    /// Optional package-unlock minimum spend (whole IDR); null = no threshold.
    /// Eligibility against a sponsor's total spend is computed by the frontend.
    #[serde(alias = "minimum_spend_idr")]
    pub minimum_spend_idr: Option<i64>,
    /// Capacity cap in 1..=10000; null = unlimited. Table CHECK enforces
    /// reserved_sponsors <= max_sponsors whenever the cap is set.
    #[serde(alias = "max_sponsors")]
    pub max_sponsors: Option<i64>,
    /// Slots already reserved against the cap; always within 0..=10000.
    #[serde(alias = "reserved_sponsors")]
    pub reserved_sponsors: i64,
    #[serde(alias = "is_unlocked", deserialize_with = "deserialize_d1_bool")]
    pub is_unlocked: bool,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

/// Entry of PUT /api/admin/events/:eventSlug/sponsor-packages.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackageUpdate {
    pub id: String,
    pub price_idr: i64,
    /// Optional threshold in the same 1..=1_000_000_000 bounds; null clears it.
    pub minimum_spend_idr: Option<i64>,
    /// Optional capacity cap in 1..=10000; null clears it (unlimited).
    pub max_sponsors: Option<i64>,
    /// Reserved slots in 0..=10000; must be <= maxSponsors when the cap is set.
    pub reserved_sponsors: i64,
    pub is_unlocked: bool,
    /// Target group id; must reference an existing group for the event.
    /// Null/missing clears the assignment.
    pub group_id: Option<String>,
}

/// POST /api/admin/events/:eventSlug/sponsor-packages body. The server owns
/// id, display_order, and the legacy category; clients send display fields
/// plus the target group and price only.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackageCreate {
    pub name: String,
    pub advantage: String,
    /// Target group id; must reference an existing group for the event.
    pub group_id: String,
    pub price_idr: i64,
}

/// Entry of the groups array of the admin PUT body (rename/reorder only;
/// groups are created/deleted only via migrations in this iteration).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackageGroupUpdate {
    pub id: String,
    pub label: String,
    pub display_order: i32,
}

/// POST /api/admin/events/:eventSlug/sponsor-groups body. The server owns
/// id and display_order; clients only send the label.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackageGroupCreate {
    pub label: String,
}

/// Row from the `sponsor_package_groups` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackageGroup {
    pub id: String,
    #[serde(alias = "event_slug")]
    pub event_slug: String,
    pub label: String,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

/// PUT /api/admin/events/:eventSlug/sponsor-packages body. Both arrays must
/// be present; either may be empty, but not both.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackageBatchUpdate {
    pub packages: Vec<SponsorPackageUpdate>,
    pub groups: Vec<SponsorPackageGroupUpdate>,
}

/// Shared response shape of the public GET and admin PUT.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SponsorPackagesResponse<'a> {
    pub event_slug: &'a str,
    pub currency: &'a str,
    /// Groups ordered by display_order; empty when the event has none seeded.
    pub groups: &'a [SponsorPackageGroup],
    pub packages: &'a [SponsorPackage],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sponsor_package_deserializes_d1_row() {
        let row = r#"{
            "id": "web-logo",
            "event_slug": "community-day-2026",
            "name": "Website Logo",
            "advantage": "Exposure",
            "category": "digital",
            "price_idr": 2500000,
            "minimum_spend_idr": null,
            "max_sponsors": null,
            "reserved_sponsors": 0,
            "is_unlocked": 1,
            "display_order": 1,
            "updated_at": "2026-09-02 12:34:56"
        }"#;
        let pkg: SponsorPackage = serde_json::from_str(row).unwrap();
        assert!(pkg.is_unlocked);
        assert_eq!(pkg.price_idr, 2_500_000);
        assert_eq!(pkg.minimum_spend_idr, None);
        assert_eq!(pkg.max_sponsors, None);
        assert_eq!(pkg.reserved_sponsors, 0);

        let locked = row.replace("\"is_unlocked\": 1", "\"is_unlocked\": 0");
        let pkg: SponsorPackage = serde_json::from_str(&locked).unwrap();
        assert!(!pkg.is_unlocked);

        let thresholded = row.replace(
            "\"minimum_spend_idr\": null",
            "\"minimum_spend_idr\": 10000000",
        );
        let pkg: SponsorPackage = serde_json::from_str(&thresholded).unwrap();
        assert_eq!(pkg.minimum_spend_idr, Some(10_000_000));
    }

    #[test]
    fn sponsor_package_deserializes_d1_capacity_row() {
        // D1 shape: snake_case, numeric booleans, NULL for unlimited.
        let row = r#"{
            "id": "lanyard",
            "event_slug": "community-day-2026",
            "name": "Lanyard",
            "advantage": "Presence",
            "category": "onsite",
            "price_idr": 7500000,
            "minimum_spend_idr": null,
            "max_sponsors": 5,
            "reserved_sponsors": 3,
            "is_unlocked": 1,
            "display_order": 6,
            "updated_at": "2026-09-02 12:34:56"
        }"#;
        let pkg: SponsorPackage = serde_json::from_str(row).unwrap();
        assert_eq!(pkg.max_sponsors, Some(5));
        assert_eq!(pkg.reserved_sponsors, 3);

        let unlimited = row.replace("\"max_sponsors\": 5", "\"max_sponsors\": null");
        let pkg: SponsorPackage = serde_json::from_str(&unlimited).unwrap();
        assert_eq!(pkg.max_sponsors, None);
    }

    #[test]
    fn sponsor_package_serializes_camel_case() {
        let pkg = SponsorPackage {
            id: "tshirt".into(),
            event_slug: "community-day-2026".into(),
            name: "T-Shirt".into(),
            advantage: "Merch".into(),
            category: "onsite".into(),
            group_id: None,
            price_idr: 6_000_000,
            minimum_spend_idr: Some(10_000_000),
            max_sponsors: Some(5),
            reserved_sponsors: 2,
            is_unlocked: true,
            display_order: 5,
            updated_at: "2026-09-02 12:34:56".into(),
        };
        let json = serde_json::to_value(&pkg).unwrap();
        assert_eq!(json["priceIdr"], 6_000_000);
        assert_eq!(json["minimumSpendIdr"], 10_000_000);
        assert_eq!(json["maxSponsors"], 5);
        assert_eq!(json["reservedSponsors"], 2);
        assert_eq!(json["isUnlocked"], true);
        assert_eq!(json["eventSlug"], "community-day-2026");
        assert_eq!(json["displayOrder"], 5);
        assert!(json.get("createdAt").is_none());

        let null_json = serde_json::to_value(SponsorPackage {
            minimum_spend_idr: None,
            max_sponsors: None,
            ..pkg
        })
        .unwrap();
        assert!(null_json["minimumSpendIdr"].is_null());
        assert!(null_json["maxSponsors"].is_null());
        assert_eq!(null_json["reservedSponsors"], 2);
    }

    #[test]
    fn update_entry_rejects_non_bool_unlock_and_float_price() {
        let base = r#"{"id":"web-logo","priceIdr":2500000,"reservedSponsors":0,"isUnlocked":true}"#;
        assert!(serde_json::from_str::<SponsorPackageUpdate>(base).is_ok());
        assert!(serde_json::from_str::<SponsorPackageUpdate>(
            r#"{"id":"web-logo","priceIdr":2500000.5,"reservedSponsors":0,"isUnlocked":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<SponsorPackageUpdate>(
            r#"{"id":"web-logo","priceIdr":2500000,"reservedSponsors":0,"isUnlocked":"true"}"#
        )
        .is_err());
    }

    #[test]
    fn update_entry_capacity_optional_max_required_reserved() {
        // Missing maxSponsors -> None (unlimited); missing reservedSponsors -> error.
        let missing_max: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"reservedSponsors":0,"isUnlocked":true}"#,
        )
        .unwrap();
        assert_eq!(missing_max.max_sponsors, None);
        let null_max: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"maxSponsors":null,"reservedSponsors":2,"isUnlocked":true}"#,
        )
        .unwrap();
        assert_eq!(null_max.max_sponsors, None);
        let set: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"maxSponsors":5,"reservedSponsors":2,"isUnlocked":true}"#,
        )
        .unwrap();
        assert_eq!(set.max_sponsors, Some(5));
        assert_eq!(set.reserved_sponsors, 2);
        assert!(serde_json::from_str::<SponsorPackageUpdate>(
            r#"{"id":"web-logo","priceIdr":2500000,"maxSponsors":5,"isUnlocked":true}"#
        )
        .is_err());
    }

    #[test]
    fn update_entry_rejects_float_capacity() {
        for body in [
            r#"{"id":"web-logo","priceIdr":2500000,"maxSponsors":5.5,"reservedSponsors":2,"isUnlocked":true}"#,
            r#"{"id":"web-logo","priceIdr":2500000,"maxSponsors":5,"reservedSponsors":2.5,"isUnlocked":true}"#,
        ] {
            assert!(serde_json::from_str::<SponsorPackageUpdate>(body).is_err());
        }
    }

    #[test]
    fn update_entry_threshold_optional_but_typed() {
        // Missing field -> None; explicit null -> None.
        let missing: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"reservedSponsors":0,"isUnlocked":true}"#,
        )
        .unwrap();
        assert_eq!(missing.minimum_spend_idr, None);
        let null: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"minimumSpendIdr":null,"reservedSponsors":0,"isUnlocked":true}"#,
        )
        .unwrap();
        assert_eq!(null.minimum_spend_idr, None);
        let set: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"minimumSpendIdr":5000000,"reservedSponsors":0,"isUnlocked":true}"#,
        )
        .unwrap();
        assert_eq!(set.minimum_spend_idr, Some(5_000_000));
        // Floats are rejected by the i64 field.
        assert!(serde_json::from_str::<SponsorPackageUpdate>(
            r#"{"id":"web-logo","priceIdr":2500000,"minimumSpendIdr":5000000.5,"reservedSponsors":0,"isUnlocked":true}"#
        )
        .is_err());
    }

    #[test]
    fn package_group_id_optional_both_directions() {
        // Missing group_id -> None (legacy D1 rows / old bodies).
        let missing: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"reservedSponsors":0,"isUnlocked":true}"#,
        )
        .unwrap();
        assert_eq!(missing.group_id, None);
        let set: SponsorPackageUpdate = serde_json::from_str(
            r#"{"id":"web-logo","priceIdr":2500000,"reservedSponsors":0,"isUnlocked":true,"groupId":"digital-media"}"#,
        )
        .unwrap();
        assert_eq!(set.group_id.as_deref(), Some("digital-media"));

        // Row serialization: camelCase groupId, null when ungrouped.
        let pkg = SponsorPackage {
            id: "web-logo".into(),
            event_slug: "community-day-2026".into(),
            name: "Website Logo".into(),
            advantage: "Exposure".into(),
            category: "digital".into(),
            group_id: Some("digital-media".into()),
            price_idr: 2_500_000,
            minimum_spend_idr: None,
            max_sponsors: None,
            reserved_sponsors: 0,
            is_unlocked: true,
            display_order: 1,
            updated_at: "2026-09-02 12:34:56".into(),
        };
        let json = serde_json::to_value(&pkg).unwrap();
        assert_eq!(json["groupId"], "digital-media");
        let ungrouped = serde_json::to_value(SponsorPackage {
            group_id: None,
            ..pkg
        })
        .unwrap();
        assert!(ungrouped["groupId"].is_null());

        // D1 snake_case row with group_id deserializes.
        let row = r#"{
            "id": "tshirt", "event_slug": "community-day-2026",
            "name": "T-Shirt", "advantage": "Merch", "category": "onsite",
            "group_id": "onsite-physical", "price_idr": 6000000,
            "minimum_spend_idr": null, "max_sponsors": null,
            "reserved_sponsors": 0, "is_unlocked": 1,
            "display_order": 5, "updated_at": "2026-09-02 12:34:56"
        }"#;
        let pkg: SponsorPackage = serde_json::from_str(row).unwrap();
        assert_eq!(pkg.group_id.as_deref(), Some("onsite-physical"));
    }

    #[test]
    fn group_row_snake_in_camel_out() {
        let row = r#"{
            "id": "digital-media",
            "event_slug": "community-day-2026",
            "label": "Digital & Media",
            "display_order": 1,
            "updated_at": "2026-09-02 12:34:56"
        }"#;
        let group: SponsorPackageGroup = serde_json::from_str(row).unwrap();
        assert_eq!(group.label, "Digital & Media");
        let json = serde_json::to_value(&group).unwrap();
        assert_eq!(json["eventSlug"], "community-day-2026");
        assert_eq!(json["displayOrder"], 1);
        assert_eq!(json["updatedAt"], "2026-09-02 12:34:56");
        assert!(json.get("createdAt").is_none());
    }

    #[test]
    fn group_update_requires_all_fields() {
        let ok: SponsorPackageGroupUpdate = serde_json::from_str(
            r#"{"id":"onsite-physical","label":"On-Site & Physical","displayOrder":2}"#,
        )
        .unwrap();
        assert_eq!(ok.display_order, 2);
        assert!(serde_json::from_str::<SponsorPackageGroupUpdate>(
            r#"{"id":"onsite-physical","displayOrder":2}"#
        )
        .is_err());
        assert!(serde_json::from_str::<SponsorPackageGroupUpdate>(
            r#"{"id":"onsite-physical","label":"X","displayOrder":2.5}"#
        )
        .is_err());
    }

    #[test]
    fn create_entry_requires_typed_fields() {
        let ok: SponsorPackageCreate = serde_json::from_str(
            r#"{"name":"Lanyard","advantage":"Lanyard branding","groupId":"onsite-physical","priceIdr":7500000}"#,
        )
        .unwrap();
        assert_eq!(ok.name, "Lanyard");
        assert_eq!(ok.advantage, "Lanyard branding");
        assert_eq!(ok.group_id, "onsite-physical");
        assert_eq!(ok.price_idr, 7_500_000);

        // Missing any field is rejected.
        for body in [
            r#"{"advantage":"X","groupId":"g","priceIdr":1}"#,
            r#"{"name":"X","groupId":"g","priceIdr":1}"#,
            r#"{"name":"X","advantage":"Y","priceIdr":1}"#,
            r#"{"name":"X","advantage":"Y","groupId":"g"}"#,
            r#"{}"#,
        ] {
            assert!(
                serde_json::from_str::<SponsorPackageCreate>(body).is_err(),
                "body {body} should not deserialize"
            );
        }
        // Field types are enforced: float price and non-string name rejected.
        assert!(serde_json::from_str::<SponsorPackageCreate>(
            r#"{"name":"X","advantage":"Y","groupId":"g","priceIdr":1.5}"#
        )
        .is_err());
        assert!(serde_json::from_str::<SponsorPackageCreate>(
            r#"{"name":42,"advantage":"Y","groupId":"g","priceIdr":1}"#
        )
        .is_err());
    }

    #[test]
    fn group_create_requires_label() {
        let ok: SponsorPackageGroupCreate =
            serde_json::from_str(r#"{"label":"Digital & Media"}"#).unwrap();
        assert_eq!(ok.label, "Digital & Media");
        assert!(serde_json::from_str::<SponsorPackageGroupCreate>(r#"{}"#).is_err());
        assert!(serde_json::from_str::<SponsorPackageGroupCreate>(r#"{"label":42}"#).is_err());
    }

    #[test]
    fn batch_body_requires_both_arrays() {
        let full: SponsorPackageBatchUpdate = serde_json::from_str(
            r#"{"packages":[],"groups":[{"id":"digital-media","label":"Digital & Media","displayOrder":1}]}"#,
        )
        .unwrap();
        assert!(full.packages.is_empty());
        assert_eq!(full.groups.len(), 1);
        // Missing groups array is rejected: both arrays must be present.
        assert!(serde_json::from_str::<SponsorPackageBatchUpdate>(
            r#"{"packages":[{"id":"web-logo","priceIdr":2500000,"reservedSponsors":0,"isUnlocked":true}]}"#
        )
        .is_err());
    }
}
