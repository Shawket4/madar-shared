//! # madar-till
//!
//! A till's money as the backend (MadarRust) and the POS core (madar-core)
//! both compute it, in ONE copy:
//!
//! - [`report`]: the drawer (`system_cash`) and Z report ([`report::fold`]),
//!   the payment summary, the movement buckets, the tip and leg cash rules,
//!   the close preview's per-method totals ([`report::close_methods`]) and the
//!   refund split of queued refunds;
//! - [`carryover`]: the drawer's last declared close (the T2 rule);
//! - [`reconcile`]: close reconciliation ([`reconcile::plan_lines`],
//!   [`reconcile::rollup_status`]) and its error codes.
//!
//! The server still computes the report in SQL; it is pinned to this fold by
//! `vectors/till_report_vectors.json` (the backend's own scenarios, the rows
//! as `/sync/pull` projects them, and what the backend computed) and
//! `vectors/till_edge_vectors.json`. The core loads its rows from SQLite and
//! calls the fold.
//!
//! Nothing here does I/O or reads a clock.

pub mod carryover;
pub mod reconcile;
pub mod report;
pub mod vectors;
