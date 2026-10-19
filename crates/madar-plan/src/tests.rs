use std::collections::BTreeMap;

use madar_authz::{
    core_set, template_grants, Cap, CapSet, EffectiveSet, Kinds, Limits, RoleKind, TEMPLATES,
};
use serde::Deserialize;

use crate::*;

fn plan(s: &str) -> Plan {
    match s {
        "free" => Plan::Free,
        "paid" => Plan::Paid,
        other => panic!("vector plan {other}"),
    }
}

#[derive(Deserialize)]
struct Vectors {
    plans: Vec<PlanCase>,
    addons: Vec<AddonCase>,
    parse: Vec<(String, String)>,
    spot: Vec<Spot>,
    ceiling_roles: Vec<RoleCase>,
    branches: Vec<BranchCase>,
    devices: Vec<DeviceCase>,
    downgrade: Vec<DowngradeCase>,
    topics: Vec<TopicCase>,
    refusals: Vec<RefusalCase>,
}

#[derive(Deserialize)]
struct PlanCase {
    plan: String,
    expected: serde_json::Value,
}

#[derive(Deserialize)]
struct AddonCase {
    plan: String,
    addons: FeatureSet,
    features: FeatureSet,
    kept: Vec<String>,
    cut: Vec<String>,
}

#[derive(Deserialize)]
struct Spot {
    plan: String,
    cap: String,
    kept: bool,
}

#[derive(Deserialize)]
struct RoleCase {
    plan: String,
    template: String,
    role: RoleKind,
    grants: usize,
    lost: Vec<String>,
}

#[derive(Deserialize)]
struct BranchCase {
    plan: String,
    live: u32,
    expected: Option<Refusal>,
}

#[derive(Deserialize)]
struct DeviceCase {
    plan: String,
    kind: DeviceKind,
    active: DeviceCounts,
    expected: Option<Refusal>,
}

#[derive(Deserialize)]
struct DowngradeCase {
    name: String,
    tills: Vec<TillUse>,
    picks: Vec<Pick>,
    expected: Vec<Keep>,
}

#[derive(Deserialize)]
struct TopicCase {
    plan: String,
    role: String,
    expected: Vec<String>,
}

#[derive(Deserialize)]
struct RefusalCase {
    code: Code,
    feature: Option<Feature>,
    en: String,
    ar: String,
}

fn vectors() -> Vectors {
    serde_json::from_str(vectors::PLAN).expect("plan_vectors.json")
}

#[test]
fn entitlements_match_the_vectors() {
    for c in vectors().plans {
        assert_eq!(
            serde_json::to_value(entitlements(plan(&c.plan))).unwrap(),
            c.expected,
            "{}",
            c.plan
        );
    }
}

#[test]
fn addons_add_features_and_lift_the_ceiling_for_them_only() {
    for c in vectors().addons {
        let p = plan(&c.plan);
        let e = entitlements_with(p, c.addons);
        assert_eq!(e.features, c.features, "{} {:?}", c.plan, c.addons);
        assert_eq!(e.limits, entitlements(p).limits, "add-ons never move limits");
        assert_eq!(e.realtime, entitlements(p).realtime);
        let ceil = ceiling_with(p, c.addons);
        for (keys, want) in [(&c.kept, true), (&c.cut, false)] {
            for k in keys {
                let cap = Cap::from_key(k).unwrap_or_else(|| panic!("no capability {k}"));
                assert_eq!(ceil.contains(cap), want, "{} {:?} {k}", c.plan, c.addons);
            }
        }
    }
    // No add-ons is exactly the plan.
    for p in [Plan::Free, Plan::Paid] {
        assert_eq!(entitlements_with(p, FeatureSet::EMPTY), entitlements(p));
        assert_eq!(ceiling_with(p, FeatureSet::EMPTY), ceiling(p));
    }
}

#[test]
fn unknown_plans_are_paid() {
    for (raw, want) in vectors().parse {
        assert_eq!(Plan::parse(&raw), plan(&want), "{raw:?}");
        let json = serde_json::to_string(&raw).unwrap();
        assert_eq!(serde_json::from_str::<Plan>(&json).unwrap(), plan(&want));
    }
    assert_eq!(serde_json::from_str::<Plan>("null").unwrap(), Plan::Paid);
    assert_eq!(serde_json::to_string(&Plan::Free).unwrap(), "\"free\"");
    assert_eq!(serde_json::to_string(&Plan::Paid).unwrap(), "\"paid\"");
    assert_eq!(Plan::default(), Plan::Paid);
}

#[test]
fn spot_checks_of_the_ceiling() {
    for s in vectors().spot {
        let cap = Cap::from_key(&s.cap).unwrap_or_else(|| panic!("no capability {}", s.cap));
        assert_eq!(
            ceiling(plan(&s.plan)).contains(cap),
            s.kept,
            "{} {}",
            s.plan,
            s.cap
        );
    }
}

#[test]
fn the_ceiling_over_every_templates_role_grants() {
    let v = vectors();
    let mut seen = 0;
    for c in &v.ceiling_roles {
        let grants = template_grants(&c.template, c.role).expect("template");
        assert_eq!(grants.len(), c.grants, "{} {:?} grants", c.template, c.role);
        let eff = EffectiveSet {
            caps: grants,
            kinds: Kinds::default(),
            ..Default::default()
        };
        let cut = apply(plan(&c.plan), eff).caps;
        let lost: Vec<&str> = grants.minus(&cut).keys();
        assert_eq!(lost, c.lost, "{} {} {:?}", c.plan, c.template, c.role);
        seen += 1;
    }
    // Both plans over every template and every role the template has.
    let want: usize = TEMPLATES.iter().map(|t| t.roles.len()).sum::<usize>() * 2;
    assert_eq!(seen, want, "a template or role has no ceiling vector");
}

#[test]
fn limits_at_and_past_their_boundary() {
    let v = vectors();
    for c in &v.branches {
        assert_eq!(
            may_add_branch(plan(&c.plan), c.live).err(),
            c.expected,
            "{} {}",
            c.plan,
            c.live
        );
    }
    for c in &v.devices {
        assert_eq!(
            may_activate(plan(&c.plan), c.kind, c.active).err(),
            c.expected,
            "{} {:?} {:?}",
            c.plan,
            c.kind,
            c.active
        );
    }
}

#[test]
fn downgrade_keeps_the_most_recent_till_unless_the_owner_picked() {
    for c in vectors().downgrade {
        assert_eq!(
            downgrade_keeps(&c.tills, &c.picks),
            c.expected,
            "{}",
            c.name
        );
        // Input order never matters.
        let mut rev = c.tills.clone();
        rev.reverse();
        assert_eq!(
            downgrade_keeps(&rev, &c.picks),
            c.expected,
            "{} reversed",
            c.name
        );
    }
}

#[test]
fn topics_per_plan_and_role() {
    for c in vectors().topics {
        let got: Vec<&str> = realtime_topics(plan(&c.plan), &c.role)
            .into_iter()
            .map(Topic::as_str)
            .collect();
        assert_eq!(got, c.expected, "{} {:?}", c.plan, c.role);
    }
}

#[test]
fn refusal_wording_is_pinned() {
    let v = vectors();
    for c in &v.refusals {
        let r = Refusal::new(c.code, c.feature);
        assert_eq!((r.en(), r.ar()), (c.en.as_str(), c.ar.as_str()), "{:?}", r);
    }
    // Every code, with and without a feature, and every feature, has a vector.
    for code in [
        Code::Feature,
        Code::BranchLimit,
        Code::DeviceLimit,
        Code::NoRealtime,
        Code::TillInactive,
    ] {
        for feature in std::iter::once(None).chain(Feature::all().map(Some)) {
            assert!(
                v.refusals
                    .iter()
                    .any(|c| c.code == code && c.feature == feature),
                "no vector for {code:?} {feature:?}"
            );
        }
    }
}

#[test]
fn refusals_read_well_in_both_languages() {
    for code in [
        Code::Feature,
        Code::BranchLimit,
        Code::DeviceLimit,
        Code::NoRealtime,
        Code::TillInactive,
    ] {
        for feature in std::iter::once(None).chain(Feature::all().map(Some)) {
            let r = Refusal::new(code, feature);
            let (en, ar) = (r.en(), r.ar());
            assert!(en.ends_with('.') && ar.ends_with('.'), "{r:?}");
            assert!(en.is_ascii() && en != ar, "{r:?}");
            // Arabic in Arabic script only: no Latin letter slips in, and no
            // Western digit (the apps print numbers themselves).
            assert!(
                ar.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
                    && !ar.chars().any(|c| c.is_ascii_alphanumeric()),
                "{r:?}: {ar}"
            );
            // No colloquial markers: formal Modern Standard Arabic.
            for slang in ["مش ", "دلوقتي", "ده ", "دي ", "إيه", "علشان"] {
                assert!(!ar.contains(slang), "{r:?}: {ar}");
            }
            assert_eq!(r.message("ar"), ar);
            assert_eq!(r.message("ar-EG"), ar);
            assert_eq!(r.message("en"), en);
            assert_eq!(r.message("fr"), en);
        }
        assert_eq!(Code::parse(code.as_str()), Some(code));
    }
    assert_eq!(Code::parse("nope"), None);
    assert_eq!(
        Refusal::feature(Feature::Kitchen).en(),
        "Kitchen screens are on the paid plan."
    );
}

#[test]
fn refusals_serialise_with_the_api_codes() {
    let r = Refusal::feature(Feature::Kitchen);
    assert_eq!(
        serde_json::to_value(r).unwrap(),
        serde_json::json!({ "code": "plan_feature", "feature": "kitchen" })
    );
    let r = Refusal::new(Code::BranchLimit, None);
    assert_eq!(
        serde_json::to_value(r).unwrap(),
        serde_json::json!({ "code": "plan_branch_limit", "feature": null })
    );
}

// ── The generated tables ────────────────────────────────────────────────────

#[test]
fn the_tables_are_in_discriminant_order() {
    for (i, m) in FEATURES.iter().enumerate() {
        assert_eq!(m.feature as usize, i);
        assert_eq!(Feature::parse(m.key), Some(m.feature));
        assert_eq!(m.feature.as_str(), m.key);
    }
    assert_eq!(FEATURES.len(), FEATURE_COUNT);
}

#[test]
fn paid_has_everything_and_free_nothing_extra() {
    assert_eq!(ceiling(Plan::Paid), CapSet::all());
    let paid = entitlements(Plan::Paid);
    let free = entitlements(Plan::Free);
    assert!(Feature::all().all(|f| paid.has(f)));
    assert!(free.has(Feature::Floor) && Feature::all().filter(|f| free.has(*f)).count() == 1);
    assert!(ceiling(Plan::Free).is_subset(&ceiling(Plan::Paid)));
    assert!(free.limits.branches <= paid.limits.branches.or(Some(u32::MAX)));
    assert!(paid.realtime && !free.realtime);
}

#[test]
fn a_feature_is_exactly_its_capabilities_gone_from_free() {
    let mut behind = CapSet::EMPTY;
    for m in FEATURES {
        let caps = m.feature.caps();
        assert!(
            behind.intersect(&caps).is_empty(),
            "{} shares a capability",
            m.key
        );
        behind = behind.union(&caps);
        assert_eq!(
            ceiling(Plan::Free).intersect(&caps).is_empty(),
            !entitlements(Plan::Free).has(m.feature),
            "Free keeps a {} capability",
            m.key
        );
        for c in m.caps {
            assert_eq!(Feature::of_cap(*c), Some(m.feature));
        }
    }
    let mut gated = CapSet::EMPTY;
    for m in FEATURES
        .iter()
        .filter(|m| !entitlements(Plan::Free).has(m.feature))
    {
        gated = gated.union(&m.feature.caps());
    }
    assert_eq!(ceiling(Plan::Free), CapSet::all().minus(&gated));
}

#[test]
fn core_capabilities_survive_every_plan() {
    for kind in [
        RoleKind::OrgAdmin,
        RoleKind::BranchManager,
        RoleKind::Teller,
        RoleKind::Waiter,
        RoleKind::Kitchen,
    ] {
        let core = core_set(kind);
        for p in [Plan::Free, Plan::Paid] {
            assert!(core.is_subset(&ceiling(p)), "{kind:?} core is cut on {p:?}");
            let eff = EffectiveSet {
                caps: core,
                ..Default::default()
            };
            assert_eq!(apply(p, eff).caps, core, "{kind:?} {p:?}");
        }
    }
}

#[test]
fn apply_cuts_limits_and_ask_manager_with_the_capability() {
    let mut eff = EffectiveSet {
        caps: [Cap::OrdersCreate, Cap::InventoryAdjust, Cap::LoyaltyUse]
            .into_iter()
            .collect(),
        ask_manager: [Cap::InventoryAdjust, Cap::OrdersVoid]
            .into_iter()
            .collect(),
        owner: true,
        ..Default::default()
    };
    let capped = Limits {
        max_amount: Some(500),
        ..Default::default()
    };
    eff.limits = BTreeMap::from([
        (Cap::InventoryAdjust.id(), capped),
        (Cap::OrdersCreate.id(), capped),
    ]);
    assert_eq!(apply(Plan::Paid, eff.clone()), eff);
    let free = apply(Plan::Free, eff);
    assert_eq!(free.caps.keys(), ["orders.create"]);
    assert_eq!(free.ask_manager.keys(), ["orders.void"]);
    assert_eq!(
        free.limits.keys().copied().collect::<Vec<_>>(),
        [Cap::OrdersCreate.id()]
    );
    assert!(free.owner, "the plan never changes who the owner is");
}

#[test]
fn feature_sets_cross_as_lists_of_keys() {
    let s: FeatureSet = [Feature::Online, Feature::Kitchen].into_iter().collect();
    assert_eq!(
        serde_json::to_string(&s).unwrap(),
        "[\"kitchen\",\"online\"]"
    );
    assert_eq!(
        serde_json::from_str::<FeatureSet>("[\"online\",\"kitchen\"]").unwrap(),
        s
    );
    assert!(s.contains(Feature::Online) && !s.contains(Feature::Basira));
    assert_eq!(s.len(), 2);
}

#[test]
fn require_names_the_feature() {
    assert_eq!(require(Plan::Paid, Feature::Basira), Ok(()));
    assert_eq!(
        require(Plan::Free, Feature::Basira),
        Err(Refusal::feature(Feature::Basira))
    );
    assert!(!has_feature(Plan::Free, Feature::Online) && has_feature(Plan::Paid, Feature::Online));
}

/// `WRITE_PLAN_VECTORS=<path>` writes the generated parts of the vector file
/// (the ceiling over every template's role grants, the refusals' wording) for
/// review: `cargo test -p madar-plan --lib print_vectors -- --ignored`.
#[test]
#[ignore]
fn print_vectors() {
    let path = std::env::var("WRITE_PLAN_VECTORS").expect("WRITE_PLAN_VECTORS");
    let mut roles = vec![];
    for t in TEMPLATES {
        for role in t.roles {
            let grants = template_grants(t.key, *role).unwrap();
            for p in [Plan::Free, Plan::Paid] {
                let cut = apply(
                    p,
                    EffectiveSet {
                        caps: grants,
                        ..Default::default()
                    },
                )
                .caps;
                roles.push(serde_json::json!({
                    "plan": p.as_str(), "template": t.key, "role": role,
                    "grants": grants.len(), "lost": grants.minus(&cut).keys(),
                }));
            }
        }
    }
    let mut refusals = vec![];
    for code in [
        Code::Feature,
        Code::BranchLimit,
        Code::DeviceLimit,
        Code::NoRealtime,
        Code::TillInactive,
    ] {
        for feature in std::iter::once(None).chain(Feature::all().map(Some)) {
            let r = Refusal::new(code, feature);
            refusals.push(
                serde_json::json!({ "code": code, "feature": feature, "en": r.en(), "ar": r.ar() }),
            );
        }
    }
    let out = serde_json::json!({ "ceiling_roles": roles, "refusals": refusals });
    std::fs::write(path, serde_json::to_string(&out).unwrap()).unwrap();
}
