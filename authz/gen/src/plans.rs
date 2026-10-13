//! `authz/spec/plans.toml`: validation against the capability registry and the
//! generated plan tables (Rust, TypeScript, Dart).
//!
//! A feature names capabilities by group and by key or `prefix.*` pattern; both
//! expand to NON-CORE capabilities only (a core capability can never be removed,
//! so it can never sit behind a plan feature; naming one exactly is an error).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::Deserialize;

use crate::{camel, js, rs_str, variant, CapSpec, Spec};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlansSpec {
    spec_version: u32,
    features: BTreeMap<String, FeatureSpec>,
    free: FreeSpec,
    plans: BTreeMap<String, PlanSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureSpec {
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    caps: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FreeSpec {
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    caps: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanSpec {
    features: Vec<String>,
    #[serde(default)]
    limits: PlanLimitsSpec,
    realtime: bool,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PlanLimitsSpec {
    branches: Option<u32>,
    pos_devices_per_branch: Option<u32>,
    kds_devices: Option<u32>,
    waiter_devices: Option<u32>,
}

/// The plans the `Plan` enum in madar-plan has; the table must match exactly.
const PLAN_KEYS: [&str; 2] = ["free", "paid"];

/// A feature's capabilities (by key, in id order), after validation.
pub struct Resolved {
    features: Vec<(String, Vec<CapSpec>)>,
}

fn pattern_matches(pat: &str, key: &str) -> bool {
    match pat.strip_suffix(".*") {
        Some(prefix) => {
            key.len() > prefix.len()
                && key.starts_with(prefix)
                && key[prefix.len()..].starts_with('.')
        }
        None => pat == key,
    }
}

pub fn validate(spec: &Spec, p: &PlansSpec) -> Result<Resolved, String> {
    if p.features.is_empty() {
        return Err("plans: no features".into());
    }
    for k in p.features.keys() {
        if k.is_empty() || !k.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Err(format!("plans: bad feature key {k:?}"));
        }
    }
    let cap = |key: &str| spec.cap.iter().find(|c| c.key == key);
    let mut owner: BTreeMap<String, String> = BTreeMap::new(); // capability -> feature
    let mut resolved = vec![];
    for (name, f) in &p.features {
        let mut keys: BTreeSet<String> = BTreeSet::new();
        for g in &f.groups {
            if !spec.groups.contains_key(g) {
                return Err(format!("plans: feature {name}: unknown group {g}"));
            }
            keys.extend(
                spec.cap
                    .iter()
                    .filter(|c| &c.group == g && c.tier != "core")
                    .map(|c| c.key.clone()),
            );
        }
        for pat in &f.caps {
            if pat.contains('*') {
                if !pat.ends_with(".*") || pat[..pat.len() - 2].contains('*') {
                    return Err(format!("plans: feature {name}: bad pattern {pat}"));
                }
                let hits: Vec<_> = spec
                    .cap
                    .iter()
                    .filter(|c| pattern_matches(pat, &c.key) && c.tier != "core")
                    .collect();
                if hits.is_empty() {
                    return Err(format!(
                        "plans: feature {name}: pattern {pat} matches no non-core capability"
                    ));
                }
                keys.extend(hits.into_iter().map(|c| c.key.clone()));
            } else {
                let Some(c) = cap(pat) else {
                    return Err(format!("plans: feature {name}: unknown capability {pat}"));
                };
                if c.tier == "core" {
                    return Err(format!(
                        "plans: feature {name}: {pat} is a core capability and can never sit behind a plan feature"
                    ));
                }
                keys.insert(pat.clone());
            }
        }
        for k in &keys {
            if let Some(other) = owner.insert(k.clone(), name.clone()) {
                return Err(format!(
                    "plans: capability {k} is behind both {other} and {name}"
                ));
            }
        }
        let mut caps: Vec<CapSpec> = keys.iter().filter_map(|k| cap(k)).cloned().collect();
        caps.sort_by_key(|c| c.id);
        resolved.push((name.clone(), caps));
    }

    // Free: groups exist, capabilities exist, and a free capability is not also
    // behind a feature.
    for g in &p.free.groups {
        if !spec.groups.contains_key(g) {
            return Err(format!("plans: free: unknown group {g}"));
        }
    }
    for k in &p.free.caps {
        let Some(c) = cap(k) else {
            return Err(format!("plans: free: unknown capability {k}"));
        };
        if c.tier == "core" {
            return Err(format!("plans: free: {k} is core, which is always on"));
        }
        if let Some(f) = owner.get(k) {
            return Err(format!("plans: free: {k} is also behind feature {f}"));
        }
    }
    // Every non-core capability is behind a feature or free. A new capability in
    // a group nobody listed must not silently become part of Free.
    let mut unaccounted = vec![];
    for c in spec.cap.iter().filter(|c| c.tier != "core") {
        let free = p.free.groups.contains(&c.group) || p.free.caps.contains(&c.key);
        if !owner.contains_key(&c.key) && !free {
            unaccounted.push(c.key.as_str());
        }
    }
    if !unaccounted.is_empty() {
        return Err(format!(
            "plans: not behind a feature and not free (add it to a feature or to [free]): {}",
            unaccounted.join(", ")
        ));
    }
    // A group that is wholly paid yet listed free would be misleading; a free
    // group with every capability carved out is a mistake worth catching.
    for g in &p.free.groups {
        let any_free = spec
            .cap
            .iter()
            .any(|c| &c.group == g && c.tier != "core" && !owner.contains_key(&c.key));
        if !any_free {
            return Err(format!(
                "plans: free group {g} has no free capability (every one is behind a feature)"
            ));
        }
    }

    // Plans.
    let keys: Vec<&str> = p.plans.keys().map(String::as_str).collect();
    if keys != PLAN_KEYS {
        return Err(format!(
            "plans: the plans must be exactly {PLAN_KEYS:?}, found {keys:?}"
        ));
    }
    for (name, plan) in &p.plans {
        let mut seen = BTreeSet::new();
        for f in &plan.features {
            if f != "*" && !p.features.contains_key(f) {
                return Err(format!("plans: plan {name}: unknown feature {f}"));
            }
            if !seen.insert(f) {
                return Err(format!("plans: plan {name}: feature {f} listed twice"));
            }
        }
        if plan.features.iter().any(|f| f == "*") && plan.features.len() != 1 {
            return Err(format!("plans: plan {name}: \"*\" must stand alone"));
        }
    }
    // Paid is a superset of Free: an upgrade never takes anything away.
    let free = feature_set(p, "free");
    let paid = feature_set(p, "paid");
    if !free.is_subset(&paid) {
        return Err("plans: free has a feature paid lacks".into());
    }
    let (fl, pl) = (&p.plans["free"].limits, &p.plans["paid"].limits);
    let le = |f: Option<u32>, pd: Option<u32>| match (f, pd) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(a), Some(b)) => a <= b,
    };
    if !(le(fl.branches, pl.branches)
        && le(fl.pos_devices_per_branch, pl.pos_devices_per_branch)
        && le(fl.kds_devices, pl.kds_devices)
        && le(fl.waiter_devices, pl.waiter_devices))
    {
        return Err("plans: free has a limit looser than paid".into());
    }
    if p.plans["free"].realtime && !p.plans["paid"].realtime {
        return Err("plans: free has realtime but paid does not".into());
    }
    // A device kind limited to zero is a feature gate: its feature must be off.
    for (limit, feature, name) in [
        (fl.kds_devices, "kitchen", "kds_devices"),
        (fl.waiter_devices, "floor", "waiter_devices"),
    ] {
        if limit == Some(0) && free.contains(feature) {
            return Err(format!(
                "plans: free allows no {name} but includes feature {feature}"
            ));
        }
    }
    Ok(Resolved { features: resolved })
}

fn feature_set(p: &PlansSpec, plan: &str) -> BTreeSet<String> {
    let f = &p.plans[plan].features;
    if f.iter().any(|x| x == "*") {
        p.features.keys().cloned().collect()
    } else {
        f.iter().cloned().collect()
    }
}

fn rs_limit(x: Option<u32>) -> String {
    x.map_or("None".into(), |n| format!("Some({n})"))
}

/// A limit in TypeScript and Dart: the number, or `null` for unlimited.
fn null_limit(x: Option<u32>) -> String {
    x.map_or("null".into(), |n| n.to_string())
}

pub fn gen_rust(p: &PlansSpec, r: &Resolved, hash: &str) -> String {
    let mut o = String::new();
    writeln!(
        o,
        "// @generated by authz-gen from authz/spec/plans.toml. DO NOT EDIT."
    )
    .unwrap();
    writeln!(o, "#![allow(clippy::all)]\n").unwrap();
    writeln!(o, "use madar_authz::Cap;\n").unwrap();
    writeln!(o, "use crate::{{FeatureMeta, PlanLimits, PlanMeta}};\n").unwrap();
    writeln!(o, "pub const SPEC_VERSION: u32 = {};", p.spec_version).unwrap();
    writeln!(o, "pub const SPEC_HASH: &str = {};\n", rs_str(hash)).unwrap();
    writeln!(
        o,
        "/// A thing a plan switches on: a set of capabilities plus the surfaces that\n/// have no capability of their own."
    )
    .unwrap();
    writeln!(
        o,
        "#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]"
    )
    .unwrap();
    writeln!(
        o,
        "#[cfg_attr(feature = \"tsify\", derive(tsify::Tsify))]\n#[serde(rename_all = \"snake_case\")]\n#[repr(u8)]\npub enum Feature {{"
    )
    .unwrap();
    for (i, (name, _)) in r.features.iter().enumerate() {
        writeln!(o, "    {} = {i},", variant(name)).unwrap();
    }
    writeln!(o, "}}\n").unwrap();
    writeln!(
        o,
        "pub const FEATURE_COUNT: usize = {};\n",
        r.features.len()
    )
    .unwrap();
    writeln!(o, "pub static FEATURES: &[FeatureMeta] = &[").unwrap();
    for (name, caps) in &r.features {
        writeln!(
            o,
            "    FeatureMeta {{ feature: Feature::{}, key: {}, caps: &[{}] }},",
            variant(name),
            rs_str(name),
            caps.iter()
                .map(|c| format!("Cap::{}", variant(&c.key)))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    writeln!(o, "];\n").unwrap();
    writeln!(o, "pub static PLANS: &[PlanMeta] = &[").unwrap();
    for (name, plan) in &p.plans {
        let feats: Vec<String> = if plan.features.iter().any(|f| f == "*") {
            r.features
                .iter()
                .map(|(f, _)| format!("Feature::{}", variant(f)))
                .collect()
        } else {
            plan.features
                .iter()
                .map(|f| format!("Feature::{}", variant(f)))
                .collect()
        };
        let l = &plan.limits;
        writeln!(
            o,
            "    PlanMeta {{ key: {}, features: &[{}], limits: PlanLimits {{ branches: {}, pos_devices_per_branch: {}, kds_devices: {}, waiter_devices: {} }}, realtime: {} }},",
            rs_str(name),
            feats.join(", "),
            rs_limit(l.branches),
            rs_limit(l.pos_devices_per_branch),
            rs_limit(l.kds_devices),
            rs_limit(l.waiter_devices),
            plan.realtime
        )
        .unwrap();
    }
    writeln!(o, "];").unwrap();
    o
}

fn plan_features(p: &PlansSpec, r: &Resolved, plan: &str) -> Vec<String> {
    let set = feature_set(p, plan);
    r.features
        .iter()
        .map(|(f, _)| f.clone())
        .filter(|f| set.contains(f))
        .collect()
}

pub fn gen_ts(p: &PlansSpec, r: &Resolved, hash: &str) -> String {
    let mut o = String::new();
    writeln!(
        o,
        "// @generated by authz-gen from MadarRust/authz/spec/plans.toml. DO NOT EDIT."
    )
    .unwrap();
    writeln!(o, "/* eslint-disable */\n").unwrap();
    writeln!(o, "import type {{ Capability }} from './capabilities';\n").unwrap();
    writeln!(o, "export const PLANS_SPEC_VERSION = {};", p.spec_version).unwrap();
    writeln!(o, "export const PLANS_SPEC_HASH = {};\n", js(hash)).unwrap();
    writeln!(o, "export type Plan = 'free' | 'paid';").unwrap();
    writeln!(o, "export type Feature =").unwrap();
    for (f, _) in &r.features {
        writeln!(o, "  | {}", js(f)).unwrap();
    }
    writeln!(o, ";\n").unwrap();
    writeln!(o, "export const FEATURES: readonly Feature[] = [").unwrap();
    for (f, _) in &r.features {
        writeln!(o, "  {},", js(f)).unwrap();
    }
    writeln!(o, "];\n").unwrap();
    writeln!(
        o,
        "/** A limit of `null` is unlimited. Devices are counted per branch. */"
    )
    .unwrap();
    writeln!(
        o,
        "export interface PlanLimits {{\n  branches: number | null;\n  pos_devices_per_branch: number | null;\n  kds_devices: number | null;\n  waiter_devices: number | null;\n}}\n"
    )
    .unwrap();
    writeln!(
        o,
        "export interface Entitlements {{\n  features: readonly Feature[];\n  limits: PlanLimits;\n  realtime: boolean;\n}}\n"
    )
    .unwrap();
    writeln!(o, "export const PLANS: Record<Plan, Entitlements> = {{").unwrap();
    for (name, plan) in &p.plans {
        let l = &plan.limits;
        let n = null_limit;
        writeln!(
            o,
            "  {name}: {{\n    features: [{}],\n    limits: {{ branches: {}, pos_devices_per_branch: {}, kds_devices: {}, waiter_devices: {} }},\n    realtime: {},\n  }},",
            plan_features(p, r, name)
                .iter()
                .map(|f| js(f))
                .collect::<Vec<_>>()
                .join(", "),
            n(l.branches),
            n(l.pos_devices_per_branch),
            n(l.kds_devices),
            n(l.waiter_devices),
            plan.realtime
        )
        .unwrap();
    }
    writeln!(o, "}};\n").unwrap();
    writeln!(
        o,
        "/** The capabilities each feature switches. A plan without the feature loses them. */"
    )
    .unwrap();
    writeln!(
        o,
        "export const FEATURE_CAPABILITIES: Record<Feature, readonly Capability[]> = {{"
    )
    .unwrap();
    for (f, caps) in &r.features {
        if caps.is_empty() {
            writeln!(o, "  {f}: [],").unwrap();
            continue;
        }
        writeln!(o, "  {f}: [").unwrap();
        for c in caps {
            writeln!(o, "    {},", js(&c.key)).unwrap();
        }
        writeln!(o, "  ],").unwrap();
    }
    writeln!(o, "}};\n").unwrap();
    writeln!(
        o,
        "/** An unknown or missing plan is Paid: an old server never locks a paying shop out. */"
    )
    .unwrap();
    writeln!(
        o,
        "export function entitlements(plan: string | null | undefined): Entitlements {{\n  return plan === 'free' ? PLANS.free : PLANS.paid;\n}}\n"
    )
    .unwrap();
    writeln!(
        o,
        "export function hasFeature(plan: string | null | undefined, feature: Feature): boolean {{\n  return entitlements(plan).features.includes(feature);\n}}"
    )
    .unwrap();
    o
}

pub fn gen_dart(p: &PlansSpec, r: &Resolved, hash: &str) -> String {
    let mut o = String::new();
    writeln!(
        o,
        "// @generated by authz-gen from MadarRust/authz/spec/plans.toml. DO NOT EDIT."
    )
    .unwrap();
    writeln!(o, "// ignore_for_file: constant_identifier_names\n").unwrap();
    writeln!(
        o,
        "/// What a plan switches on. Ask `entitlements()` from the core for the live"
    )
    .unwrap();
    writeln!(
        o,
        "/// answer; this table is for screens that render before a session exists."
    )
    .unwrap();
    writeln!(o, "abstract final class PlanFeature {{").unwrap();
    writeln!(o, "  static const int specVersion = {};", p.spec_version).unwrap();
    writeln!(o, "  static const String specHash = '{hash}';").unwrap();
    for (f, _) in &r.features {
        writeln!(o, "  static const String {} = '{f}';", camel(f)).unwrap();
    }
    writeln!(o, "}}\n").unwrap();
    writeln!(o, "class PlanEntitlements {{").unwrap();
    writeln!(o, "  const PlanEntitlements({{").unwrap();
    writeln!(o, "    required this.features,").unwrap();
    writeln!(o, "    required this.branches,").unwrap();
    writeln!(o, "    required this.posDevicesPerBranch,").unwrap();
    writeln!(o, "    required this.kdsDevices,").unwrap();
    writeln!(o, "    required this.waiterDevices,").unwrap();
    writeln!(o, "    required this.realtime,").unwrap();
    writeln!(o, "  }});\n").unwrap();
    writeln!(o, "  final Set<String> features;").unwrap();
    writeln!(
        o,
        "\n  /// A limit of null is unlimited. Devices are counted per branch."
    )
    .unwrap();
    writeln!(o, "  final int? branches;").unwrap();
    writeln!(o, "  final int? posDevicesPerBranch;").unwrap();
    writeln!(o, "  final int? kdsDevices;").unwrap();
    writeln!(o, "  final int? waiterDevices;").unwrap();
    writeln!(o, "  final bool realtime;\n").unwrap();
    writeln!(
        o,
        "  bool has(String feature) => features.contains(feature);"
    )
    .unwrap();
    writeln!(o, "}}\n").unwrap();
    writeln!(o, "abstract final class Plans {{").unwrap();
    for (name, plan) in &p.plans {
        let l = &plan.limits;
        let n = null_limit;
        writeln!(
            o,
            "  static const PlanEntitlements {name} = PlanEntitlements("
        )
        .unwrap();
        let feats = plan_features(p, r, name);
        if feats.is_empty() {
            writeln!(o, "    features: <String>{{}},").unwrap();
        } else {
            writeln!(o, "    features: {{").unwrap();
            for f in &feats {
                writeln!(o, "      '{f}',").unwrap();
            }
            writeln!(o, "    }},").unwrap();
        }
        writeln!(o, "    branches: {},", n(l.branches)).unwrap();
        writeln!(
            o,
            "    posDevicesPerBranch: {},",
            n(l.pos_devices_per_branch)
        )
        .unwrap();
        writeln!(o, "    kdsDevices: {},", n(l.kds_devices)).unwrap();
        writeln!(o, "    waiterDevices: {},", n(l.waiter_devices)).unwrap();
        writeln!(o, "    realtime: {},", plan.realtime).unwrap();
        writeln!(o, "  );\n").unwrap();
    }
    writeln!(
        o,
        "  /// An unknown or missing plan is Paid: an old server never locks a"
    )
    .unwrap();
    writeln!(o, "  /// paying shop out.").unwrap();
    writeln!(
        o,
        "  static PlanEntitlements of(String? plan) => plan == 'free' ? free : paid;"
    )
    .unwrap();
    writeln!(o, "}}").unwrap();
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPS: &str = include_str!("../../spec/capabilities.toml");
    const PLANS: &str = include_str!("../../spec/plans.toml");

    fn check(plans: &str) -> Result<(), String> {
        let spec: Spec = toml::from_str(CAPS).unwrap();
        let p: PlansSpec = toml::from_str(plans).map_err(|e| e.to_string())?;
        validate(&spec, &p).map(|_| ())
    }

    fn fails(from: &str, to: &str, needle: &str) {
        assert!(PLANS.contains(from), "{from:?} is not in plans.toml");
        let err = check(&PLANS.replacen(from, to, 1)).expect_err(needle);
        assert!(err.contains(needle), "{err}");
    }

    #[test]
    fn the_real_spec_is_valid() {
        check(PLANS).unwrap();
    }

    #[test]
    fn a_group_must_exist() {
        fails(
            "groups = [\"delivery\"]",
            "groups = [\"delivry\"]",
            "unknown group delivry",
        );
    }

    #[test]
    fn a_core_capability_cannot_sit_behind_a_feature() {
        fails(
            "caps = [\"tickets.*\"",
            "caps = [\"tickets.read\", \"tickets.*\"",
            "core capability",
        );
        // `tickets.read` is core and named exactly; the pattern form skips it.
        fails(
            "caps = [\"orders.service_charge.waive\"]",
            "caps = [\"branches.read\"]",
            "is core",
        );
    }

    #[test]
    fn every_capability_is_behind_a_feature_or_free() {
        // Drop the `hr` group from Dawam: its capabilities fall out of every list.
        fails("groups = [\"hr\"]", "groups = []", "hr.staff.read");
        // Drop the `customers` group from the free list.
        fails(
            "\"settings\", \"reports\", \"customers\"]",
            "\"settings\", \"reports\"]",
            "customers.attach",
        );
    }

    #[test]
    fn a_capability_has_one_feature() {
        fails(
            "groups = [\"hr\"]",
            "groups = [\"hr\"]\ncaps = [\"inventory.read\"]",
            "behind both",
        );
    }

    #[test]
    fn patterns_must_match_something() {
        fails(
            "\"recipes.*\"",
            "\"recipez.*\"",
            "matches no non-core capability",
        );
    }

    #[test]
    fn plans_are_exactly_free_and_paid() {
        fails("[plans.free]", "[plans.trial]", "must be exactly");
        fails(
            "features = [\"*\"]",
            "features = [\"nope\"]",
            "unknown feature nope",
        );
    }

    #[test]
    fn free_may_not_exceed_paid() {
        fails(
            "limits = {}",
            "limits = { branches = 0 }",
            "looser than paid",
        );
        fails(
            "[plans.free]\nfeatures = []",
            "[plans.free]\nfeatures = [\"kitchen\"]",
            "allows no kds_devices",
        );
    }
}
