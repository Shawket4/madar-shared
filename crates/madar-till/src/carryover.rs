//! The drawer carryover: the declared close the next opening is compared with.
//!
//! The server picks it in SQL (MadarRust `tills::handlers::
//! last_close_declared`: `ORDER BY COALESCE(device_id = $2, false) DESC,
//! opened_at DESC LIMIT 1` over the branch's closed tills that declared a
//! count); the till picks it from its rows offline (madar-core
//! `till::last_close_declared_rows`, which calls this). A drawer is a physical
//! box, identified by DEVICE where one is known and by the branch otherwise —
//! never by the person, because cash stays in the drawer when a shift changes.
//! Since fix T2 (2026-09-23) a device-less close no longer beats this device's
//! own (Postgres sorted the NULL key first).
//!
//! Pinned by `vectors/carryover_vectors.json`, which the backend also runs
//! through its SQL.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

/// One till of the branch, as the picker needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedTill {
    pub status: String,
    pub device_id: Option<String>,
    /// RFC 3339.
    pub opened_at: String,
    pub closing_cash_declared: Option<i64>,
}

fn opened(t: &ClosedTill) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(&t.opened_at).ok()
}

/// The drawer's most recent declared close: this device's own last close
/// wins; otherwise the branch's. `rows` must be one branch's.
pub fn last_close_declared(rows: &[ClosedTill], device_id: Option<&str>) -> Option<i64> {
    let closed = || {
        rows.iter()
            .filter(|t| matches!(t.status.as_str(), "closed" | "force_closed"))
            .filter(|t| t.closing_cash_declared.is_some())
    };
    device_id
        .and_then(|dev| {
            closed()
                .filter(|t| t.device_id.as_deref() == Some(dev))
                .max_by_key(|t| opened(t))
        })
        .or_else(|| closed().max_by_key(|t| opened(t)))
        .and_then(|t| t.closing_cash_declared)
}

pub mod vectors {
    //! Carryover cases. Regenerate deliberately:
    //! `MADAR_REGENERATE_CARRYOVER_VECTORS=1 cargo test -p madar-till carryover_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{last_close_declared, ClosedTill};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct CarryoverVector {
        pub name: String,
        pub tills: Vec<ClosedTill>,
        pub device_id: Option<String>,
        pub expected: Option<i64>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/carryover_vectors.json")
    }

    pub const DEV_A: &str = "00000000-0000-4000-8000-00000000000a";
    pub const DEV_B: &str = "00000000-0000-4000-8000-00000000000b";

    fn till(status: &str, device: Option<&str>, opened: &str, declared: Option<i64>) -> ClosedTill {
        ClosedTill {
            status: status.into(),
            device_id: device.map(str::to_string),
            opened_at: opened.into(),
            closing_cash_declared: declared,
        }
    }

    pub fn generate() -> Vec<CarryoverVector> {
        let d1 = "2026-09-01T08:00:00+00:00";
        let d2 = "2026-09-05T08:00:00+00:00";
        let d3 = "2026-09-10T08:00:00+00:00";
        let cases: Vec<(&str, Vec<ClosedTill>, Option<&str>)> = vec![
            ("no tills", vec![], Some(DEV_A)),
            (
                "T2: this device's own close beats an older device-less one",
                vec![
                    till("closed", None, d1, Some(500)),
                    till("closed", Some(DEV_A), d3, Some(900)),
                ],
                Some(DEV_A),
            ),
            (
                "T2 reversed: this device's own close beats a NEWER device-less one",
                vec![
                    till("closed", None, d3, Some(500)),
                    till("closed", Some(DEV_A), d1, Some(900)),
                ],
                Some(DEV_A),
            ),
            (
                "no device known: the branch's newest close",
                vec![
                    till("closed", None, d1, Some(500)),
                    till("closed", Some(DEV_A), d3, Some(900)),
                ],
                None,
            ),
            (
                "another device's close only: the branch's newest",
                vec![
                    till("closed", Some(DEV_B), d2, Some(700)),
                    till("closed", None, d1, Some(300)),
                ],
                Some(DEV_A),
            ),
            (
                "an open till and an undeclared close are not carryovers",
                vec![
                    till("open", Some(DEV_A), d3, Some(111)),
                    till("closed", Some(DEV_A), d2, None),
                    till("force_closed", Some(DEV_A), d1, Some(222)),
                ],
                Some(DEV_A),
            ),
            (
                "a force-closed till counts",
                vec![
                    till("force_closed", None, d3, Some(0)),
                    till("closed", None, d1, Some(50)),
                ],
                None,
            ),
            (
                "two devices side by side keep their own drawers",
                vec![
                    till("closed", Some(DEV_A), d1, Some(1000)),
                    till("closed", Some(DEV_B), d3, Some(2000)),
                    till("closed", Some(DEV_A), d2, Some(1500)),
                ],
                Some(DEV_A),
            ),
            (
                "an unknown device falls back to the branch",
                vec![
                    till("closed", Some(DEV_A), d1, Some(1000)),
                    till("closed", Some(DEV_B), d3, Some(2000)),
                ],
                Some("00000000-0000-4000-8000-0000000000cc"),
            ),
        ];
        cases
            .into_iter()
            .map(|(name, tills, dev)| CarryoverVector {
                name: name.into(),
                expected: last_close_declared(&tills, dev),
                tills,
                device_id: dev.map(str::to_string),
            })
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn carryover_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_CARRYOVER_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vec<CarryoverVector> =
                serde_json::from_str(crate::vectors::CARRYOVER).unwrap();
            assert_eq!(generated, expected, "the carryover picker drifted");
            let t2 = &generated[1];
            assert_eq!(t2.expected, Some(900), "T2: the server's fixed rule");
        }
    }
}
