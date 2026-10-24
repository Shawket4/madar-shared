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
//!
//! [`tracks`] plans a basket across a card's stamp tracks (up to four, each
//! its own balance, one shared per-order ceiling); a sale on track 1 alone is
//! [`plan`] itself. Pinned by `vectors/loyalty_tracks_vectors.json`.
//!
//! [`gift`] is which line may take the birthday gift (one unit, no balance):
//! `plan_gift` and `gift_eligible`, the server's claim check and the till's
//! board. Pinned by `vectors/loyalty_gift_vectors.json`.
//!
//! [`card`] is what a member's card shows for a balance: rewards earned,
//! progress to the next, what it still needs, the stamps (the server's
//! `loyalty::model` rule). Pinned by `vectors/loyalty_card_vectors.json`.

pub mod card;
pub mod gift;
pub mod plan;
pub mod tracks;
pub mod vectors;

pub use gift::{gift_eligible, plan_gift, GiftCover, GiftProgramme, GiftRefusal};
pub use plan::{
    plan, replay_lines, unit_cost, Ask, Line, Mode, Plan, Planned, Programme, Refusal, Reward, Trim,
};
