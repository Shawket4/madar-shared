//! Loyalty rules both sides run.
//!
//! v1 holds the reward COVER only: what one reward takes off a line. Which
//! picks a sale may redeem (the server's `redeem::plan`, the till's
//! `loyalty.rs`) is still decided on each side.

pub mod vectors;

/// Minor units a reward takes off one line: whole units at the price the line
/// was charged per unit, modifiers included, never more than the line itself.
///
/// Moved from MadarRust `loyalty::redeem::covered_minor`. madar-core's
/// `loyalty::covered_minor(line_total, qty, units)` is this with
/// `charged_per_unit = line_total / qty`. Pinned by
/// `vectors/loyalty_reward_vectors.json`.
pub fn covered_minor(charged_per_unit: i64, line_subtotal: i64, units: i64) -> i64 {
    (charged_per_unit.max(0) * units.max(0)).min(line_subtotal.max(0))
}
