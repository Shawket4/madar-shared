//! The stock-transfer lifecycle (WAREHOUSE_DESIGN.md §5).
//!
//! ```text
//! requested ──accept──▶ draft ──dispatch──▶ dispatched ──receive──▶ received
//!     │                   │                     │
//!     └─decline/cancel──▶ cancelled ◀──cancel───┴ (a dispatched cancel returns the stock)
//! ```
//!
//! A transfer starts as a `requested` draft (the destination asks) or as a
//! plain `draft` (the source sends without being asked). Stock moves twice:
//! out of the source at dispatch, into the destination at receive. A received
//! transfer is final; a mistake is fixed by a transfer the other way.

use madar_authz::Cap;
use serde::{Deserialize, Serialize};

use crate::milli;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    /// The destination asked for stock; the source has not answered.
    Requested,
    /// The source is preparing it. Nothing has moved.
    Draft,
    /// Left the source; in transit. Not on hand anywhere.
    Dispatched,
    /// Landed at the destination. Final.
    Received,
    /// Stopped. Final.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Change lines or quantities (the side that owns the current step).
    Edit,
    /// The source turns a request into its draft.
    Accept,
    /// The source refuses a request.
    Decline,
    /// Stock leaves the source.
    Dispatch,
    /// Stock lands at the destination.
    Receive,
    /// Stop it. After dispatch this returns the stock to the source.
    Cancel,
}

/// Whose location the caller must work at to take an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Source,
    Destination,
}

/// What an allowed action needs and leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub side: Side,
    pub cap: Cap,
    pub next: TransferStatus,
}

/// The one table. `None` = the action is not open in this status (409).
///
/// Capabilities: everything that moves nothing or answers a request is
/// `inventory.transfers.create` (a branch manager holds it); receiving is
/// `.edit` ("Receive or edit stock transfers"); only cancelling stock that is
/// already in transit is `.delete` (owner by default), because it moves stock.
pub fn step(status: TransferStatus, action: Action) -> Option<Step> {
    use Action as A;
    use Side::*;
    use TransferStatus as S;
    let (side, cap, next) = match (status, action) {
        (S::Requested, A::Edit) => (Destination, Cap::InventoryTransfersCreate, S::Requested),
        (S::Requested, A::Cancel) => (Destination, Cap::InventoryTransfersCreate, S::Cancelled),
        (S::Requested, A::Accept) => (Source, Cap::InventoryTransfersCreate, S::Draft),
        (S::Requested, A::Decline) => (Source, Cap::InventoryTransfersCreate, S::Cancelled),
        (S::Draft, A::Edit) => (Source, Cap::InventoryTransfersCreate, S::Draft),
        (S::Draft, A::Dispatch) => (Source, Cap::InventoryTransfersCreate, S::Dispatched),
        (S::Draft, A::Cancel) => (Source, Cap::InventoryTransfersCreate, S::Cancelled),
        (S::Dispatched, A::Receive) => (Destination, Cap::InventoryTransfersEdit, S::Received),
        (S::Dispatched, A::Cancel) => (Source, Cap::InventoryTransfersDelete, S::Cancelled),
        _ => return None,
    };
    Some(Step { side, cap, next })
}

/// How one line arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[serde(rename_all = "snake_case")]
pub enum Arrival {
    Exact,
    /// Less than sent: the gap is a transit loss.
    Short,
    /// More than sent: allowed only with a note.
    Over,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[serde(rename_all = "snake_case")]
pub enum ReceiveRefusal {
    /// `qty_received` below zero.
    Negative,
    /// More than sent with no note saying why.
    OverNeedsNote,
}

/// A received line, judged. `difference` = received − sent (negative when
/// short), in the unit, to the thousandth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct LineCheck {
    pub arrival: Arrival,
    pub difference: f64,
}

/// Judge one line at receive. A note of only whitespace is no note.
pub fn check_receive_line(
    qty_sent: f64,
    qty_received: f64,
    note: Option<&str>,
) -> Result<LineCheck, ReceiveRefusal> {
    let (sent, got) = (milli(qty_sent), milli(qty_received));
    if got < 0 {
        return Err(ReceiveRefusal::Negative);
    }
    let arrival = match got.cmp(&sent) {
        std::cmp::Ordering::Equal => Arrival::Exact,
        std::cmp::Ordering::Less => Arrival::Short,
        std::cmp::Ordering::Greater => {
            if note.is_none_or(|n| n.trim().is_empty()) {
                return Err(ReceiveRefusal::OverNeedsNote);
            }
            Arrival::Over
        }
    };
    Ok(LineCheck {
        arrival,
        difference: crate::from_milli(got - sent),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_dispatched_transfer_can_be_received() {
        for s in [
            TransferStatus::Requested,
            TransferStatus::Draft,
            TransferStatus::Received,
            TransferStatus::Cancelled,
        ] {
            assert_eq!(step(s, Action::Receive), None, "{s:?}");
        }
        let r = step(TransferStatus::Dispatched, Action::Receive).unwrap();
        assert_eq!(
            (r.side, r.next),
            (Side::Destination, TransferStatus::Received)
        );
    }

    #[test]
    fn finals_are_final() {
        for a in [
            Action::Edit,
            Action::Accept,
            Action::Decline,
            Action::Dispatch,
            Action::Receive,
            Action::Cancel,
        ] {
            assert_eq!(step(TransferStatus::Received, a), None);
            assert_eq!(step(TransferStatus::Cancelled, a), None);
        }
    }

    #[test]
    fn over_receive_needs_a_note() {
        assert_eq!(
            check_receive_line(5.0, 5.5, None),
            Err(ReceiveRefusal::OverNeedsNote)
        );
        assert_eq!(
            check_receive_line(5.0, 5.5, Some("  ")),
            Err(ReceiveRefusal::OverNeedsNote)
        );
        assert_eq!(
            check_receive_line(5.0, 5.5, Some("miscounted"))
                .unwrap()
                .arrival,
            Arrival::Over
        );
        // Float noise below a thousandth is not "over".
        assert_eq!(
            check_receive_line(0.3, 0.1 + 0.2, None).unwrap().arrival,
            Arrival::Exact
        );
        let short = check_receive_line(10.0, 7.25, None).unwrap();
        assert_eq!((short.arrival, short.difference), (Arrival::Short, -2.75));
        assert_eq!(
            check_receive_line(1.0, -0.001, None),
            Err(ReceiveRefusal::Negative)
        );
    }
}
