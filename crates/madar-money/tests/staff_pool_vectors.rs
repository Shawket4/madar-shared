//! The staff pool rule, executed from its vectors.
//!
//! `vectors/staff_pool_vectors.json` is hand-authored: it is the specification.
//! It used to be committed to both MadarRust and madar-core and run by an
//! identical test on each side (MadarRust `tests/staff_pool_vectors_tests.rs`,
//! madar-core `tests/staff_pool_vectors.rs`); this is that test, once. Change
//! the rule in the fixture first.

use madar_money::staff_pool::{self as engine, StaffPoolSettings};
use serde_json::Value;

fn vectors() -> Value {
    let raw = madar_money::vectors::STAFF_POOL;
    serde_json::from_str(raw).expect("staff_pool_vectors.json is valid JSON")
}

fn settings_of(v: &Value) -> StaffPoolSettings {
    StaffPoolSettings {
        enabled: v["enabled"].as_bool().unwrap(),
        daily_allowance: v["daily_allowance"].as_i64().unwrap() as i32,
        eligible_item_ids: v["eligible_item_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_str().unwrap().to_string())
            .collect(),
    }
}

#[test]
fn the_shared_vectors_decide_exactly_as_this_side_does() {
    let doc = vectors();
    let cases = doc["cases"].as_array().expect("cases");
    assert!(!cases.is_empty(), "the fixture must actually contain cases");

    for c in cases {
        let name = c["name"].as_str().unwrap();
        let d = engine::decide(
            &settings_of(&c["settings"]),
            c["business_date"].as_str().unwrap(),
            c["item_id"].as_str().unwrap(),
            c["note"].as_str().unwrap(),
            c["used"].as_i64().unwrap() as i32,
        );
        let want = &c["expect"];

        assert_eq!(
            d.allowed,
            want["allowed"].as_bool().unwrap(),
            "[{name}] allowed"
        );
        assert_eq!(
            d.refusal.map(|r| r.token()),
            want["refusal"].as_str(),
            "[{name}] refusal"
        );
        assert_eq!(
            d.overspent,
            want["overspent"].as_bool().unwrap(),
            "[{name}] overspent"
        );

        let p = &want["pool"];
        assert_eq!(
            d.pool.business_date,
            p["business_date"].as_str().unwrap(),
            "[{name}] date"
        );
        assert_eq!(
            d.pool.allowance,
            p["allowance"].as_i64().unwrap() as i32,
            "[{name}] allowance"
        );
        assert_eq!(
            d.pool.used,
            p["used"].as_i64().unwrap() as i32,
            "[{name}] used"
        );
        assert_eq!(
            d.pool.remaining,
            p["remaining"].as_i64().unwrap() as i32,
            "[{name}] remaining"
        );
        assert_eq!(
            d.pool.over,
            p["over"].as_i64().unwrap() as i32,
            "[{name}] over"
        );
    }
}

#[test]
fn the_shared_vectors_place_the_business_day_exactly_as_this_side_does() {
    let doc = vectors();
    let cases = doc["business_date_cases"]
        .as_array()
        .expect("business_date_cases");
    assert!(!cases.is_empty());

    for c in cases {
        let name = c["name"].as_str().unwrap();
        let tz: chrono_tz::Tz = c["tz"].as_str().unwrap().parse().expect("a real timezone");
        let at = chrono::DateTime::parse_from_rfc3339(c["at"].as_str().unwrap())
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(
            engine::business_date_of(tz, at).to_string(),
            c["expect"].as_str().unwrap(),
            "[{name}]"
        );
    }
}

/// An overspend must never turn into a refusal: the drink was already made.
#[test]
fn no_vector_ever_refuses_a_drink_for_being_over_the_allowance() {
    for c in vectors()["cases"].as_array().unwrap() {
        let e = &c["expect"];
        if e["overspent"].as_bool().unwrap() {
            assert!(
                e["allowed"].as_bool().unwrap(),
                "[{}] an overspend must land, never be refused",
                c["name"].as_str().unwrap()
            );
        }
    }
}
