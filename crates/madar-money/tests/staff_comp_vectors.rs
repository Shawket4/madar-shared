//! What a staff drink is given for free, executed from its vectors.
//!
//! The till prices a staff drink offline and the server re-prices it at
//! replay; `vectors/staff_comp_vectors.json` is what stands between the two and
//! a `comp_mismatch` flag on every sale. It used to be committed to both repos
//! and run by a test on each side (MadarRust `tests/staff_comp_vectors_tests.rs`,
//! madar-core `tests/staff_comp_vectors.rs`); this is that test, once, with the
//! POS side's stricter case count. Change the rule in the fixture first.

use madar_money::staff_comp::{self as comp, CompInput, CompResult};
use serde_json::Value;

fn vectors() -> Value {
    let raw = madar_money::vectors::STAFF_COMP;
    serde_json::from_str(raw).expect("staff_comp_vectors.json is valid JSON")
}

#[test]
fn the_shared_vectors_comp_exactly_as_this_side_does() {
    let doc = vectors();
    let cases = doc["cases"].as_array().expect("cases");
    assert!(
        cases.len() >= 33,
        "the fixture pins all 33 cases, has {}",
        cases.len()
    );

    for c in cases {
        let name = c["name"].as_str().unwrap();
        let input: CompInput = serde_json::from_value(c["input"].clone())
            .unwrap_or_else(|e| panic!("[{name}] input does not decode: {e}"));
        let want: CompResult = serde_json::from_value(c["expect"].clone())
            .unwrap_or_else(|e| panic!("[{name}] expect does not decode: {e}"));
        assert_eq!(comp::comp(&input), want, "[{name}]");
    }
}

/// The properties the rule promises, over every vector: nothing is ever
/// charged below zero, the comp never exceeds what rang, and the parts add up.
#[test]
fn every_vector_adds_up_and_never_goes_negative() {
    for c in vectors()["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let input: CompInput = serde_json::from_value(c["input"].clone()).unwrap();
        let r = comp::comp(&input);
        let b = &r.breakdown;
        assert!(
            r.free_per_unit >= 0 && r.charged_per_unit >= 0,
            "[{name}] sign"
        );
        assert_eq!(
            r.free_per_unit + r.charged_per_unit,
            b.normal_per_unit,
            "[{name}] sum"
        );
        assert_eq!(
            r.free_per_unit,
            b.size_comp + b.groups.iter().map(|g| g.comp).sum::<i32>(),
            "[{name}] parts"
        );
        assert_eq!(
            b.picks.iter().map(|p| p.comp).sum::<i32>(),
            b.groups.iter().map(|g| g.comp).sum::<i32>(),
            "[{name}] the group comps are the pick comps"
        );
        for g in &b.groups {
            assert!(
                g.comp <= g.allowance && g.comp <= g.picked,
                "[{name}] group cap"
            );
        }
        assert!(b.size_comp <= input.unit_price.max(0), "[{name}] size cap");
        assert_eq!(
            r.line_comp,
            r.free_per_unit * input.quantity.max(0),
            "[{name}] line"
        );
    }
}

/// The names the task's owner asked to see pinned. A rename in the fixture that
/// drops one of these scenarios should be a decision, not an accident.
#[test]
fn the_fixture_covers_the_scenarios_the_rule_was_written_for() {
    let doc = vectors();
    let names: Vec<&str> = doc["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for needle in [
        "no sizes",
        "cheapest size",
        "larger size",
        "inactive cheapest",
        "branch price override",
        "optional add-ons",
        "default option is free",
        "no default",
        "equal-priced",
        "no credit",
        "pricier",
        "min 2",
        "two required groups",
        "quantity 3",
        "zero-priced item",
        "everything free",
        "nothing free",
        "rounding",
    ] {
        assert!(
            names.iter().any(|n| n.contains(needle)),
            "no vector covers `{needle}`"
        );
    }
}
