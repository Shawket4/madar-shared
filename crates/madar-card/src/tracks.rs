//! Stamp tracks: one card, up to four rewards.
//!
//! A shop that rewards coffee and sandwiches separately gives each its own row
//! of stamps on the SAME card. Track 1 is the programme's own counter — the
//! balance every released till and pass already reads (`visits_balance`), its
//! size the programme's card size — so a card with one track is exactly the
//! card it always was. Tracks 2–4 are stamped and completed at the counter,
//! each with its own size, name, stamp and colour.
//!
//! Points programmes have one track: points are money spent, and a second
//! "points for sandwiches" balance would need a second bill.

use serde::{Deserialize, Serialize};

use crate::design::StampShape;
use crate::validate::Problem;

/// At most this many tracks on one card (the strip's rows and the wallets'
/// field slots both run out past four).
pub const MAX_TRACKS: usize = 4;
/// The largest card a track 2–4 may have, as the programme's own card size.
pub const MAX_TRACK_SIZE: i32 = 50;
/// A track's name is a field label on the pass: short or the OS cuts it.
pub const NAME_MAX: usize = 24;

/// What a programme collects. One track only for points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Points,
    Visits,
}

/// A track as the owner defines it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Track {
    /// 1 to 4, in order. Track 1 is the programme's own counter.
    pub position: u8,
    /// Shown on the pass and the teller page; required from track 2 on.
    #[serde(default)]
    pub name_en: String,
    #[serde(default)]
    pub name_ar: String,
    /// Stamps that fill this track's card. `None` on track 1, whose size is
    /// the programme's card size (one number, one place).
    #[serde(default)]
    pub size: Option<i32>,
    /// This track's stamp, else the design's.
    #[serde(default)]
    pub shape: Option<StampShape>,
    /// This track's colour (`#RRGGBB`), else the pass's foreground.
    #[serde(default)]
    pub colour: Option<String>,
    /// Tracks 2–4: the menu items (ids) a sale earns on and a completed card
    /// covers. Empty = a counter-only track. Always empty on track 1, whose
    /// items are the programme's earning list and reward catalogue.
    #[serde(default)]
    pub items: Vec<String>,
    /// Tracks 2–4: a stamp per unit of its items (default), or one per sale
    /// holding any. Track 1 follows the programme's own setting.
    #[serde(default = "yes")]
    pub per_item: bool,
    /// Tracks 2–4: the balance ceiling (a stamp past it is trimmed); at least
    /// the track's size. `None` = no ceiling.
    #[serde(default)]
    pub cap: Option<i32>,
}

/// What rendering and validation need of a track: where it is and how big a
/// card it fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TrackRef {
    pub position: u8,
    pub size: i32,
}

/// The most menu items one track may be bound to.
pub const MAX_TRACK_ITEMS: usize = 200;

fn yes() -> bool {
    true
}

/// A `#RRGGBB` colour.
pub fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// The problems with a set of tracks for a programme of `mode`; empty when it
/// may be saved.
pub fn validate_tracks(tracks: &[Track], mode: Mode) -> Vec<Problem> {
    let mut out = Vec::new();
    if tracks.is_empty() || tracks.len() > MAX_TRACKS {
        out.push(Problem::error("tracks_count", "tracks"));
        return out;
    }
    if mode == Mode::Points && tracks.len() > 1 {
        out.push(Problem::error("tracks_points_single", "tracks"));
    }
    for (i, t) in tracks.iter().enumerate() {
        let path = format!("tracks[{i}]");
        if usize::from(t.position) != i + 1 {
            out.push(Problem::error("track_position", &path));
        }
        if t.position == 1 {
            if t.size.is_some() {
                out.push(Problem::error("track_one_size", &path));
            }
            if !t.items.is_empty() || t.cap.is_some() {
                out.push(Problem::error("track_one_items", &path));
            }
        } else {
            if t.items.len() > MAX_TRACK_ITEMS {
                out.push(Problem::error("track_items_many", &path));
            }
            let mut seen: Vec<&str> = Vec::new();
            for i in &t.items {
                if i.trim().is_empty() || seen.contains(&i.as_str()) {
                    out.push(Problem::error("track_items_duplicate", &path));
                    break;
                }
                seen.push(i);
            }
            if let (Some(c), Some(n)) = (t.cap, t.size) {
                if c < n {
                    out.push(Problem::error("track_cap", &path));
                }
            }
            match t.size {
                Some(n) if (1..=MAX_TRACK_SIZE).contains(&n) => {}
                _ => out.push(Problem::error("track_size", &path)),
            }
            if t.name_en.trim().is_empty() && t.name_ar.trim().is_empty() {
                out.push(Problem::error("track_name_missing", &path));
            }
        }
        if t.name_en.chars().count() > NAME_MAX || t.name_ar.chars().count() > NAME_MAX {
            out.push(Problem::error("track_name_long", &path));
        }
        if t.colour.as_deref().is_some_and(|c| !is_hex(c)) {
            out.push(Problem::error("track_colour", &path));
        }
    }
    out
}

/// The card one track shows for its balance: madar-loyalty's rule, per track.
pub fn track_card(balance: i32, size: i32) -> madar_loyalty::card::Card {
    madar_loyalty::card::card(balance, size)
}

/// Whether a track's card is full (it may be completed at the counter).
pub fn can_complete(balance: i32, size: i32) -> bool {
    size > 0 && balance >= size
}

/// How many of `add` stamps a track takes under a balance ceiling (`None` =
/// no ceiling): a stamp past the ceiling is trimmed, never refused, as on
/// track 1.
pub fn stamps_within_cap(balance: i32, add: i32, cap: Option<i32>) -> i32 {
    let add = add.max(0);
    match cap {
        Some(c) => add.min((c - balance).max(0)),
        None => add,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(position: u8, size: Option<i32>, name: &str) -> Track {
        Track {
            position,
            name_en: name.into(),
            name_ar: String::new(),
            size,
            shape: None,
            colour: None,
            items: Vec::new(),
            per_item: true,
            cap: None,
        }
    }

    #[test]
    fn one_track_is_todays_card() {
        assert!(validate_tracks(&[t(1, None, "")], Mode::Visits).is_empty());
        assert!(validate_tracks(&[t(1, None, "")], Mode::Points).is_empty());
    }

    #[test]
    fn refusals() {
        let codes = |v: Vec<Problem>| v.into_iter().map(|p| p.code).collect::<Vec<_>>();
        assert_eq!(codes(validate_tracks(&[], Mode::Visits)), ["tracks_count"]);
        assert_eq!(
            codes(validate_tracks(
                &[t(1, None, ""), t(2, Some(6), "Tea")],
                Mode::Points
            )),
            ["tracks_points_single"]
        );
        assert_eq!(
            codes(validate_tracks(&[t(1, Some(5), "")], Mode::Visits)),
            ["track_one_size"]
        );
        assert_eq!(
            codes(validate_tracks(
                &[t(1, None, ""), t(3, Some(6), "Tea")],
                Mode::Visits
            )),
            ["track_position"]
        );
        assert_eq!(
            codes(validate_tracks(
                &[t(1, None, ""), t(2, Some(51), "Tea")],
                Mode::Visits
            )),
            ["track_size"]
        );
        assert_eq!(
            codes(validate_tracks(
                &[t(1, None, ""), t(2, Some(6), " ")],
                Mode::Visits
            )),
            ["track_name_missing"]
        );
    }

    #[test]
    fn the_cap_trims() {
        assert_eq!(stamps_within_cap(8, 3, Some(10)), 2);
        assert_eq!(stamps_within_cap(10, 3, Some(10)), 0);
        assert_eq!(stamps_within_cap(10, 3, None), 3);
        assert!(can_complete(6, 6));
        assert!(!can_complete(5, 6));
        assert!(!can_complete(5, 0));
    }
}
