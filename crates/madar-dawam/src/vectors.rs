//! Dawam vectors. Regenerate deliberately:
//! `MADAR_REGENERATE_DAWAM_VECTORS=1 cargo test -p madar-dawam dawam_vectors`.

use std::path::PathBuf;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{geofence, pay, stamp};

/// The file, for consumer tests.
pub const DAWAM: &str = include_str!("../vectors/dawam_vectors.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FenceVector {
    pub branch: (f64, f64),
    pub phone: (f64, f64),
    pub geo_radius_meters: Option<i64>,
    pub distance_m: f64,
    pub radius_m: i64,
    pub inside: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeriodVector {
    pub day: NaiveDate,
    pub start_day: i64,
    pub start: NaiveDate,
    pub end: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorVector {
    pub anchor: String,
    /// The server time it carries, `null` when it is not an anchor.
    pub ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vectors {
    pub fences: Vec<FenceVector>,
    pub periods: Vec<PeriodVector>,
    pub anchors: Vec<AnchorVector>,
    /// Stamps as the phone sends them; each decodes as [`stamp::OfflineStamp`].
    pub stamps: Vec<serde_json::Value>,
}

pub fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/dawam_vectors.json")
}

pub fn generate() -> Vectors {
    let zamalek = (30.02047, 31.004112);
    let mut fences = Vec::new();
    for dm in [0.0, 1.0, 150.0, 199.9, 200.0, 200.1, 350.0, 5000.0] {
        let phone = (zamalek.0 + dm / 111_195.0, zamalek.1);
        for radius in [None, Some(0), Some(-5), Some(150), Some(200), Some(350)] {
            let d = geofence::haversine_m(zamalek, phone);
            let r = geofence::effective_radius(radius);
            fences.push(FenceVector {
                branch: zamalek,
                phone,
                geo_radius_meters: radius,
                distance_m: d,
                radius_m: r,
                inside: geofence::inside(d, r),
            });
        }
    }
    // Far apart, across the date line, and the poles.
    for (a, b) in [
        ((0.0, 0.0), (1.0, 0.0)),
        ((30.0444, 31.2357), (31.2001, 29.9187)),
        ((0.0, 179.9), (0.0, -179.9)),
        ((90.0, 0.0), (-90.0, 0.0)),
    ] {
        let d = geofence::haversine_m(a, b);
        fences.push(FenceVector {
            branch: a,
            phone: b,
            geo_radius_meters: None,
            distance_m: d,
            radius_m: 200,
            inside: geofence::inside(d, 200),
        });
    }
    let mut periods = Vec::new();
    let days = [
        "2026-01-01",
        "2026-01-25",
        "2026-01-26",
        "2026-01-31",
        "2026-02-28",
        "2026-03-01",
        "2026-03-25",
        "2026-03-26",
        "2026-12-31",
        "2028-02-29",
    ];
    for day in days {
        let day: NaiveDate = day.parse().unwrap();
        for start_day in [-3, 0, 1, 15, 26, 28, 29, 31] {
            let (start, end) = pay::period_window(day, start_day);
            periods.push(PeriodVector {
                day,
                start_day,
                start,
                end,
            });
        }
    }
    let hex = "ab".repeat(32);
    let anchors = [
        format!("v1.1758700000000.{hex}"),
        format!("  v1.1758700000000.{hex}\n"),
        format!("v2.1758700000000.{hex}"),
        format!("v1.x.{hex}"),
        format!("v1.1758700000000.{}", "ab".repeat(31)),
        format!("v1.1758700000000.{}zz", "ab".repeat(31)),
        format!("v1.-5.{hex}"),
        "v1.1758700000000".to_string(),
        String::new(),
    ]
    .into_iter()
    .map(|a| AnchorVector {
        ms: stamp::parse_anchor(&a).map(|x| x.ms),
        anchor: a,
    })
    .collect();
    let stamps = vec![
        serde_json::json!({ "server_time": "2026-09-22T08:00:00+00:00", "elapsed_ms": 1800000, "rebooted": false,
                            "gps_time": "2026-09-22T08:31:00Z", "anchor": format!("v1.1758528000000.{hex}") }),
        serde_json::json!({ "server_time": "2026-09-22T08:00:00.123+00:00", "elapsed_ms": 0, "rebooted": true, "gps_time": null }),
        serde_json::json!({ "server_time": "2026-09-22T08:00:00Z", "elapsed_ms": 5 }),
    ];
    Vectors {
        fences,
        periods,
        anchors,
        stamps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dawam_vectors() {
        let generated = generate();
        if std::env::var("MADAR_REGENERATE_DAWAM_VECTORS").is_ok() {
            std::fs::write(
                fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let expected: Vectors = serde_json::from_str(DAWAM).unwrap();
        // JSON floats parse back to within an ulp or so (serde_json without
        // `float_roundtrip`): the distances are compared to a micrometre, and
        // everything else exactly.
        assert_eq!(generated.fences.len(), expected.fences.len());
        for (g, e) in generated.fences.iter().zip(&expected.fences) {
            assert!((g.distance_m - e.distance_m).abs() < 1e-6, "{g:?} vs {e:?}");
            assert_eq!(
                (g.geo_radius_meters, g.radius_m, g.inside),
                (e.geo_radius_meters, e.radius_m, e.inside),
                "{g:?}"
            );
        }
        assert_eq!(
            generated.periods, expected.periods,
            "the pay period drifted"
        );
        assert_eq!(
            generated.anchors, expected.anchors,
            "the anchor format drifted"
        );
        assert_eq!(generated.stamps, expected.stamps);
        for s in &expected.stamps {
            serde_json::from_value::<stamp::OfflineStamp>(s.clone()).expect("a stamp decodes");
        }
    }

    #[test]
    fn dw2_a_zero_radius_is_zero() {
        assert_eq!(geofence::effective_radius(Some(0)), 0);
        assert_eq!(geofence::effective_radius(None), 200);
        let d = geofence::haversine_m(
            (30.02047, 31.004112),
            (30.02047 + 150.0 / 111_195.0, 31.004112),
        );
        assert!(!geofence::inside(d, geofence::effective_radius(Some(0))));
    }

    #[test]
    fn an_anchor_round_trips() {
        let tag = [0xab; 32];
        let a = stamp::format_anchor(42, &tag);
        assert_eq!(stamp::parse_anchor(&a), Some(stamp::Anchor { ms: 42, tag }));
    }
}
