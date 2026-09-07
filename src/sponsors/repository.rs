use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::d1::D1Result;
use worker::{D1Database, Result as WorkerResult};

use crate::sponsors::types::{
    SponsorPackage, SponsorPackageGroup, SponsorPackageGroupUpdate, SponsorPackageUpdate,
    SponsorTier, SponsorTierUpdate,
};

/// Hard cap on packages per event, enforced on create.
pub(crate) const MAX_PACKAGES_PER_EVENT: usize = 200;

/// Hard cap on sponsor tiers per event, enforced on create and batch update.
pub(crate) const MAX_TIERS_PER_EVENT: usize = 10;

/// Seeded group whose packages must keep the legacy `onsite` category.
pub(crate) const ONSITE_GROUP_ID: &str = "onsite-physical";

/// Failure modes of a group create. `DuplicateLabel` is detected before any
/// statement runs, so no row is mutated.
#[derive(Debug)]
pub enum CreateGroupError {
    DuplicateLabel,
    Db(worker::Error),
}

impl From<worker::Error> for CreateGroupError {
    fn from(err: worker::Error) -> Self {
        CreateGroupError::Db(err)
    }
}

/// Failure modes of a package create. `DuplicateName`, `UnknownGroupId`, and
/// `PackageLimit` are detected before any statement runs, so no row is mutated.
#[derive(Debug)]
pub enum CreatePackageError {
    DuplicateName,
    UnknownGroupId(String),
    PackageLimit,
    Db(worker::Error),
}

impl From<worker::Error> for CreatePackageError {
    fn from(err: worker::Error) -> Self {
        CreatePackageError::Db(err)
    }
}

/// Lowercase-ASCII slug for a label: `[a-z0-9-]`, separators collapsed,
/// trimmed; falls back to `fallback` when nothing alphanumeric survives.
pub(crate) fn slugify(label: &str, fallback: &str) -> String {
    let mut slug = String::new();
    let mut sep = false;
    for ch in label.chars() {
        let lc = ch.to_ascii_lowercase();
        if lc.is_ascii_lowercase() || lc.is_ascii_digit() {
            slug.push(lc);
            sep = false;
        } else if !sep && !slug.is_empty() {
            slug.push('-');
            sep = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str(fallback);
    }
    slug
}

/// Assemble an entity id from a slug plus timestamp/random entropy. Pure so
/// the shape is unit-testable; the caller supplies live `now_ms`/`rand`.
fn entity_id_from(slug: &str, now_ms: u64, rand: u64) -> String {
    format!("{slug}-{:x}-{:x}", now_ms, rand & 0xffff)
}

/// Slug a display label, then append timestamp/random entropy until the id
/// is absent per `is_taken`. Collision with an existing id is negligible;
/// retry a few times, then surface an error rather than risk a PK violation.
fn generate_unique_id(
    label: &str,
    fallback: &str,
    is_taken: impl Fn(&str) -> bool,
) -> Result<String, worker::Error> {
    let slug = slugify(label, fallback);
    for _ in 0..5 {
        let candidate = entity_id_from(
            &slug,
            js_sys::Date::now() as u64,
            (js_sys::Math::random() * 65535.0) as u64,
        );
        if !is_taken(&candidate) {
            return Ok(candidate);
        }
    }
    Err(worker::Error::RustError(format!(
        "could not generate an unused sponsor {fallback} id"
    )))
}

/// Failure modes of a package delete. `NotFound` means no row matched the
/// (event_slug, id) pair, so nothing was mutated.
#[derive(Debug)]
pub enum DeletePackageError {
    NotFound,
    Db(worker::Error),
}

impl From<worker::Error> for DeletePackageError {
    fn from(err: worker::Error) -> Self {
        DeletePackageError::Db(err)
    }
}

/// Failure modes of a group delete. `GroupInUse` is decided by the DELETE
/// statement itself (NOT EXISTS guard), so a referenced group is never
/// removed and packages can never be orphaned.
#[derive(Debug)]
pub enum DeleteGroupError {
    NotFound,
    GroupInUse,
    Db(worker::Error),
}

impl From<worker::Error> for DeleteGroupError {
    fn from(err: worker::Error) -> Self {
        DeleteGroupError::Db(err)
    }
}

fn changed_rows(result: &D1Result) -> WorkerResult<usize> {
    Ok(result.meta()?.and_then(|m| m.changes).unwrap_or(0))
}

/// Whether the final per-event threshold map (current values with update
/// overrides applied) contains any duplicate. Pure so the swap/collision
/// logic is unit-testable without D1.
fn has_duplicate_final_threshold(existing: &[SponsorTier], updates: &[SponsorTierUpdate]) -> bool {
    let mut finals: HashMap<&str, i64> = existing
        .iter()
        .map(|t| (t.id.as_str(), t.threshold_idr))
        .collect();
    for u in updates {
        finals.insert(u.id.trim(), u.threshold_idr);
    }
    let mut seen = HashSet::with_capacity(finals.len());
    finals.values().any(|v| !seen.insert(*v))
}

/// Failure modes of a batch update. `UnknownIds`, `UnknownGroupIds`, and
/// `OrderConflict` are reported before any statement runs, so no row is
/// mutated.
#[derive(Debug)]
pub enum UpdatePackagesError {
    UnknownIds(Vec<String>),
    UnknownGroupIds(Vec<String>),
    OrderConflict,
    Db(worker::Error),
}

impl From<worker::Error> for UpdatePackagesError {
    fn from(err: worker::Error) -> Self {
        UpdatePackagesError::Db(err)
    }
}

/// Failure modes of a tier create. `DuplicateLabel`, `DuplicateThreshold`,
/// and `TierLimit` are detected before any statement runs, so no row is
/// mutated.
#[derive(Debug)]
pub enum CreateTierError {
    DuplicateLabel,
    DuplicateThreshold,
    TierLimit,
    Db(worker::Error),
}

impl From<worker::Error> for CreateTierError {
    fn from(err: worker::Error) -> Self {
        CreateTierError::Db(err)
    }
}

/// Failure modes of a tier batch update. `UnknownIds` and
/// `DuplicateThreshold` are reported before any statement runs, so no row
/// is mutated.
#[derive(Debug)]
pub enum UpdateTiersError {
    UnknownIds(Vec<String>),
    DuplicateThreshold,
    Db(worker::Error),
}

impl From<worker::Error> for UpdateTiersError {
    fn from(err: worker::Error) -> Self {
        UpdateTiersError::Db(err)
    }
}

/// Failure modes of a tier delete. `NotFound` means no row matched the
/// (event_slug, id) pair, so nothing was mutated. Tiers have no dependents,
/// so there is no in-use variant.
#[derive(Debug)]
pub enum DeleteTierError {
    NotFound,
    Db(worker::Error),
}

impl From<worker::Error> for DeleteTierError {
    fn from(err: worker::Error) -> Self {
        DeleteTierError::Db(err)
    }
}

pub struct SponsorPackageRepository {
    db: D1Database,
}

impl SponsorPackageRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// List all groups for an event ordered by display_order, id.
    pub async fn list_groups(&self, event_slug: &str) -> WorkerResult<Vec<SponsorPackageGroup>> {
        let sql = r#"
            SELECT id, event_slug, label, display_order, updated_at
            FROM sponsor_package_groups
            WHERE event_slug = ?
            ORDER BY display_order, id
        "#;
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        result.results::<SponsorPackageGroup>()
    }

    /// List all packages for an event (locked included) ordered by display_order, id.
    pub async fn list_packages(&self, event_slug: &str) -> WorkerResult<Vec<SponsorPackage>> {
        let sql = r#"
            SELECT id, event_slug, name, advantage, category, group_id, price_idr, minimum_spend_idr, max_sponsors, reserved_sponsors, is_unlocked, display_order, updated_at
            FROM sponsor_packages
            WHERE event_slug = ?
            ORDER BY display_order, id
        "#;
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        result.results::<SponsorPackage>()
    }

    /// Load the fixed set of package ids for an event.
    async fn package_ids(&self, event_slug: &str) -> WorkerResult<HashSet<String>> {
        let result = self
            .db
            .prepare("SELECT id FROM sponsor_packages WHERE event_slug = ?")
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        Ok(result
            .results::<IdRow>()?
            .into_iter()
            .map(|r| r.id)
            .collect())
    }

    /// Create a group for an event: server-generated id (slug + entropy,
    /// checked absent), display_order = max existing + 1. Returns the new id
    /// plus the refreshed groups/packages listing.
    pub async fn create_group(
        &self,
        event_slug: &str,
        label: &str,
    ) -> Result<(String, Vec<SponsorPackageGroup>, Vec<SponsorPackage>), CreateGroupError> {
        let existing = self.list_groups(event_slug).await?;
        if existing
            .iter()
            .any(|g| g.label.trim().eq_ignore_ascii_case(label))
        {
            return Err(CreateGroupError::DuplicateLabel);
        }
        let next_order = existing.iter().map(|g| g.display_order).max().unwrap_or(0) + 1;

        let id = generate_unique_id(label, "group", |candidate| {
            existing.iter().any(|g| g.id == candidate)
        })?;

        self.db
            .prepare(
                r#"
                INSERT INTO sponsor_package_groups
                    (id, event_slug, label, display_order, updated_at)
                VALUES (?, ?, ?, ?, datetime('now'))
            "#,
            )
            .bind(&[
                JsValue::from_str(&id),
                JsValue::from_str(event_slug),
                JsValue::from_str(label),
                JsValue::from_f64(next_order as f64),
            ])?
            .run()
            .await?;

        Ok((
            id,
            self.list_groups(event_slug).await?,
            self.list_packages(event_slug).await?,
        ))
    }

    /// Create a package for an event: server-generated id (slug + entropy,
    /// checked absent), display_order = max existing + 1, unlock/capacity
    /// defaults. The legacy `category` column is derived from the target
    /// group (`onsite` only for the seeded on-site group, else `digital`);
    /// it is compatibility-only and never drives grouping. Returns the new
    /// id plus the refreshed groups/packages listing.
    pub async fn create_package(
        &self,
        event_slug: &str,
        name: &str,
        advantage: &str,
        group_id: &str,
        price_idr: i64,
    ) -> Result<(String, Vec<SponsorPackageGroup>, Vec<SponsorPackage>), CreatePackageError> {
        let groups = self.list_groups(event_slug).await?;
        if !groups.iter().any(|g| g.id == group_id) {
            return Err(CreatePackageError::UnknownGroupId(group_id.to_string()));
        }
        let packages = self.list_packages(event_slug).await?;
        if packages
            .iter()
            .any(|p| p.name.trim().eq_ignore_ascii_case(name))
        {
            return Err(CreatePackageError::DuplicateName);
        }
        if packages.len() >= MAX_PACKAGES_PER_EVENT {
            return Err(CreatePackageError::PackageLimit);
        }
        let next_order = packages.iter().map(|p| p.display_order).max().unwrap_or(0) + 1;
        let category = if group_id == ONSITE_GROUP_ID {
            "onsite"
        } else {
            "digital"
        };
        let id = generate_unique_id(name, "package", |candidate| {
            packages.iter().any(|p| p.id == candidate)
        })?;

        self.db
            .prepare(
                r#"
                INSERT INTO sponsor_packages
                    (id, event_slug, name, advantage, category, group_id, price_idr,
                     minimum_spend_idr, max_sponsors, reserved_sponsors, is_unlocked,
                     display_order, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL, 0, 1, ?, datetime('now'))
            "#,
            )
            .bind(&[
                JsValue::from_str(&id),
                JsValue::from_str(event_slug),
                JsValue::from_str(name),
                JsValue::from_str(advantage),
                JsValue::from_str(category),
                JsValue::from_str(group_id),
                JsValue::from_f64(price_idr as f64),
                JsValue::from_f64(next_order as f64),
            ])?
            .run()
            .await?;

        Ok((
            id,
            self.list_groups(event_slug).await?,
            self.list_packages(event_slug).await?,
        ))
    }

    /// Validate requested group and package ids against the fixed event sets,
    /// then apply all updates in one D1 batch and return the refreshed listing.
    ///
    /// Membership may change concurrently via the create/delete endpoints, so
    /// unknown ids can still surface as a no-op UPDATE; the pre-checks keep
    /// that window negligible and the UNIQUE(event_slug, display_order)
    /// constraint backstops order collisions.
    ///
    /// Group display_order rewrites are two-phase: the UNIQUE(event_slug,
    /// display_order) constraint is immediate, so swaps would collide if done
    /// in one pass. Phase one moves every updated group to a distinct negative
    /// temporary order, phase two writes the final positive orders — all inside
    /// the same batch, so a failure leaves rows untouched.
    pub async fn update(
        &self,
        event_slug: &str,
        group_updates: &[SponsorPackageGroupUpdate],
        package_updates: &[SponsorPackageUpdate],
    ) -> Result<(Vec<SponsorPackageGroup>, Vec<SponsorPackage>), UpdatePackagesError> {
        let existing_groups = self.list_groups(event_slug).await?;

        let known_group_ids: HashSet<&str> =
            existing_groups.iter().map(|g| g.id.as_str()).collect();
        let unknown_groups: Vec<String> = group_updates
            .iter()
            .filter(|g| !known_group_ids.contains(g.id.trim()))
            .map(|g| g.id.trim().to_string())
            .collect();
        if !unknown_groups.is_empty() {
            return Err(UpdatePackagesError::UnknownGroupIds(unknown_groups));
        }

        // Package group references must point at existing groups.
        let bad_refs: Vec<String> = package_updates
            .iter()
            .filter_map(|p| p.group_id.as_deref())
            .filter(|gid| !known_group_ids.contains(gid.trim()))
            .map(|gid| gid.trim().to_string())
            .collect();
        if !bad_refs.is_empty() {
            return Err(UpdatePackagesError::UnknownGroupIds(bad_refs));
        }

        // Final orders (updates applied over current state) must stay unique
        // per event; checked here so a partial reorder targeting an order still
        // held by an untouched group fails before any mutation.
        let mut final_orders: HashMap<&str, i32> = existing_groups
            .iter()
            .map(|g| (g.id.as_str(), g.display_order))
            .collect();
        for g in group_updates {
            final_orders.insert(g.id.trim(), g.display_order);
        }
        let mut seen_orders: HashSet<i32> = HashSet::with_capacity(final_orders.len());
        if final_orders.values().any(|o| !seen_orders.insert(*o)) {
            return Err(UpdatePackagesError::OrderConflict);
        }

        let known = self.package_ids(event_slug).await?;
        let unknown: Vec<String> = package_updates
            .iter()
            .filter(|u| !known.contains(u.id.as_str()))
            .map(|u| u.id.clone())
            .collect();
        if !unknown.is_empty() {
            return Err(UpdatePackagesError::UnknownIds(unknown));
        }

        let group_order_sql =
            "UPDATE sponsor_package_groups SET display_order = ? WHERE event_slug = ? AND id = ?";
        let group_final_sql = r#"
            UPDATE sponsor_package_groups
            SET label = ?, display_order = ?, updated_at = datetime('now')
            WHERE event_slug = ? AND id = ?
        "#;
        let package_sql = r#"
            UPDATE sponsor_packages
            SET price_idr = ?,
                minimum_spend_idr = ?,
                max_sponsors = ?,
                reserved_sponsors = ?,
                is_unlocked = ?,
                group_id = ?,
                updated_at = datetime('now')
            WHERE event_slug = ? AND id = ?
        "#;

        let mut statements = Vec::with_capacity(group_updates.len() * 2 + package_updates.len());
        // Phase one: distinct negative temp orders (never collide with the
        // 1..=1000 app-validated range or with each other).
        for (i, g) in group_updates.iter().enumerate() {
            statements.push(self.db.prepare(group_order_sql).bind(&[
                JsValue::from_f64(-(i as f64) - 1.0),
                JsValue::from_str(event_slug),
                JsValue::from_str(g.id.trim()),
            ])?);
        }
        // Phase two: final positive orders and labels.
        for g in group_updates {
            statements.push(self.db.prepare(group_final_sql).bind(&[
                JsValue::from_str(g.label.trim()),
                JsValue::from_f64(g.display_order as f64),
                JsValue::from_str(event_slug),
                JsValue::from_str(g.id.trim()),
            ])?);
        }
        for u in package_updates {
            // NULL unbinds the threshold/cap/group; whole-IDR i64 fits losslessly in f64.
            let minimum_spend_idr = match u.minimum_spend_idr {
                Some(v) => JsValue::from_f64(v as f64),
                None => JsValue::NULL,
            };
            let max_sponsors = match u.max_sponsors {
                Some(v) => JsValue::from_f64(v as f64),
                None => JsValue::NULL,
            };
            let group_id = match u.group_id.as_deref() {
                Some(g) => JsValue::from_str(g.trim()),
                None => JsValue::NULL,
            };
            let stmt = self.db.prepare(package_sql).bind(&[
                JsValue::from_f64(u.price_idr as f64),
                minimum_spend_idr,
                max_sponsors,
                JsValue::from_f64(u.reserved_sponsors as f64),
                JsValue::from_bool(u.is_unlocked),
                group_id,
                JsValue::from_str(event_slug),
                JsValue::from_str(&u.id),
            ])?;
            statements.push(stmt);
        }
        self.db.batch(statements).await?;

        Ok((
            self.list_groups(event_slug).await?,
            self.list_packages(event_slug).await?,
        ))
    }

    /// Delete a package for an event. Verifies the affected-row count so a
    /// missing (or cross-event) id maps to `NotFound` instead of a silent
    /// success. Returns the refreshed groups/packages listing.
    pub async fn delete_package(
        &self,
        event_slug: &str,
        package_id: &str,
    ) -> Result<(Vec<SponsorPackageGroup>, Vec<SponsorPackage>), DeletePackageError> {
        let result = self
            .db
            .prepare("DELETE FROM sponsor_packages WHERE event_slug = ? AND id = ?")
            .bind(&[JsValue::from_str(event_slug), JsValue::from_str(package_id)])?
            .run()
            .await?;
        if changed_rows(&result)? == 0 {
            return Err(DeletePackageError::NotFound);
        }

        Ok((
            self.list_groups(event_slug).await?,
            self.list_packages(event_slug).await?,
        ))
    }

    /// Delete a group only while no package still references it. The NOT
    /// EXISTS guard lives inside the DELETE itself, so even a concurrent
    /// package create cannot race past the check and end up orphaned. A zero
    /// affected-row count is disambiguated afterwards: missing group vs group
    /// still in use.
    pub async fn delete_group(
        &self,
        event_slug: &str,
        group_id: &str,
    ) -> Result<(Vec<SponsorPackageGroup>, Vec<SponsorPackage>), DeleteGroupError> {
        let result = self
            .db
            .prepare(
                r#"
                DELETE FROM sponsor_package_groups
                WHERE event_slug = ? AND id = ?
                  AND NOT EXISTS (
                      SELECT 1 FROM sponsor_packages
                      WHERE event_slug = ? AND group_id = ?
                  )
            "#,
            )
            .bind(&[
                JsValue::from_str(event_slug),
                JsValue::from_str(group_id),
                JsValue::from_str(event_slug),
                JsValue::from_str(group_id),
            ])?
            .run()
            .await?;
        if changed_rows(&result)? == 0 {
            let exists = self
                .db
                .prepare("SELECT id FROM sponsor_package_groups WHERE event_slug = ? AND id = ?")
                .bind(&[JsValue::from_str(event_slug), JsValue::from_str(group_id)])?
                .first::<IdRow>(None)
                .await?
                .is_some();
            return Err(if exists {
                DeleteGroupError::GroupInUse
            } else {
                DeleteGroupError::NotFound
            });
        }

        Ok((
            self.list_groups(event_slug).await?,
            self.list_packages(event_slug).await?,
        ))
    }

    /// List all tiers for an event ordered by threshold_idr DESC (tier order
    /// IS threshold order; there is deliberately no display_order).
    pub async fn list_tiers(&self, event_slug: &str) -> WorkerResult<Vec<SponsorTier>> {
        let sql = r#"
            SELECT id, event_slug, label, threshold_idr, accent, updated_at
            FROM sponsor_tiers
            WHERE event_slug = ?
            ORDER BY threshold_idr DESC
        "#;
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        result.results::<SponsorTier>()
    }

    /// Create a tier for an event: server-generated id (slug + entropy,
    /// checked absent). Label (case-insensitive) and threshold must be unique
    /// within the event. Returns the new id plus the refreshed tier listing.
    pub async fn create_tier(
        &self,
        event_slug: &str,
        label: &str,
        threshold_idr: i64,
        accent: &str,
    ) -> Result<(String, Vec<SponsorTier>), CreateTierError> {
        let existing = self.list_tiers(event_slug).await?;
        if existing
            .iter()
            .any(|t| t.label.trim().eq_ignore_ascii_case(label))
        {
            return Err(CreateTierError::DuplicateLabel);
        }
        if existing.iter().any(|t| t.threshold_idr == threshold_idr) {
            return Err(CreateTierError::DuplicateThreshold);
        }
        if existing.len() >= MAX_TIERS_PER_EVENT {
            return Err(CreateTierError::TierLimit);
        }

        let id = generate_unique_id(label, "tier", |candidate| {
            existing.iter().any(|t| t.id == candidate)
        })?;

        self.db
            .prepare(
                r#"
                INSERT INTO sponsor_tiers
                    (id, event_slug, label, threshold_idr, accent, updated_at)
                VALUES (?, ?, ?, ?, ?, datetime('now'))
            "#,
            )
            .bind(&[
                JsValue::from_str(&id),
                JsValue::from_str(event_slug),
                JsValue::from_str(label),
                JsValue::from_f64(threshold_idr as f64),
                JsValue::from_str(accent),
            ])?
            .run()
            .await?;

        Ok((id, self.list_tiers(event_slug).await?))
    }

    /// Validate requested tier ids against the fixed event set and final
    /// thresholds for uniqueness across the whole event, then apply all
    /// updates in one D1 batch and return the refreshed tier listing.
    ///
    /// Membership may change concurrently via the create/delete endpoints,
    /// so unknown ids can still surface as a no-op UPDATE; the pre-checks
    /// keep that window negligible and the UNIQUE(event_slug, threshold_idr)
    /// constraint backstops collisions.
    ///
    /// Threshold rewrites are two-phase for the same reason as group
    /// display_order rewrites: the UNIQUE(event_slug, threshold_idr)
    /// constraint is immediate, so swapping thresholds between tiers would
    /// collide in one pass. Phase one moves every updated tier to a distinct
    /// negative temporary threshold, phase two writes the final positive
    /// values — all inside the same batch, so a failure leaves rows
    /// untouched. Negative temps are why sponsor_tiers has no CHECK on
    /// threshold_idr; the 1..=1_000_000_000 range is enforced at the
    /// application layer.
    pub async fn update_tiers(
        &self,
        event_slug: &str,
        tier_updates: &[SponsorTierUpdate],
    ) -> Result<Vec<SponsorTier>, UpdateTiersError> {
        let existing = self.list_tiers(event_slug).await?;

        let known_ids: HashSet<&str> = existing.iter().map(|t| t.id.as_str()).collect();
        let unknown: Vec<String> = tier_updates
            .iter()
            .filter(|t| !known_ids.contains(t.id.trim()))
            .map(|t| t.id.trim().to_string())
            .collect();
        if !unknown.is_empty() {
            return Err(UpdateTiersError::UnknownIds(unknown));
        }

        // Final thresholds (updates applied over current state) must stay
        // unique per event; checked before any mutation so a partial edit
        // targeting a threshold still held by an untouched tier fails clean.
        if has_duplicate_final_threshold(&existing, tier_updates) {
            return Err(UpdateTiersError::DuplicateThreshold);
        }

        let temp_sql = "UPDATE sponsor_tiers SET threshold_idr = ? WHERE event_slug = ? AND id = ?";
        let final_sql = r#"
            UPDATE sponsor_tiers
            SET label = ?, threshold_idr = ?, accent = ?, updated_at = datetime('now')
            WHERE event_slug = ? AND id = ?
        "#;

        let mut statements = Vec::with_capacity(tier_updates.len() * 2);
        // Phase one: distinct negative temp thresholds (never collide with
        // the 1..=1_000_000_000 app-validated range or with each other).
        for (i, t) in tier_updates.iter().enumerate() {
            statements.push(self.db.prepare(temp_sql).bind(&[
                JsValue::from_f64(-(i as f64) - 1.0),
                JsValue::from_str(event_slug),
                JsValue::from_str(t.id.trim()),
            ])?);
        }
        // Phase two: final positive thresholds, labels, and accents.
        for t in tier_updates {
            statements.push(self.db.prepare(final_sql).bind(&[
                JsValue::from_str(t.label.trim()),
                JsValue::from_f64(t.threshold_idr as f64),
                JsValue::from_str(&t.accent),
                JsValue::from_str(event_slug),
                JsValue::from_str(t.id.trim()),
            ])?);
        }
        self.db.batch(statements).await?;

        Ok(self.list_tiers(event_slug).await?)
    }

    /// Delete a tier for an event. Tiers have no dependents, so there is no
    /// in-use guard. Verifies the affected-row count so a missing (or
    /// cross-event) id maps to `NotFound` instead of a silent success.
    /// Returns the refreshed tier listing.
    pub async fn delete_tier(
        &self,
        event_slug: &str,
        tier_id: &str,
    ) -> Result<Vec<SponsorTier>, DeleteTierError> {
        let result = self
            .db
            .prepare("DELETE FROM sponsor_tiers WHERE event_slug = ? AND id = ?")
            .bind(&[JsValue::from_str(event_slug), JsValue::from_str(tier_id)])?
            .run()
            .await?;
        if changed_rows(&result)? == 0 {
            return Err(DeleteTierError::NotFound);
        }

        Ok(self.list_tiers(event_slug).await?)
    }
}

#[derive(Deserialize)]
struct IdRow {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_labels() {
        assert_eq!(slugify("Digital & Media", "group"), "digital-media");
        assert_eq!(
            slugify("  On-Site & Physical  ", "group"),
            "on-site-physical"
        );
        assert_eq!(slugify("---Main---///Partner---", "group"), "main-partner");
        assert_eq!(slugify("Hello World", "group"), "hello-world");
        assert_eq!(slugify("Café Bar!", "group"), "caf-bar");
        assert_eq!(slugify("42nd Tier", "group"), "42nd-tier");
        assert_eq!(slugify("GROUP", "group"), "group");
        // Package names reuse the same slugifier with their own fallback.
        assert_eq!(
            slugify("Lanyard Sponsorship", "package"),
            "lanyard-sponsorship"
        );
    }

    #[test]
    fn slugify_falls_back_when_no_alnum() {
        for label in ["", "   ", "---", "&&&", "✨🎉"] {
            assert_eq!(slugify(label, "group"), "group", "label {label:?}");
            assert_eq!(slugify(label, "package"), "package", "label {label:?}");
        }
    }

    #[test]
    fn entity_id_shape_and_entropy() {
        let a = entity_id_from("lanyard-sponsorship", 1_799_000_000_000, 0xdead);
        let b = entity_id_from("lanyard-sponsorship", 1_799_000_000_001, 0xbeef);
        assert!(a.starts_with("lanyard-sponsorship-"));
        assert!(a
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        assert_ne!(a, b);
        // Same entropy inputs yield a deterministic id (stable server shape).
        assert_eq!(
            a,
            entity_id_from("lanyard-sponsorship", 1_799_000_000_000, 0xdead)
        );
        // rand is masked to a short suffix.
        assert!(a.ends_with("-dead"));
    }

    fn tier(id: &str, threshold_idr: i64) -> SponsorTier {
        SponsorTier {
            id: id.to_string(),
            event_slug: "community-day-2026".to_string(),
            label: format!("Tier {id}"),
            threshold_idr,
            accent: "default".to_string(),
            updated_at: "2026-01-01 00:00:00".to_string(),
        }
    }

    fn tier_update(id: &str, threshold_idr: i64) -> SponsorTierUpdate {
        SponsorTierUpdate {
            id: id.to_string(),
            label: format!("Tier {id}"),
            threshold_idr,
            accent: "default".to_string(),
        }
    }

    #[test]
    fn final_threshold_swap_is_unique() {
        // Swapping the two thresholds is exactly the case the two-phase
        // rewrite exists for: final values stay unique, so it must pass.
        let existing = vec![tier("platinum", 40_000_000), tier("gold", 25_000_000)];
        let updates = vec![
            tier_update("platinum", 25_000_000),
            tier_update("gold", 40_000_000),
        ];
        assert!(!has_duplicate_final_threshold(&existing, &updates));
    }

    #[test]
    fn final_threshold_collision_is_detected() {
        let existing = vec![tier("platinum", 40_000_000), tier("gold", 25_000_000)];
        // Update targets a threshold still held by an untouched tier.
        let updates = vec![tier_update("platinum", 25_000_000)];
        assert!(has_duplicate_final_threshold(&existing, &updates));
        // Duplicate inside the update body itself.
        let dup_in_body = vec![
            tier_update("platinum", 10_000_000),
            tier_update("gold", 10_000_000),
        ];
        assert!(has_duplicate_final_threshold(&existing, &dup_in_body));
        // Distinct values stay clean, including overlapping an old value that
        // the same update vacates.
        let ok = vec![
            tier_update("platinum", 25_000_000),
            tier_update("gold", 10_000_000),
        ];
        assert!(!has_duplicate_final_threshold(&existing, &ok));
        // Untouched duplicate-free state stays clean.
        assert!(!has_duplicate_final_threshold(&existing, &[]));
    }
}
