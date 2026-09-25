//! # madar-sync
//!
//! Pieces of the `/sync/pull` contract the backend (MadarRust) and the Rust
//! cores (madar-core) must agree on byte for byte, in ONE copy:
//!
//! - the wire type lists and the ledger classification ([`ALL_TYPES`],
//!   [`LEDGER_TYPES`], [`REQUIRED_TYPES`], [`LEDGER_WINDOW_HOURS`],
//!   [`is_ledger`]) — the core once left `staff_drink` out of its ledger
//!   types and deleted drinks a snapshot page did not carry;
//! - the R-checksum ([`checksum_of`]), pinned by
//!   `vectors/sync_checksum_vector.json`;
//! - the deterministic kitchen ids ([`kitchen`]) an offline device predicts;
//! - the `/sync/replay` envelopes ([`replay`]) and the current release's
//!   fixture ([`vectors::REPLAY_CURRENT`]).

pub mod kitchen;
pub mod replay;

/// Every wire type the server serves and a POS syncs (a LAN peer may hand
/// over any of them too). The order is the server's response order.
///
/// `bundle` is a stub: combos were removed on 2026-09-25 and the server
/// answers it with an empty set, because tills from before ask for it (and
/// v0.8 counts a full snapshot complete only when it is answered). It leaves
/// once no such till is in the field.
pub const ALL_TYPES: &[&str] = &[
    "category",
    "menu_item",
    "bundle",
    "ingredient",
    "payment_method",
    "payment_availability",
    "discount",
    "branch_settings",
    "device",
    "teller",
    "floor_section",
    "floor_table",
    "table_occupancy",
    "table_transfer",
    "open_ticket",
    "kitchen_ticket",
    "delivery",
    "booking",
    "till",
    "cash_movement",
    "order",
    "refund",
    "addon_item",
    "customer",
    "staff_drink",
];

/// The types a snapshot must list to count as COMPLETE (and move the
/// device's cursor): the contract's original set. A type added later
/// (`addon_item`, `customer`, `staff_drink`) is applied when a server sends
/// it, but a server that predates it still completes — a till never waits on
/// the customer list to open. `bundle` left this list with combos
/// (2026-09-25): a till no longer waits on it either.
pub const REQUIRED_TYPES: &[&str] = &[
    "category",
    "menu_item",
    "ingredient",
    "payment_method",
    "payment_availability",
    "discount",
    "branch_settings",
    "device",
    "teller",
    "floor_section",
    "floor_table",
    "table_occupancy",
    "table_transfer",
    "open_ticket",
    "kitchen_ticket",
    "delivery",
    "booking",
    "till",
    "cash_movement",
    "order",
    "refund",
];

/// Ledger types: never checksummed; windowed in full snapshots and paged with
/// each other, so a snapshot page missing a row says nothing about it.
///
/// `staff_drink` belongs here rather than among the state types: the rows are
/// dated and grow forever, and a till only ever needs the business day it is
/// working. The 48-hour window covers today and yesterday, which spans the
/// business-day boundary in any timezone.
pub const LEDGER_TYPES: &[&str] = &["till", "cash_movement", "order", "refund", "staff_drink"];

/// How far back a full snapshot's ledger window reaches.
pub const LEDGER_WINDOW_HOURS: i64 = 48;

/// Whether a wire type is a ledger type ([`LEDGER_TYPES`]).
pub fn is_ledger(ty: &str) -> bool {
    LEDGER_TYPES.contains(&ty)
}

/// The R-checksum of a state type: the first 16 hex of sha256 over the sorted
/// `"<entity_id>:<seq>"` lines joined by `\n`.
pub fn checksum_of(rows: &[(String, i64)]) -> String {
    use sha2::{Digest, Sha256};
    let mut lines: Vec<String> = rows.iter().map(|(id, seq)| format!("{id}:{seq}")).collect();
    lines.sort();
    let digest = Sha256::digest(lines.join("\n").as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The vector files this crate is pinned by, as bytes.
pub mod vectors {
    /// `checksum_of` over four rows.
    pub const SYNC_CHECKSUM: &str = include_str!("../vectors/sync_checksum_vector.json");
    /// One `/sync/replay` envelope per op, as the CURRENT till writes them
    /// (madar-core `replay_fixture`, `MADAR_WRITE_REPLAY_FIXTURE=1`).
    pub const REPLAY_CURRENT: &str = include_str!("../vectors/replay_current.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Moved from MadarRust `sync::pull::checksum::tests` (madar-core ran the
    /// same assertion as `sync_pull::tests::checksum_formula_matches_backend_vector`).
    #[test]
    fn checksum_formula_matches_pos_vector() {
        let v: serde_json::Value = serde_json::from_str(vectors::SYNC_CHECKSUM).unwrap();
        let rows: Vec<(String, i64)> = v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["id"].as_str().unwrap().to_string(),
                    r["seq"].as_i64().unwrap(),
                )
            })
            .collect();
        assert_eq!(checksum_of(&rows), v["checksum"].as_str().unwrap());
    }

    #[test]
    fn the_ledger_types_are_wire_types_and_staff_drink_is_one() {
        for t in LEDGER_TYPES {
            assert!(ALL_TYPES.contains(t), "{t}");
            assert!(is_ledger(t));
        }
        assert!(is_ledger("staff_drink"));
        assert!(!is_ledger("customer"));
        for t in REQUIRED_TYPES {
            assert!(ALL_TYPES.contains(t), "{t}");
        }
        let mut all = ALL_TYPES.to_vec();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), ALL_TYPES.len(), "no type listed twice");
    }
}
