use std::collections::HashSet;

use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::{json_success, json_success_cors, with_cors};
use crate::sponsors::repository::{
    CreateGroupError, CreatePackageError, DeleteGroupError, DeletePackageError,
    SponsorPackageRepository, UpdatePackagesError, MAX_PACKAGES_PER_EVENT,
};
use crate::sponsors::types::{
    SponsorPackageBatchUpdate, SponsorPackageCreate, SponsorPackageGroupCreate,
    SponsorPackagesResponse,
};

const MAX_PACKAGES_PER_UPDATE: usize = 50;
const MAX_GROUPS_PER_UPDATE: usize = 20;
const MIN_PRICE_IDR: i64 = 1;
const MAX_PRICE_IDR: i64 = 1_000_000_000;
const MAX_SPONSORS: i64 = 10_000;
const MAX_GROUP_LABEL_LEN: usize = 80;
const MAX_PACKAGE_NAME_LEN: usize = 80;
const MAX_PACKAGE_ADVANTAGE_LEN: usize = 500;
const MIN_GROUP_ORDER: i32 = 1;
const MAX_GROUP_ORDER: i32 = 1_000;
const MAX_EVENT_SLUG_LEN: usize = 100;

/// GET /api/events/:eventSlug/sponsor-packages — public listing, locked rows included.
pub async fn handle_public_sponsor_packages(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let _origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SponsorPackageRepository::new(db);

    let groups = repo
        .list_groups(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let packages = repo
        .list_packages(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if packages.is_empty() {
        return Err(AppError::NotFound(format!(
            "No sponsor packages found for event {event_slug}"
        ))
        .into());
    }

    let body = SponsorPackagesResponse {
        event_slug,
        currency: "IDR",
        groups: &groups,
        packages: &packages,
    };
    let resp = json_success(&body)?;
    with_cors(resp, &config.allowed_origins)
}

/// PUT /api/admin/events/:eventSlug/sponsor-packages — batch update of
/// package fields/group assignments and group label/order (rename/reorder).
pub async fn handle_admin_update_sponsor_packages(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;

    let bytes = req.bytes().await?;
    let input: SponsorPackageBatchUpdate =
        serde_json::from_slice(&bytes).map_err(AppError::from)?;
    validate_batch(event_slug, &input)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SponsorPackageRepository::new(db);

    let (groups, packages) = repo
        .update(event_slug, &input.groups, &input.packages)
        .await
        .map_err(|e| match e {
            UpdatePackagesError::UnknownIds(ids) => AppError::BadRequest(format!(
                "unknown sponsor package id(s) for event {event_slug}: {}",
                ids.join(", ")
            )),
            UpdatePackagesError::UnknownGroupIds(ids) => AppError::BadRequest(format!(
                "unknown sponsor package group id(s) for event {event_slug}: {}",
                ids.join(", ")
            )),
            UpdatePackagesError::OrderConflict => AppError::BadRequest(
                "group displayOrder values must stay unique per event".to_string(),
            ),
            UpdatePackagesError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    console_log!(
        "sponsor packages updated: event={event_slug} package_count={} group_count={} thresholded={} capacity_limited={}",
        input.packages.len(),
        input.groups.len(),
        input
            .packages
            .iter()
            .filter(|p| p.minimum_spend_idr.is_some())
            .count(),
        input
            .packages
            .iter()
            .filter(|p| p.max_sponsors.is_some())
            .count(),
    );

    let body = SponsorPackagesResponse {
        event_slug,
        currency: "IDR",
        groups: &groups,
        packages: &packages,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// POST /api/admin/events/:eventSlug/sponsor-groups — create a group with a
/// server-generated id and display_order = max + 1. Returns the refreshed
/// canonical listing so the admin can replace its state.
pub async fn handle_admin_create_sponsor_group(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;

    let bytes = req.bytes().await?;
    let input: SponsorPackageGroupCreate =
        serde_json::from_slice(&bytes).map_err(AppError::from)?;
    validate_group_create(event_slug, &input)?;
    let label = input.label.trim();

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SponsorPackageRepository::new(db);

    let (group_id, groups, packages) =
        repo.create_group(event_slug, label)
            .await
            .map_err(|e| match e {
                CreateGroupError::DuplicateLabel => AppError::Conflict(format!(
                    "sponsor group label '{label}' already exists for event {event_slug}"
                )),
                CreateGroupError::Db(e) => AppError::Internal(e.to_string()),
            })?;

    console_log!("sponsor group created: event={event_slug} group_id={group_id}");

    let body = SponsorPackagesResponse {
        event_slug,
        currency: "IDR",
        groups: &groups,
        packages: &packages,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// POST /api/admin/events/:eventSlug/sponsor-packages — create a package in
/// an existing group with a server-generated id and display_order = max + 1.
/// Returns the refreshed canonical listing so the admin can replace its state.
pub async fn handle_admin_create_sponsor_package(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;

    let bytes = req.bytes().await?;
    let input: SponsorPackageCreate = serde_json::from_slice(&bytes).map_err(AppError::from)?;
    validate_package_create(event_slug, &input)?;
    let name = input.name.trim();
    let advantage = input.advantage.trim();
    let group_id = input.group_id.trim();

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SponsorPackageRepository::new(db);

    let (package_id, groups, packages) = repo
        .create_package(event_slug, name, advantage, group_id, input.price_idr)
        .await
        .map_err(|e| match e {
            CreatePackageError::DuplicateName => AppError::Conflict(format!(
                "sponsor package name '{name}' already exists for event {event_slug}"
            )),
            CreatePackageError::UnknownGroupId(gid) => AppError::BadRequest(format!(
                "unknown sponsor package group id '{gid}' for event {event_slug}"
            )),
            CreatePackageError::PackageLimit => AppError::BadRequest(format!(
                "event {event_slug} already has the maximum of {MAX_PACKAGES_PER_EVENT} sponsor packages"
            )),
            CreatePackageError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    // Safe log: ids only, never free-text name/advantage.
    console_log!(
        "sponsor package created: event={event_slug} package_id={package_id} group_id={group_id}"
    );

    let body = SponsorPackagesResponse {
        event_slug,
        currency: "IDR",
        groups: &groups,
        packages: &packages,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// DELETE /api/admin/events/:eventSlug/sponsor-packages/:packageId — remove
/// one package. Returns the refreshed canonical listing so the admin can
/// replace its state.
pub async fn handle_admin_delete_sponsor_package(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    let package_id = ctx
        .param("packageId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: packageId".to_string()))?;
    validate_delete(event_slug, package_id, "packageId")?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SponsorPackageRepository::new(db);

    let (groups, packages) = repo
        .delete_package(event_slug, package_id.trim())
        .await
        .map_err(|e| match e {
            DeletePackageError::NotFound => AppError::NotFound(format!(
                "sponsor package '{package_id}' not found for event {event_slug}"
            )),
            DeletePackageError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    // Safe log: ids only.
    console_log!("sponsor package deleted: event={event_slug} package_id={package_id}");

    let body = SponsorPackagesResponse {
        event_slug,
        currency: "IDR",
        groups: &groups,
        packages: &packages,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// DELETE /api/admin/events/:eventSlug/sponsor-groups/:groupId — remove a
/// group only while no package references it (409 otherwise, so packages are
/// never orphaned). Returns the refreshed canonical listing so the admin can
/// replace its state.
pub async fn handle_admin_delete_sponsor_group(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    let group_id = ctx
        .param("groupId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: groupId".to_string()))?;
    validate_delete(event_slug, group_id, "groupId")?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SponsorPackageRepository::new(db);

    let (groups, packages) = repo
        .delete_group(event_slug, group_id.trim())
        .await
        .map_err(|e| match e {
            DeleteGroupError::NotFound => AppError::NotFound(format!(
                "sponsor group '{group_id}' not found for event {event_slug}"
            )),
            DeleteGroupError::GroupInUse => AppError::Conflict(
                "sponsor group still has packages assigned; reassign or remove them first"
                    .to_string(),
            ),
            DeleteGroupError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    // Safe log: ids only.
    console_log!("sponsor group deleted: event={event_slug} group_id={group_id}");

    let body = SponsorPackagesResponse {
        event_slug,
        currency: "IDR",
        groups: &groups,
        packages: &packages,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

// --- validation helpers ---

fn validate_event_slug(slug: &str) -> Result<(), AppError> {
    if slug.is_empty() || slug.len() > MAX_EVENT_SLUG_LEN {
        return Err(AppError::BadRequest(
            "eventSlug must be 1..=100 chars".to_string(),
        ));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::BadRequest(
            "eventSlug must contain only lowercase letters, digits, and '-'".to_string(),
        ));
    }
    Ok(())
}

fn validate_package_create(event_slug: &str, input: &SponsorPackageCreate) -> Result<(), AppError> {
    validate_event_slug(event_slug)?;
    let name = input.name.trim();
    if name.is_empty() || name.len() > MAX_PACKAGE_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "name must be 1..={MAX_PACKAGE_NAME_LEN} chars after trim"
        )));
    }
    let advantage = input.advantage.trim();
    if advantage.is_empty() || advantage.len() > MAX_PACKAGE_ADVANTAGE_LEN {
        return Err(AppError::BadRequest(format!(
            "advantage must be 1..={MAX_PACKAGE_ADVANTAGE_LEN} chars after trim"
        )));
    }
    if input.group_id.trim().is_empty() {
        return Err(AppError::BadRequest(
            "groupId must not be empty".to_string(),
        ));
    }
    if !(MIN_PRICE_IDR..=MAX_PRICE_IDR).contains(&input.price_idr) {
        return Err(AppError::BadRequest(format!(
            "priceIdr must be {MIN_PRICE_IDR}..={MAX_PRICE_IDR}"
        )));
    }
    Ok(())
}

fn validate_group_create(
    event_slug: &str,
    input: &SponsorPackageGroupCreate,
) -> Result<(), AppError> {
    validate_event_slug(event_slug)?;
    let label = input.label.trim();
    if label.is_empty() || label.len() > MAX_GROUP_LABEL_LEN {
        return Err(AppError::BadRequest(format!(
            "label must be 1..={MAX_GROUP_LABEL_LEN} chars after trim"
        )));
    }
    Ok(())
}

/// Shared path validation for the DELETE endpoints: event slug shape plus a
/// non-empty entity id after trim.
fn validate_delete(event_slug: &str, entity_id: &str, field: &str) -> Result<(), AppError> {
    validate_event_slug(event_slug)?;
    if entity_id.trim().is_empty() {
        return Err(AppError::BadRequest(format!("{field} must not be empty")));
    }
    Ok(())
}

fn validate_batch(event_slug: &str, input: &SponsorPackageBatchUpdate) -> Result<(), AppError> {
    validate_event_slug(event_slug)?;
    if input.packages.is_empty() && input.groups.is_empty() {
        return Err(AppError::BadRequest(
            "packages and groups must not both be empty".to_string(),
        ));
    }
    if input.packages.len() > MAX_PACKAGES_PER_UPDATE {
        return Err(AppError::BadRequest(format!(
            "packages count must be <= {MAX_PACKAGES_PER_UPDATE}"
        )));
    }
    if input.groups.len() > MAX_GROUPS_PER_UPDATE {
        return Err(AppError::BadRequest(format!(
            "groups count must be <= {MAX_GROUPS_PER_UPDATE}"
        )));
    }

    let mut group_ids: HashSet<&str> = HashSet::with_capacity(input.groups.len());
    let mut group_orders: HashSet<i32> = HashSet::with_capacity(input.groups.len());
    for g in &input.groups {
        let id = g.id.trim();
        if id.is_empty() {
            return Err(AppError::BadRequest(
                "group id must not be empty".to_string(),
            ));
        }
        if !group_ids.insert(id) {
            return Err(AppError::BadRequest(format!("duplicate group id: {id}")));
        }
        let label = g.label.trim();
        if label.is_empty() || label.len() > MAX_GROUP_LABEL_LEN {
            return Err(AppError::BadRequest(format!(
                "group label for {id} must be 1..={MAX_GROUP_LABEL_LEN} chars after trim"
            )));
        }
        if !(MIN_GROUP_ORDER..=MAX_GROUP_ORDER).contains(&g.display_order) {
            return Err(AppError::BadRequest(format!(
                "group displayOrder for {id} must be {MIN_GROUP_ORDER}..={MAX_GROUP_ORDER}"
            )));
        }
        if !group_orders.insert(g.display_order) {
            return Err(AppError::BadRequest(format!(
                "duplicate group displayOrder: {}",
                g.display_order
            )));
        }
    }

    let mut seen: HashSet<&str> = HashSet::with_capacity(input.packages.len());
    for p in &input.packages {
        let id = p.id.trim();
        if id.is_empty() {
            return Err(AppError::BadRequest(
                "package id must not be empty".to_string(),
            ));
        }
        if !seen.insert(p.id.as_str()) {
            return Err(AppError::BadRequest(format!("duplicate package id: {id}")));
        }
        if !(MIN_PRICE_IDR..=MAX_PRICE_IDR).contains(&p.price_idr) {
            return Err(AppError::BadRequest(format!(
                "priceIdr for {id} must be {MIN_PRICE_IDR}..={MAX_PRICE_IDR}"
            )));
        }
        if let Some(minimum_spend_idr) = p.minimum_spend_idr {
            if !(MIN_PRICE_IDR..=MAX_PRICE_IDR).contains(&minimum_spend_idr) {
                return Err(AppError::BadRequest(format!(
                    "minimumSpendIdr for {id} must be null or {MIN_PRICE_IDR}..={MAX_PRICE_IDR}"
                )));
            }
        }
        if !(0..=MAX_SPONSORS).contains(&p.reserved_sponsors) {
            return Err(AppError::BadRequest(format!(
                "reservedSponsors for {id} must be 0..={MAX_SPONSORS}"
            )));
        }
        if let Some(max_sponsors) = p.max_sponsors {
            if !(1..=MAX_SPONSORS).contains(&max_sponsors) {
                return Err(AppError::BadRequest(format!(
                    "maxSponsors for {id} must be null or 1..={MAX_SPONSORS}"
                )));
            }
            if p.reserved_sponsors > max_sponsors {
                return Err(AppError::BadRequest(format!(
                    "reservedSponsors for {id} must be <= maxSponsors {max_sponsors}"
                )));
            }
        }
        if let Some(group_id) = p.group_id.as_deref() {
            if group_id.trim().is_empty() {
                return Err(AppError::BadRequest(format!(
                    "groupId for {id} must not be empty"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sponsors::types::{SponsorPackageGroupUpdate, SponsorPackageUpdate};

    fn create(label: &str) -> SponsorPackageGroupCreate {
        SponsorPackageGroupCreate {
            label: label.to_string(),
        }
    }

    fn package_create(
        name: &str,
        advantage: &str,
        group_id: &str,
        price_idr: i64,
    ) -> SponsorPackageCreate {
        SponsorPackageCreate {
            name: name.to_string(),
            advantage: advantage.to_string(),
            group_id: group_id.to_string(),
            price_idr,
        }
    }

    #[test]
    fn package_create_field_boundaries() {
        let ok = package_create("Lanyard", "Lanyard branding", "onsite-physical", 7_500_000);
        assert!(validate_package_create("community-day-2026", &ok).is_ok());
        // Whitespace-only trims to empty.
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("   ", "Adv", "onsite-physical", 1)
        )
        .is_err());
        // Name 1..=80 after trim.
        assert!(validate_package_create(
            "community-day-2026",
            &package_create(&"x".repeat(80), "Adv", "onsite-physical", 1)
        )
        .is_ok());
        assert!(validate_package_create(
            "community-day-2026",
            &package_create(&"x".repeat(81), "Adv", "onsite-physical", 1)
        )
        .is_err());
        // Advantage 1..=500 after trim.
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", &"x".repeat(500), "onsite-physical", 1)
        )
        .is_ok());
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", &"x".repeat(501), "onsite-physical", 1)
        )
        .is_err());
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", "  ", "onsite-physical", 1)
        )
        .is_err());
    }

    #[test]
    fn package_create_group_and_price_bounds() {
        // groupId nonempty after trim.
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", "Adv", "", 1)
        )
        .is_err());
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", "Adv", "   ", 1)
        )
        .is_err());
        // priceIdr 1..=1_000_000_000.
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", "Adv", "onsite-physical", 0)
        )
        .is_err());
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", "Adv", "onsite-physical", 1)
        )
        .is_ok());
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", "Adv", "onsite-physical", 1_000_000_000)
        )
        .is_ok());
        assert!(validate_package_create(
            "community-day-2026",
            &package_create("Name", "Adv", "onsite-physical", 1_000_000_001)
        )
        .is_err());
    }

    #[test]
    fn package_create_rejects_invalid_event_slug() {
        let input = package_create("Lanyard", "Lanyard branding", "onsite-physical", 7_500_000);
        for slug in ["", "Community Day", "cdn/evil", "UPPER"] {
            assert!(
                validate_package_create(slug, &input).is_err(),
                "slug {slug:?} should be rejected"
            );
        }
    }

    #[test]
    fn group_create_label_boundaries() {
        assert!(validate_group_create("community-day-2026", &create("Digital & Media")).is_ok());
        // Whitespace-only trims to empty.
        assert!(validate_group_create("community-day-2026", &create("   ")).is_err());
        assert!(validate_group_create("community-day-2026", &create("")).is_err());
        // Exactly 80 chars (after trim) is allowed.
        let eighty = "x".repeat(80);
        assert!(validate_group_create("community-day-2026", &create(&eighty)).is_ok());
        // 81 chars rejected.
        let eighty_one = "x".repeat(81);
        assert!(validate_group_create("community-day-2026", &create(&eighty_one)).is_err());
        // Surrounding whitespace does not count toward the limit.
        let padded = format!("  {}  ", "x".repeat(80));
        assert!(validate_group_create("community-day-2026", &create(&padded)).is_ok());
    }

    #[test]
    fn group_create_rejects_invalid_event_slug() {
        assert!(validate_group_create("Community-Day-2026", &create("Digital & Media")).is_err());
        assert!(validate_group_create("community day 2026", &create("Digital & Media")).is_err());
        assert!(validate_group_create("", &create("Digital & Media")).is_err());
        assert!(validate_group_create("community_day_2026", &create("Digital & Media")).is_err());
    }

    #[test]
    fn delete_rejects_invalid_event_slug() {
        for slug in ["", "Community Day", "cdn/evil", "UPPER"] {
            assert!(
                validate_delete(slug, "lanyard-sponsorship-1-2", "packageId").is_err(),
                "slug {slug:?} should be rejected"
            );
        }
    }

    #[test]
    fn delete_requires_non_empty_entity_id() {
        // Empty and whitespace-only ids are rejected for both entity kinds.
        assert!(validate_delete("community-day-2026", "", "packageId").is_err());
        assert!(validate_delete("community-day-2026", "   ", "packageId").is_err());
        assert!(validate_delete("community-day-2026", "", "groupId").is_err());
        assert!(validate_delete("community-day-2026", " \t ", "groupId").is_err());
        // Valid slug + non-empty id passes; field name only shapes the message.
        assert!(validate_delete("community-day-2026", "digital-media-1-2", "groupId").is_ok());
    }

    #[test]
    fn delete_error_mentions_offending_field() {
        let err = validate_delete("community-day-2026", "  ", "packageId").unwrap_err();
        match err {
            AppError::BadRequest(msg) => {
                assert!(
                    msg.contains("packageId"),
                    "message should name the field: {msg}"
                );
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    fn update(
        id: &str,
        price_idr: i64,
        is_unlocked: bool,
        minimum_spend_idr: Option<i64>,
        max_sponsors: Option<i64>,
        reserved_sponsors: i64,
    ) -> SponsorPackageUpdate {
        SponsorPackageUpdate {
            id: id.to_string(),
            price_idr,
            minimum_spend_idr,
            max_sponsors,
            reserved_sponsors,
            is_unlocked,
            group_id: None,
        }
    }

    fn group(id: &str, label: &str, display_order: i32) -> SponsorPackageGroupUpdate {
        SponsorPackageGroupUpdate {
            id: id.to_string(),
            label: label.to_string(),
            display_order,
        }
    }

    fn batch(pkgs: Vec<SponsorPackageUpdate>) -> SponsorPackageBatchUpdate {
        SponsorPackageBatchUpdate {
            packages: pkgs,
            groups: Vec::new(),
        }
    }

    fn full_batch(
        pkgs: Vec<SponsorPackageUpdate>,
        groups: Vec<SponsorPackageGroupUpdate>,
    ) -> SponsorPackageBatchUpdate {
        SponsorPackageBatchUpdate {
            packages: pkgs,
            groups,
        }
    }

    #[test]
    fn accepts_valid_mixed_lock_states() {
        let input = full_batch(
            vec![
                update("web-logo", 3_000_000, true, None, None, 0),
                update("video-ad", 5_000_000, false, Some(10_000_000), None, 0),
            ],
            vec![],
        );
        assert!(validate_batch("community-day-2026", &input).is_ok());
    }

    #[test]
    fn rejects_empty_batch() {
        let input = batch(vec![]);
        let err = validate_batch("community-day-2026", &input).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.message().contains("both be empty"));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let input = batch(vec![
            update("web-logo", 3_000_000, true, None, None, 0),
            update("web-logo", 4_000_000, false, None, None, 0),
        ]);
        let err = validate_batch("community-day-2026", &input).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.message().contains("duplicate"));
    }

    #[test]
    fn rejects_empty_id() {
        let input = batch(vec![update("  ", 3_000_000, true, None, None, 0)]);
        let err = validate_batch("community-day-2026", &input).unwrap_err();
        assert!(err.message().contains("empty"));
    }

    #[test]
    fn rejects_prices_out_of_range() {
        for price in [0, -1, 1_000_000_001, i64::MAX] {
            let input = batch(vec![update("web-logo", price, true, None, None, 0)]);
            assert!(
                validate_batch("community-day-2026", &input).is_err(),
                "price {price}"
            );
        }
    }

    #[test]
    fn accepts_boundary_prices() {
        for price in [1, 1_000_000_000] {
            let input = batch(vec![update("web-logo", price, true, None, None, 0)]);
            assert!(
                validate_batch("community-day-2026", &input).is_ok(),
                "price {price}"
            );
        }
    }

    #[test]
    fn accepts_null_and_boundary_thresholds() {
        for threshold in [None, Some(1), Some(1_000_000_000)] {
            let input = batch(vec![update(
                "web-logo", 3_000_000, true, threshold, None, 0,
            )]);
            assert!(
                validate_batch("community-day-2026", &input).is_ok(),
                "threshold {threshold:?}"
            );
        }
    }

    #[test]
    fn rejects_thresholds_out_of_range() {
        for threshold in [
            Some(0),
            Some(-1),
            Some(1_000_000_001),
            Some(i64::MIN),
            Some(i64::MAX),
        ] {
            let input = batch(vec![update(
                "web-logo", 3_000_000, true, threshold, None, 0,
            )]);
            let err = validate_batch("community-day-2026", &input).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert!(
                err.message().contains("minimumSpendIdr"),
                "threshold {threshold:?}"
            );
        }
    }

    #[test]
    fn rejects_over_limit_count() {
        let input = batch(
            (0..51)
                .map(|i| update(&format!("pkg-{i}"), 1_000_000, true, None, None, 0))
                .collect(),
        );
        let err = validate_batch("community-day-2026", &input).unwrap_err();
        assert!(err.message().contains("<= 50"));
    }

    #[test]
    fn accepts_exactly_limit_count() {
        let input = batch(
            (0..50)
                .map(|i| update(&format!("pkg-{i}"), 1_000_000, true, None, None, 0))
                .collect(),
        );
        assert!(validate_batch("community-day-2026", &input).is_ok());
    }

    #[test]
    fn rejects_invalid_event_slug() {
        let input = batch(vec![update("web-logo", 3_000_000, true, None, None, 0)]);
        for slug in ["", "Community Day", "cdn/evil", "UPPER"] {
            assert!(validate_batch(slug, &input).is_err(), "slug '{slug}'");
        }
    }

    #[test]
    fn deserializes_camel_case_body() {
        let body = r#"{"packages":[{"id":"web-logo","priceIdr":3000000,"minimumSpendIdr":7500000,"maxSponsors":5,"reservedSponsors":2,"isUnlocked":true}],"groups":[]}"#;
        let input: SponsorPackageBatchUpdate = serde_json::from_str(body).unwrap();
        assert_eq!(input.packages.len(), 1);
        assert_eq!(input.packages[0].price_idr, 3_000_000);
        assert_eq!(input.packages[0].minimum_spend_idr, Some(7_500_000));
        assert_eq!(input.packages[0].max_sponsors, Some(5));
        assert_eq!(input.packages[0].reserved_sponsors, 2);
        assert!(input.packages[0].is_unlocked);
    }

    #[test]
    fn accepts_unlimited_capacity() {
        // null/missing cap with any reserved count within 0..=10000 is valid.
        for reserved in [0, 1, 10_000] {
            let input = batch(vec![update(
                "web-logo", 3_000_000, true, None, None, reserved,
            )]);
            assert!(
                validate_batch("community-day-2026", &input).is_ok(),
                "reserved {reserved}"
            );
        }
    }

    #[test]
    fn accepts_max_five_reserved_boundaries() {
        for reserved in [0, 5] {
            let input = batch(vec![update(
                "lanyard",
                7_500_000,
                true,
                None,
                Some(5),
                reserved,
            )]);
            assert!(
                validate_batch("community-day-2026", &input).is_ok(),
                "reserved {reserved}"
            );
        }
    }

    #[test]
    fn rejects_zero_and_out_of_range_max() {
        for max in [0, -1, 10_001, i64::MAX] {
            let input = batch(vec![update(
                "web-logo",
                3_000_000,
                true,
                None,
                Some(max),
                0,
            )]);
            let err = validate_batch("community-day-2026", &input).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert!(err.message().contains("maxSponsors"), "max {max}");
        }
    }

    #[test]
    fn rejects_reserved_over_cap() {
        let input = batch(vec![update("lanyard", 7_500_000, true, None, Some(5), 6)]);
        let err = validate_batch("community-day-2026", &input).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.message().contains("reservedSponsors"));
    }

    #[test]
    fn rejects_negative_and_over_range_reserved() {
        for reserved in [-1, i64::MIN, 10_001, i64::MAX] {
            let input = batch(vec![update(
                "web-logo", 3_000_000, true, None, None, reserved,
            )]);
            let err = validate_batch("community-day-2026", &input).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert!(
                err.message().contains("reservedSponsors"),
                "reserved {reserved}"
            );
        }
    }

    #[test]
    fn rejects_float_capacity_in_body() {
        for body in [
            r#"{"packages":[{"id":"web-logo","priceIdr":3000000,"maxSponsors":5.5,"reservedSponsors":2,"isUnlocked":true}],"groups":[]}"#,
            r#"{"packages":[{"id":"web-logo","priceIdr":3000000,"maxSponsors":5,"reservedSponsors":2.5,"isUnlocked":true}],"groups":[]}"#,
        ] {
            assert!(serde_json::from_str::<SponsorPackageBatchUpdate>(body).is_err());
        }
    }

    #[test]
    fn accepts_groups_only_batch() {
        let input = full_batch(
            vec![],
            vec![
                group("digital-media", "Digital & Media", 2),
                group("onsite-physical", "On-Site & Physical", 1),
            ],
        );
        assert!(validate_batch("community-day-2026", &input).is_ok());
    }

    #[test]
    fn accepts_mixed_packages_and_groups() {
        let mut pkg = update("web-logo", 3_000_000, true, None, None, 0);
        pkg.group_id = Some("digital-media".into());
        let input = full_batch(
            vec![pkg],
            vec![group("onsite-physical", "On-Site & Physical", 1)],
        );
        assert!(validate_batch("community-day-2026", &input).is_ok());
    }

    #[test]
    fn rejects_group_label_boundaries() {
        for label in [String::new(), "   ".to_string(), "x".repeat(81)] {
            let input = full_batch(vec![], vec![group("digital-media", &label, 1)]);
            let err = validate_batch("community-day-2026", &input).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert!(err.message().contains("label"), "label {label:?}");
        }
        // 80 chars and whitespace-padded single char both pass (trim applied).
        for label in ["x".repeat(80), "  x  ".to_string()] {
            let input = full_batch(vec![], vec![group("digital-media", &label, 1)]);
            assert!(
                validate_batch("community-day-2026", &input).is_ok(),
                "label len {}",
                label.len()
            );
        }
    }

    #[test]
    fn rejects_group_order_out_of_range() {
        for order in [0, -1, 1_001, i32::MAX] {
            let input = full_batch(vec![], vec![group("digital-media", "Digital", order)]);
            let err = validate_batch("community-day-2026", &input).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert!(err.message().contains("displayOrder"), "order {order}");
        }
        for order in [1, 1_000] {
            let input = full_batch(vec![], vec![group("digital-media", "Digital", order)]);
            assert!(
                validate_batch("community-day-2026", &input).is_ok(),
                "order {order}"
            );
        }
    }

    #[test]
    fn rejects_duplicate_group_ids_and_orders() {
        let dup_ids = full_batch(
            vec![],
            vec![
                group("digital-media", "A", 1),
                group("digital-media", "B", 2),
            ],
        );
        let err = validate_batch("community-day-2026", &dup_ids).unwrap_err();
        assert!(err.message().contains("duplicate group id"));

        let dup_orders = full_batch(
            vec![],
            vec![
                group("digital-media", "A", 1),
                group("onsite-physical", "B", 1),
            ],
        );
        let err = validate_batch("community-day-2026", &dup_orders).unwrap_err();
        assert!(err.message().contains("duplicate group displayOrder"));
    }

    #[test]
    fn rejects_over_limit_groups() {
        let groups = (0..21)
            .map(|i| group(&format!("g-{i}"), "Label", i + 1))
            .collect();
        let input = full_batch(vec![], groups);
        let err = validate_batch("community-day-2026", &input).unwrap_err();
        assert!(err.message().contains("<= 20"));

        let groups = (0..20)
            .map(|i| group(&format!("g-{i}"), "Label", i + 1))
            .collect();
        let input = full_batch(vec![], groups);
        assert!(validate_batch("community-day-2026", &input).is_ok());
    }

    #[test]
    fn rejects_whitespace_group_id_and_empty_group_ref() {
        let bad_group = full_batch(vec![], vec![group("  ", "Label", 1)]);
        assert!(validate_batch("community-day-2026", &bad_group).is_err());

        let mut pkg = update("web-logo", 3_000_000, true, None, None, 0);
        pkg.group_id = Some("   ".into());
        let input = full_batch(vec![pkg], vec![]);
        let err = validate_batch("community-day-2026", &input).unwrap_err();
        assert!(err.message().contains("groupId"));
    }

    #[test]
    fn deserializes_combined_body() {
        let body = r#"{
            "packages":[{"id":"tshirt","priceIdr":6000000,"reservedSponsors":2,"isUnlocked":true,"groupId":"onsite-physical"}],
            "groups":[{"id":"onsite-physical","label":"On-Site & Physical","displayOrder":1}]
        }"#;
        let input: SponsorPackageBatchUpdate = serde_json::from_str(body).unwrap();
        assert_eq!(input.packages.len(), 1);
        assert_eq!(
            input.packages[0].group_id.as_deref(),
            Some("onsite-physical")
        );
        assert_eq!(input.groups.len(), 1);
        assert_eq!(input.groups[0].label, "On-Site & Physical");
        assert_eq!(input.groups[0].display_order, 1);
        assert!(validate_batch("community-day-2026", &input).is_ok());
    }
}
