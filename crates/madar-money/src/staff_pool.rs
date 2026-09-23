//! The staff drinks pool — what a branch may give its own people in a day, and
//! the one place that decides it.
//!
//! The shop used to ring a staff drink as its own zero-priced twin of the real
//! item ("Latte staff" beside "Latte"). That hid staff consumption in a second
//! menu item nobody kept a recipe on, so the stock was never deducted and the
//! menu grew a shadow copy of itself. The pool replaces it: the REAL item is
//! rung, at zero, against an allowance that belongs to the BRANCH and resets
//! every business day.
//!
//! ## The rules (owner, 2026-09-19)
//!
//! * **The pool belongs to the branch, per day** — not to a person. There is no
//!   "who is this for" picker and a staff drink is never attributed to a staff
//!   member.
//! * **A note is REQUIRED.** Whitespace is not a note. The act is refused
//!   without one, because the note is the only record of who drank it and why,
//!   in the teller's own words.
//! * **Eligibility is a list.** Only the items in it count. An EMPTY list means
//!   the pool is off — a pool with no eligible items is not a pool.
//! * **An overspend is allowed and MARKED, never blocked.** The drink was
//!   already made and the sale already happened; refusing it after the fact
//!   would only lose the record. The (allowance+1)th drink of the day lands
//!   with `overspent: true`, and that flag is what the review queue, the
//!   reports and the Z report read.
//! * **The day is the branch's business day** — the same local-midnight to
//!   local-midnight boundary the Z report and the backend's
//!   `service_day_bounds` use, in the branch's timezone. Never midnight UTC:
//!   a shop in Cairo closing at 01:00 must not have its pool turn over while
//!   the last order is still being rung.
//!
//! ## One copy
//!
//! The till decides offline and the server re-decides at replay, so this rule
//! runs on both sides. It used to be two copies (madar-core `staff_pool.rs` and
//! MadarRust `staff_pool/engine.rs`) pinned by a hand-copied
//! `staff_pool_vectors.json`; it is this one now, and the vectors are this
//! crate's tests. When the two sides disagree the SERVER's answer is recorded,
//! but a disagreement never rejects a drink that already happened: it lands,
//! marked `overspent` and flagged (PERMISSIONS_ARCHITECTURE §4.4.5).

use serde::{Deserialize, Serialize};

/// What the org (or a branch overriding it) allows.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct StaffPoolSettings {
    /// The owner's master switch. Off means the action never offers itself.
    pub enabled: bool,
    /// How many staff drinks this branch may give in one business day.
    pub daily_allowance: i32,
    /// The menu items that count. EMPTY = nothing counts = the pool is off.
    pub eligible_item_ids: Vec<String>,
}

/// A branch's pool as it stands on one business day.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StaffPoolDay {
    /// The branch-local business date, `YYYY-MM-DD`.
    pub business_date: String,
    pub allowance: i32,
    /// Drinks already recorded on this date, this device's view of it.
    pub used: i32,
    /// What is left. Never negative — an overspend shows in `over`.
    pub remaining: i32,
    /// How far past the allowance the day has gone. 0 when inside it.
    pub over: i32,
}

/// Why a staff drink was refused. These are the ONLY refusals: an overspend is
/// never one of them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaffDrinkRefusal {
    /// The owner has not switched the pool on.
    PoolOff,
    /// The pool is on but nothing is eligible, so it cannot be spent.
    NoEligibleItems,
    /// This item is not on the list.
    ItemNotEligible,
    /// No note, or only whitespace.
    NoteRequired,
}

impl StaffDrinkRefusal {
    /// The i18n key of the teller-facing reason (the till's words).
    pub fn key(self) -> &'static str {
        match self {
            Self::PoolOff => "staff_pool.refused.off",
            Self::NoEligibleItems => "staff_pool.refused.no_items",
            Self::ItemNotEligible => "staff_pool.refused.item",
            Self::NoteRequired => "staff_pool.refused.note",
        }
    }

    /// The stable token the server records on a flag, and reports read.
    pub fn token(self) -> &'static str {
        match self {
            Self::PoolOff => "pool_off",
            Self::NoEligibleItems => "no_eligible_items",
            Self::ItemNotEligible => "item_not_eligible",
            Self::NoteRequired => "note_required",
        }
    }

    /// The refusal as the server's API states it.
    pub fn message(self) -> &'static str {
        match self {
            Self::PoolOff => "The staff pool is switched off for this branch",
            Self::NoEligibleItems => "No drinks are set for the staff pool yet",
            Self::ItemNotEligible => "This item is not on the staff pool list",
            Self::NoteRequired => "A staff drink needs a note saying who it is for",
        }
    }
}

/// The verdict on one staff drink. Both the till (before ringing) and the
/// server (at replay) compute this from the same inputs and must agree.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StaffDrinkDecision {
    /// The drink may be recorded. False means refused outright.
    pub allowed: bool,
    /// Set when `allowed` is false.
    pub refusal: Option<StaffDrinkRefusal>,
    /// Allowed, but past the allowance. The record carries this flag and the
    /// owner's review queue and the reports show it.
    pub overspent: bool,
    /// The pool AFTER this drink, when it is allowed; the pool as it stands
    /// when it is refused.
    pub pool: StaffPoolDay,
}

/// A note is a note when it has a non-whitespace character in it.
pub fn note_is_given(note: &str) -> bool {
    !note.trim().is_empty()
}

/// The pool's state from an allowance and a used count. `remaining` never goes
/// below zero and `over` never above it; exactly one of them is non-zero once
/// the day is past its allowance.
pub fn pool_state(business_date: &str, allowance: i32, used: i32) -> StaffPoolDay {
    let allowance = allowance.max(0);
    let used = used.max(0);
    StaffPoolDay {
        business_date: business_date.to_string(),
        allowance,
        used,
        remaining: (allowance - used).max(0),
        over: (used - allowance).max(0),
    }
}

/// THE decision. `used` is how many drinks the branch has already recorded on
/// `business_date` as this side knows it — the till's own count offline, the
/// converged count once peers or the cloud have been heard from, the server's
/// authoritative count at replay.
///
/// Order matters: the settings are checked before the item, and the item
/// before the note, so a teller is told the most fundamental thing that is
/// wrong rather than being sent to write a note for a pool that is switched
/// off.
pub fn decide(
    settings: &StaffPoolSettings,
    business_date: &str,
    item_id: &str,
    note: &str,
    used: i32,
) -> StaffDrinkDecision {
    let refuse = |r: StaffDrinkRefusal| StaffDrinkDecision {
        allowed: false,
        refusal: Some(r),
        overspent: false,
        pool: pool_state(business_date, settings.daily_allowance, used),
    };

    if !settings.enabled {
        return refuse(StaffDrinkRefusal::PoolOff);
    }
    if settings.eligible_item_ids.is_empty() {
        return refuse(StaffDrinkRefusal::NoEligibleItems);
    }
    if !settings.eligible_item_ids.iter().any(|i| i == item_id) {
        return refuse(StaffDrinkRefusal::ItemNotEligible);
    }
    if !note_is_given(note) {
        return refuse(StaffDrinkRefusal::NoteRequired);
    }

    // This drink is the (used + 1)th of the day. It is over the allowance when
    // that ordinal exceeds it — so an allowance of 0 makes the FIRST drink an
    // overspend, and an allowance of 5 makes the sixth one. It still lands.
    let after = used.max(0) + 1;
    StaffDrinkDecision {
        allowed: true,
        refusal: None,
        overspent: after > settings.daily_allowance.max(0),
        pool: pool_state(business_date, settings.daily_allowance, after),
    }
}

/// The branch-local business date of an instant — the same boundary the Z
/// report and the backend's `service_day_bounds` use. A DST gap cannot move a
/// date, so the naive local date is exactly right here. (The till formats it
/// `YYYY-MM-DD`, which is `NaiveDate`'s `Display`.)
pub fn business_date_of(tz: chrono_tz::Tz, at: chrono::DateTime<chrono::Utc>) -> chrono::NaiveDate {
    use chrono::TimeZone as _;
    tz.from_utc_datetime(&at.naive_utc()).date_naive()
}

/// Moved from madar-core `staff_pool::tests`.
#[cfg(test)]
mod tests {
    use super::*;

    fn settings(enabled: bool, allowance: i32, items: &[&str]) -> StaffPoolSettings {
        StaffPoolSettings {
            enabled,
            daily_allowance: allowance,
            eligible_item_ids: items.iter().map(|s| s.to_string()).collect(),
        }
    }

    const D: &str = "2026-09-19";

    #[test]
    fn a_drink_inside_the_allowance_is_plain_allowed() {
        let d = decide(
            &settings(true, 5, &["latte"]),
            D,
            "latte",
            "for Sara, closing shift",
            2,
        );
        assert!(d.allowed && !d.overspent);
        assert_eq!(d.pool.used, 3);
        assert_eq!(d.pool.remaining, 2);
        assert_eq!(d.pool.over, 0);
    }

    #[test]
    fn the_drink_that_fills_the_allowance_is_still_not_an_overspend() {
        let d = decide(&settings(true, 5, &["latte"]), D, "latte", "n", 4);
        assert!(d.allowed && !d.overspent);
        assert_eq!(d.pool.remaining, 0);
        assert_eq!(d.pool.over, 0);
    }

    #[test]
    fn the_drink_past_the_allowance_lands_and_is_marked() {
        let d = decide(&settings(true, 5, &["latte"]), D, "latte", "n", 5);
        assert!(
            d.allowed,
            "an overspend is never refused — the drink was already made"
        );
        assert!(d.overspent);
        assert_eq!(d.pool.remaining, 0);
        assert_eq!(d.pool.over, 1);
    }

    #[test]
    fn an_allowance_of_zero_makes_every_drink_an_overspend_but_refuses_none() {
        let d = decide(&settings(true, 0, &["latte"]), D, "latte", "n", 0);
        assert!(d.allowed && d.overspent);
        assert_eq!(d.pool.over, 1);
    }

    #[test]
    fn a_note_is_required_and_whitespace_is_not_a_note() {
        for note in ["", "   ", "\t\n"] {
            let d = decide(&settings(true, 5, &["latte"]), D, "latte", note, 0);
            assert!(!d.allowed);
            assert_eq!(d.refusal, Some(StaffDrinkRefusal::NoteRequired));
        }
        assert!(decide(&settings(true, 5, &["latte"]), D, "latte", " x ", 0).allowed);
    }

    #[test]
    fn an_empty_eligible_list_switches_the_pool_off() {
        let d = decide(&settings(true, 5, &[]), D, "latte", "n", 0);
        assert!(!d.allowed);
        assert_eq!(d.refusal, Some(StaffDrinkRefusal::NoEligibleItems));
    }

    #[test]
    fn an_item_off_the_list_never_counts() {
        let d = decide(&settings(true, 5, &["latte"]), D, "cheesecake", "n", 0);
        assert_eq!(d.refusal, Some(StaffDrinkRefusal::ItemNotEligible));
    }

    #[test]
    fn the_master_switch_is_checked_before_anything_else() {
        // No items, no note, wrong item — the teller is still told the pool is off.
        let d = decide(&settings(false, 0, &[]), D, "nope", "", 0);
        assert_eq!(d.refusal, Some(StaffDrinkRefusal::PoolOff));
    }

    #[test]
    fn pool_state_never_reports_both_remaining_and_over() {
        for used in 0..12 {
            let p = pool_state(D, 5, used);
            assert!(p.remaining == 0 || p.over == 0);
            assert_eq!(p.remaining as i64 - p.over as i64, 5 - used as i64);
        }
    }

    #[test]
    fn the_business_day_turns_over_at_branch_midnight_not_utc() {
        use chrono::TimeZone as _;
        let cairo = chrono_tz::Africa::Cairo;
        // 22:30 UTC on the 19th is 01:30 on the 20th in Cairo (UTC+3): the
        // branch has already turned the page even though UTC has not.
        let late = chrono::Utc
            .with_ymd_and_hms(2026, 9, 19, 22, 30, 0)
            .unwrap();
        assert_eq!(business_date_of(cairo, late).to_string(), "2026-09-20");
        // 21:00 UTC is midnight exactly — the first instant of the new day.
        let midnight = chrono::Utc.with_ymd_and_hms(2026, 9, 19, 21, 0, 0).unwrap();
        assert_eq!(business_date_of(cairo, midnight).to_string(), "2026-09-20");
        // One second earlier is still the old day, and the pool has not reset.
        let before = chrono::Utc
            .with_ymd_and_hms(2026, 9, 19, 20, 59, 59)
            .unwrap();
        assert_eq!(business_date_of(cairo, before).to_string(), "2026-09-19");
    }

    #[test]
    fn a_reset_is_simply_a_new_date_with_no_use_on_it() {
        let s = settings(true, 3, &["latte"]);
        let spent = decide(&s, "2026-09-19", "latte", "n", 3);
        assert!(spent.overspent);
        let fresh = decide(&s, "2026-09-20", "latte", "n", 0);
        assert!(fresh.allowed && !fresh.overspent);
        assert_eq!(fresh.pool.remaining, 2);
    }
}

/// Moved from MadarRust `staff_pool::engine::tests` (the one that reads
/// `service_day_bounds` stays in the backend, beside that function).
#[cfg(test)]
mod server_tests {
    use super::*;

    fn settings(enabled: bool, allowance: i32, items: &[&str]) -> StaffPoolSettings {
        StaffPoolSettings {
            enabled,
            daily_allowance: allowance,
            eligible_item_ids: items.iter().map(|s| s.to_string()).collect(),
        }
    }

    const D: &str = "2026-09-19";

    #[test]
    fn an_overspend_lands_and_is_marked_never_refused() {
        let d = decide(&settings(true, 5, &["latte"]), D, "latte", "n", 5);
        assert!(d.allowed);
        assert!(d.overspent);
        assert_eq!(d.pool.over, 1);
        assert_eq!(d.pool.remaining, 0);
    }

    #[test]
    fn a_note_is_required_and_whitespace_is_not_a_note() {
        let s = settings(true, 5, &["latte"]);
        assert_eq!(
            decide(&s, D, "latte", "  ", 0).refusal,
            Some(StaffDrinkRefusal::NoteRequired)
        );
        assert!(decide(&s, D, "latte", "for Sara", 0).allowed);
    }

    #[test]
    fn an_empty_eligible_list_is_a_pool_that_is_off() {
        assert_eq!(
            decide(&settings(true, 5, &[]), D, "latte", "n", 0).refusal,
            Some(StaffDrinkRefusal::NoEligibleItems)
        );
    }

    #[test]
    fn the_business_day_turns_over_at_branch_midnight_not_utc() {
        use chrono::TimeZone as _;
        let cairo = chrono_tz::Africa::Cairo;
        let late = chrono::Utc
            .with_ymd_and_hms(2026, 9, 19, 22, 30, 0)
            .unwrap();
        assert_eq!(business_date_of(cairo, late).to_string(), "2026-09-20");
        let before = chrono::Utc
            .with_ymd_and_hms(2026, 9, 19, 20, 59, 59)
            .unwrap();
        assert_eq!(business_date_of(cairo, before).to_string(), "2026-09-19");
    }
}
