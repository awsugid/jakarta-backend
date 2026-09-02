use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::{D1Database, Result as WorkerResult};

use crate::sponsors::types::{
    SponsorPackage, SponsorPackageGroup, SponsorPackageGroupUpdate, SponsorPackageUpdate,
};

/// Hard cap on packages per event, enforced on create.
pub(crate) const MAX_PACKAGES_PER_EVENT: usize = 200;

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
    /// Membership cannot change through the application between this check and
    /// the batch (no create/delete endpoint), so the check is sufficient to
    /// reject unknown ids without partial updates.
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
}
