//! # madar-plan
//!
//! What a plan allows, defined once (FREEMIUM_SPEC_2026-10-11.md §3). The
//! backend, the Rust cores (POS, dashboard app, KDS) and the web dashboard all
//! ask this crate; none keeps its own list.
//!
//! - [`Plan`]: `free` or `paid`. An unknown value is Paid, so an old server
//!   never locks a paying shop out.
//! - [`entitlements`]: the plan's [`Feature`]s, [`PlanLimits`] and whether its
//!   tills hold a realtime stream.
//! - [`ceiling`] / [`apply`]: the capabilities a plan can grant at all, and a
//!   person's effective set cut to them. Core capabilities always survive:
//!   `authz-gen` refuses a plan table that puts one behind a feature.
//! - [`may_add_branch`], [`may_activate`]: the limits, as refusals the apps
//!   can show in English and Arabic ([`Refusal`]).
//! - [`downgrade_keeps`]: which till each branch keeps selling on after a
//!   paid organisation drops to Free.
//! - [`realtime_topics`]: the topics a role subscribes to; none on Free.
//!
//! The tables ([`FEATURES`], [`PLANS`]) are generated from
//! `authz/spec/plans.toml` by `authz-gen`, which also checks them against
//! `authz/spec/capabilities.toml`. Nothing here does I/O. Pinned by
//! `vectors/plan_vectors.json`.

#[rustfmt::skip]
pub mod generated;
mod downgrade;
mod refusal;
mod topics;
pub mod vectors;

#[cfg(test)]
mod tests;

use madar_authz::{Cap, CapSet, EffectiveSet};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use downgrade::{downgrade_keeps, Keep, Pick, TillUse};
pub use generated::{Feature, FEATURES, FEATURE_COUNT, PLANS, SPEC_HASH, SPEC_VERSION};
pub use refusal::{Code, Refusal};
pub use topics::{realtime_topics, Topic};

// ── Plan ────────────────────────────────────────────────────────────────────

/// A plan. Stored on the organisation as `'free'` or `'paid'`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Plan {
    Free,
    Paid,
}

impl Default for Plan {
    /// Paid: the plan of every organisation that exists today.
    fn default() -> Plan {
        Plan::Paid
    }
}

impl Plan {
    pub fn as_str(self) -> &'static str {
        match self {
            Plan::Free => "free",
            Plan::Paid => "paid",
        }
    }

    /// `"free"` is Free; anything else (`"paid"`, a plan a newer server has
    /// invented, an empty string, other casing) is Paid, so an old build never
    /// locks a paying shop out.
    pub fn parse(s: &str) -> Plan {
        if s == "free" {
            Plan::Free
        } else {
            Plan::Paid
        }
    }

    fn meta(self) -> &'static PlanMeta {
        PLANS
            .iter()
            .find(|p| p.key == self.as_str())
            .expect("generated PLANS has every plan")
    }
}

impl Serialize for Plan {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Plan {
    /// Unknown strings and `null` are Paid (see [`Plan::parse`]).
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Plan, D::Error> {
        Ok(Option::<String>::deserialize(d)?
            .as_deref()
            .map_or(Plan::Paid, Plan::parse))
    }
}

// ── Features and limits ─────────────────────────────────────────────────────

/// A feature's generated row: the capabilities it switches.
#[derive(Copy, Clone, Debug)]
pub struct FeatureMeta {
    pub feature: Feature,
    pub key: &'static str,
    pub caps: &'static [Cap],
}

/// A plan's generated row.
#[derive(Copy, Clone, Debug)]
pub struct PlanMeta {
    pub key: &'static str,
    pub features: &'static [Feature],
    pub limits: PlanLimits,
    pub realtime: bool,
}

impl Feature {
    pub fn as_str(self) -> &'static str {
        self.meta().key
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Feature> {
        FEATURES.iter().find(|m| m.key == s).map(|m| m.feature)
    }

    pub fn all() -> impl Iterator<Item = Feature> {
        FEATURES.iter().map(|m| m.feature)
    }

    fn meta(self) -> &'static FeatureMeta {
        &FEATURES[self as usize]
    }

    /// The capabilities this feature switches on. Never a core capability.
    pub fn caps(self) -> CapSet {
        self.meta().caps.iter().copied().collect()
    }

    /// The feature that gates `cap`, if any. A capability behind no feature is
    /// free (or core), on every plan.
    #[must_use]
    pub fn of_cap(cap: Cap) -> Option<Feature> {
        FEATURES
            .iter()
            .find(|m| m.caps.contains(&cap))
            .map(|m| m.feature)
    }
}

/// A bitset of [`Feature`]s. Crosses the wire as a list of feature keys.
#[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
pub struct FeatureSet(u16);

const _: () = assert!(FEATURE_COUNT <= 16);

impl FeatureSet {
    pub const EMPTY: FeatureSet = FeatureSet(0);

    pub fn contains(self, f: Feature) -> bool {
        self.0 & (1 << f as u16) != 0
    }

    pub fn insert(&mut self, f: Feature) {
        self.0 |= 1 << f as u16;
    }

    pub fn iter(self) -> impl Iterator<Item = Feature> {
        Feature::all().filter(move |f| self.contains(*f))
    }

    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn union(self, other: FeatureSet) -> FeatureSet {
        FeatureSet(self.0 | other.0)
    }
}

impl std::fmt::Debug for FeatureSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.iter().map(Feature::as_str))
            .finish()
    }
}

impl FromIterator<Feature> for FeatureSet {
    fn from_iter<I: IntoIterator<Item = Feature>>(iter: I) -> Self {
        let mut s = FeatureSet::EMPTY;
        for f in iter {
            s.insert(f);
        }
        s
    }
}

impl Serialize for FeatureSet {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for FeatureSet {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<FeatureSet, D::Error> {
        Ok(Vec::<Feature>::deserialize(d)?.into_iter().collect())
    }
}

/// A plan's limits. `None` is unlimited. Devices are counted per branch.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct PlanLimits {
    /// Live branches the organisation may have.
    pub branches: Option<u32>,
    /// Active POS devices (tills) per branch.
    pub pos_devices_per_branch: Option<u32>,
    /// Active kitchen screens per branch.
    pub kds_devices: Option<u32>,
    /// Active waiter devices per branch.
    pub waiter_devices: Option<u32>,
}

/// What a plan switches on.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct Entitlements {
    #[cfg_attr(feature = "tsify", tsify(type = "Feature[]"))]
    pub features: FeatureSet,
    pub limits: PlanLimits,
    /// May a till hold the realtime stream. Free tills poll instead.
    pub realtime: bool,
}

impl Entitlements {
    pub fn has(&self, f: Feature) -> bool {
        self.features.contains(f)
    }

    /// `Ok` when the plan has `f`, else the refusal to show.
    pub fn require(&self, f: Feature) -> Result<(), Refusal> {
        if self.has(f) {
            Ok(())
        } else {
            Err(Refusal::feature(f))
        }
    }
}

pub fn entitlements(plan: Plan) -> Entitlements {
    entitlements_with(plan, FeatureSet::EMPTY)
}

/// The plan's entitlements plus the organisation's add-ons (Wafa: loyalty
/// sold alone on Free). An add-on adds features only; limits and realtime stay
/// the plan's.
pub fn entitlements_with(plan: Plan, addons: FeatureSet) -> Entitlements {
    let m = plan.meta();
    Entitlements {
        features: m
            .features
            .iter()
            .copied()
            .collect::<FeatureSet>()
            .union(addons),
        limits: m.limits,
        realtime: m.realtime,
    }
}

/// `plan.has(feature)` without building the whole table.
pub fn has_feature(plan: Plan, f: Feature) -> bool {
    plan.meta().features.contains(&f)
}

/// `Ok` when the plan has `f`, else the refusal to show ("Kitchen screens are
/// on the paid plan").
pub fn require(plan: Plan, f: Feature) -> Result<(), Refusal> {
    entitlements(plan).require(f)
}

// ── The capability ceiling ──────────────────────────────────────────────────

/// Every capability the plan can grant: all of them, less those behind a
/// feature the plan lacks. Core capabilities are never behind a feature, so
/// they are always inside it.
pub fn ceiling(plan: Plan) -> CapSet {
    ceiling_with(plan, FeatureSet::EMPTY)
}

/// [`ceiling`] with the organisation's add-ons switched on too.
pub fn ceiling_with(plan: Plan, addons: FeatureSet) -> CapSet {
    let mut set = CapSet::all();
    for m in FEATURES {
        if !has_feature(plan, m.feature) && !addons.contains(m.feature) {
            for c in m.caps {
                set.remove(*c);
            }
        }
    }
    set
}

/// A person's effective capabilities cut to the plan's ceiling: the
/// intersection. Capped capabilities that fall out lose their limits, and "ask a
/// manager" cannot stand in for a capability the plan does not have. Paid
/// changes nothing; core capabilities survive every plan.
pub fn apply(plan: Plan, eff: EffectiveSet) -> EffectiveSet {
    apply_with(plan, FeatureSet::EMPTY, eff)
}

/// [`apply`] under the ceiling the plan and the add-ons make together.
pub fn apply_with(plan: Plan, addons: FeatureSet, mut eff: EffectiveSet) -> EffectiveSet {
    let ceil = ceiling_with(plan, addons);
    if ceil == CapSet::all() {
        return eff;
    }
    eff.caps = eff.caps.intersect(&ceil);
    eff.ask_manager = eff.ask_manager.intersect(&ceil);
    eff.limits
        .retain(|id, _| Cap::from_id(*id).is_some_and(|c| ceil.contains(c)));
    eff
}

// ── Limits ──────────────────────────────────────────────────────────────────

/// The kinds of device a branch activates.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Pos,
    Kds,
    Waiter,
}

/// The devices already active in one branch.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCounts {
    #[serde(default)]
    pub pos: u32,
    #[serde(default)]
    pub kds: u32,
    #[serde(default)]
    pub waiter: u32,
}

/// May the organisation add a branch, given its live (not deleted) branches?
/// A downgraded organisation keeps all of its branches but is past the limit,
/// so it cannot add one.
pub fn may_add_branch(plan: Plan, live_branches: u32) -> Result<(), Refusal> {
    match plan.meta().limits.branches {
        Some(max) if live_branches >= max => Err(Refusal::new(Code::BranchLimit, None)),
        _ => Ok(()),
    }
}

/// May one more device of `kind` be activated in a branch that already has
/// `active_in_branch`? A kind whose limit is zero is a feature the plan lacks
/// (kitchen screens, waiter devices) and says so, unless the plan has the
/// feature: Free has the floor on its till, but no waiter handhelds, so that
/// is a device limit.
pub fn may_activate(
    plan: Plan,
    kind: DeviceKind,
    active_in_branch: DeviceCounts,
) -> Result<(), Refusal> {
    let limits = plan.meta().limits;
    let (max, active, feature) = match kind {
        DeviceKind::Pos => (limits.pos_devices_per_branch, active_in_branch.pos, None),
        DeviceKind::Kds => (
            limits.kds_devices,
            active_in_branch.kds,
            Some(Feature::Kitchen),
        ),
        DeviceKind::Waiter => (
            limits.waiter_devices,
            active_in_branch.waiter,
            Some(Feature::Floor),
        ),
    };
    // A plan that has the feature (Free's floor) is limited, not locked out.
    let feature = feature.filter(|f| !plan.meta().features.contains(f));
    match max {
        Some(0) if feature.is_some() => Err(Refusal::new(Code::Feature, feature)),
        Some(max) if active >= max => Err(Refusal::new(Code::DeviceLimit, feature)),
        _ => Ok(()),
    }
}
