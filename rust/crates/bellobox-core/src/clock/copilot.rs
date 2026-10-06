//! Pure suggestion planning, ported from `WorldClockAIResolver.swift`'s
//! `WorldClockCopilotSuggestion.plan` and `parts`. This module does not parse
//! provider responses, apply plans, persist changes, or contact a provider.
//!
//! Zone validation uses the existing `chrono-tz` catalog, not Foundation's
//! `TimeZone(identifier:)`. It accepts exact catalog identifiers and aliases,
//! preserves their spelling, and rejects unknown IDs without trimming or
//! canonicalizing them. Foundation's OS-provided catalog and accepted names can
//! differ; this is bounded Rust IANA coverage, not Foundation catalog parity.
//! The current lockfile's `chrono-tz` 0.10.4 includes 597 identifiers from IANA
//! 2025b. Foundation also accepts offset names such as `GMT+8` and `GMT+05:30`,
//! which this catalog rejects; they are not converted to other identifiers.
//!
//! As in the source plan, current locations are taken as supplied and the
//! resulting list is not truncated. Parser/request limits and instant validation
//! belong at the input boundary. In particular, the Swift response parser's
//! 12-suggestion limit and the existing Rust planner's 24-location setter limit
//! are not limits imposed by this calculation.

use std::collections::HashSet;
use std::ops::{BitOr, BitOrAssign};

use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;

use super::zone_option;

/// Parts proposed or planned, so hosts can track time and locations separately.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Parts(u8);

impl Parts {
    pub const TIME: Self = Self(1);
    pub const LOCATIONS: Self = Self(2);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl BitOr for Parts {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl BitOrAssign for Parts {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

/// An already-parsed proposal. Constructing or planning one changes no planner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suggestion {
    pub instant: Option<DateTime<Utc>>,
    pub zone_ids: Vec<String>,
    pub replaces_locations: bool,
    pub anchor_zone_id: Option<String>,
}

/// The concrete changes available to a host under its current capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub instant: Option<DateTime<Utc>>,
    pub zone_ids: Option<Vec<String>>,
    pub anchor_zone_id: Option<String>,
    pub summary: String,
    pub parts: Parts,
}

impl Suggestion {
    pub fn is_empty(&self) -> bool {
        self.instant.is_none() && self.zone_ids.is_empty() && self.anchor_zone_id.is_none()
    }

    /// Proposed parts, even when they are disallowed, invalid, or already current.
    pub fn parts(&self) -> Parts {
        let mut parts = Parts::empty();
        if self.instant.is_some() {
            parts |= Parts::TIME;
        }
        if !self.zone_ids.is_empty() || self.anchor_zone_id.is_some() {
            parts |= Parts::LOCATIONS;
        }
        parts
    }

    /// Calculates only real changes. The host supplies the instant description
    /// just as in Swift; it is called once only when a time change is planned.
    /// Location changes, including reference-only changes, share one capability.
    pub fn plan(
        &self,
        current_zone_ids: &[String],
        current_anchor_zone_id: &str,
        current_instant: DateTime<Utc>,
        allows_time_changes: bool,
        allows_location_changes: bool,
        describe_instant: impl FnOnce(DateTime<Utc>) -> String,
    ) -> Option<Plan> {
        let mut plan = Plan {
            instant: None,
            zone_ids: None,
            anchor_zone_id: None,
            summary: String::new(),
            parts: Parts::empty(),
        };
        let mut summaries = Vec::new();

        if allows_time_changes
            && let Some(instant) = self.instant
            && (instant - current_instant).abs() >= Duration::seconds(60)
        {
            plan.instant = Some(instant);
            plan.parts |= Parts::TIME;
            summaries.push(format!("Set time to {}", describe_instant(instant)));
        }

        if allows_location_changes {
            let valid = valid_identifiers(&self.zone_ids);
            let replacing = self.replaces_locations && !valid.is_empty();
            let added: Vec<_> = valid
                .iter()
                .filter(|id| !current_zone_ids.contains(id))
                .cloned()
                .collect();
            let resulting = if replacing {
                valid.clone()
            } else {
                let mut resulting = current_zone_ids.to_vec();
                resulting.extend_from_slice(&added);
                resulting
            };
            if resulting != current_zone_ids {
                plan.zone_ids = Some(resulting.clone());
                summaries.push(if replacing {
                    format!("Replace locations with {}", names(&valid))
                } else {
                    format!("Add {}", names(&added))
                });
            }

            if let Some(anchor) = &self.anchor_zone_id
                && resulting.contains(anchor)
                && anchor != current_anchor_zone_id
            {
                plan.anchor_zone_id = Some(anchor.clone());
            } else if plan.zone_ids.is_some()
                && !resulting.iter().any(|id| id == current_anchor_zone_id)
            {
                plan.anchor_zone_id = resulting.first().cloned();
            }
            if let Some(anchor) = &plan.anchor_zone_id {
                summaries.push(format!("Reference: {}", zone_name(anchor)));
            }
        }

        if summaries.is_empty() {
            return None;
        }
        if plan.zone_ids.is_some() || plan.anchor_zone_id.is_some() {
            plan.parts |= Parts::LOCATIONS;
        }
        plan.summary = summaries.join(" · ");
        Some(plan)
    }
}

/// Stable, exact-string deduplication of IDs accepted by the Rust IANA catalog.
/// This intentionally does not use `normalized_zones`, which inserts defaults.
pub fn valid_identifiers(identifiers: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    identifiers
        .iter()
        .filter(|id| id.parse::<Tz>().is_ok() && seen.insert(id.as_str()))
        .cloned()
        .collect()
}

fn names(ids: &[String]) -> String {
    ids.iter()
        .map(|id| zone_name(id))
        .collect::<Vec<_>>()
        .join(", ")
}

fn zone_name(id: &str) -> String {
    if id.ends_with('/') {
        // Current IDs are preserved literally, including malformed anchors.
        // Swift's split omits empty components; preferred names still match
        // the original ID, so "Asia/Kolkata/" falls back to "Kolkata", not "India".
        id.rsplit('/')
            .find(|component| !component.is_empty())
            .unwrap_or(id)
            .replace('_', " ")
    } else {
        zone_option(id).name
    }
}

#[cfg(test)]
mod tests;
