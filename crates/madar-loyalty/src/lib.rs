//! # madar-loyalty
//!
//! Which rewards a sale may take, in ONE copy: the redemption planner the
//! backend (MadarRust `loyalty::redeem`) and the POS core (madar-core
//! `loyalty::reward_board`) both run.
//!
//! A reward covers whole units of one line. Which lines may carry one, how
//! many units, what they cost in the member's currency, the shop's per-order
//! ceiling and the balance are the same questions on both sides; only the
//! answer to "no" differs:
//!
//! - the SERVER is strict ([`Mode::Server`]): a sale asking for more than it
//!   may take is refused ([`Refusal`]) — nothing has been handed over yet;
//! - the TILL trims ([`Mode::Till`]): it keeps what may be taken and names the
//!   first thing it took off ([`Trim`]), so the teller hears it before Charge
//!   and the sale it sends is one the server will not refuse.
//!
//! Before this crate the two were written separately (discovery M12): a till
//! that under-trimmed while offline sent a sale the server's lenient replay
//! then honoured with no points debited.
//!
//! [`replay_lines`] is the lines a REPLAYED sale covered when the strict plan
//! refused it: the sale already happened, so the cover stands and no points
//! move.
//!
//! What a reward takes off a line in money is `madar_money::loyalty::
//! covered_minor`; this crate only decides units and cost. Nothing here does
//! I/O. Pinned by `vectors/loyalty_plan_vectors.json`.

pub mod plan;
pub mod vectors;

pub use plan::{
    plan, replay_lines, unit_cost, Ask, Line, Mode, Plan, Planned, Programme, Refusal, Reward, Trim,
};
