//! # madar-money
//!
//! Money rules the backend (MadarRust) and the Rust cores (madar-core) both
//! run, in ONE copy. v1 is a zero-behaviour move: only code that was already
//! identical on both sides and pinned by vectors.
//!
//! - [`tax`]: the bill engine (`compute`, `discount_amount`), the sale-channel
//!   rule (`SaleChannel`, `TaxPolicy::for_sale`), `negative_part`, `is_sane`,
//!   and the refund tax/service split (`refund_split`).
//! - [`staff_pool`]: the staff drinks pool decision.
//! - [`staff_comp`]: what a staff drink is given free (the rule, not the
//!   input builders).
//! - [`loyalty`]: what a reward covers on a line.
//! - [`metrics`]: POS metrics `average_ticket` and the report's constants.
//! - [`line`] (v2): what a sale line comes to.
//! - [`bill`] (v2): bill assembly (staff comp → reward → discount → tax), a
//!   table bill's preview, and the tender / change / split rules.
//! - [`discount`] (v2): the discount act a sale asks for, its capability and
//!   figures (`ask_from`, basis points, `figures`).
//! - [`waste`] (v2): what a waste is worth and which inputs may be recorded.
//! - [`alloc`] (v0.5): the one allocator — an amount over weights, the
//!   shares summing to it exactly (a combo's price over its parts, a deal's
//!   discount over a chunk's units).
//! - [`vectors`]: the vector files, for consumer tests that pin their own
//!   code (SQL, bill assembly) to the same bytes.
//!
//! Nothing here does I/O or reads a clock. Callers pass everything in.

pub mod alloc;
pub mod bill;
pub mod discount;
pub mod line;
pub mod loyalty;
pub mod metrics;
pub mod staff_comp;
pub mod staff_pool;
pub mod tax;
pub mod vectors;
pub mod waste;
