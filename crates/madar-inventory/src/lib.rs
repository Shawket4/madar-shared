//! # madar-inventory
//!
//! Warehouse and stock-transfer rules the backend (MadarRust `inventory/`)
//! and any client that drives transfers must agree on, in ONE copy
//! (`WAREHOUSE_DESIGN.md` in MadarRust):
//!
//! - [`transfer`]: the transfer lifecycle — which side may take which action
//!   in which status, the capability it needs, and the status it leads to;
//!   and the receive check for one line (short, exact, over; an over-receive
//!   needs a note);
//! - [`replenish`]: how much a warehouse should send a branch;
//! - [`purchase`]: purchase-order money — a line's cost and unit cost, what a
//!   delivery cost (pro rata), the order dialog's estimate;
//! - [`count`]: which stock-count row needs a variance reason;
//! - [`api`]: the request and response bodies of the transfer and
//!   replenishment endpoints (OpenAPI schemas behind the `utoipa` feature).
//!
//! Quantities travel as JSON numbers in the ingredient's base stock unit and
//! are stored `numeric(12,3)`. Every comparison here is made in whole
//! thousandths ([`milli`]) so float noise never decides a refusal.
//! Pinned by `vectors/inventory_vectors.json`; purchases by
//! `vectors/purchase_vectors.json`, counts by `vectors/count_vectors.json`.

pub mod api;
pub mod count;
pub mod purchase;
pub mod replenish;
pub mod transfer;
pub mod vectors;

/// A quantity in whole thousandths of its unit (the `numeric(12,3)` grain).
pub fn milli(q: f64) -> i64 {
    (q * 1000.0).round() as i64
}

/// Back from thousandths to a quantity.
pub fn from_milli(m: i64) -> f64 {
    m as f64 / 1000.0
}
