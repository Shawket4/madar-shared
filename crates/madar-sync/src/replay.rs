//! The `/sync/replay` envelopes: one queued op from a device, carrying its
//! ORIGINAL actor (`teller_id`), in the shape the server accepts (MadarRust
//! `sync::handlers::ReplayOp`, a `#[serde(tag = "op")]` enum with permanent
//! aliases for older tills). The till builds each envelope in
//! `MadarCore::replay_envelope`.
//!
//! The `request` bodies stay [`serde_json::Value`] here: they are the SAME
//! bodies the live routes accept, and each side types them against its own
//! request structs. What this pins is the envelope: the op names, the aliases
//! the server still reads, and every field beside `request`.
//!
//! An envelope may also carry a manager's `approval` (read separately by the
//! server, see [`approval_of`]).
//!
//! `vectors/replay_current.json` is what the CURRENT release of the till
//! writes, one envelope per op (madar-core `replay_fixture` test,
//! `MADAR_WRITE_REPLAY_FIXTURE=1`); this crate and the backend both
//! deserialize it, so a till and a server that stop agreeing on an envelope
//! fail CI before a queued sale dead-letters.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Every op name the current server accepts, in its declaration order.
pub const OPS: &[&str] = &[
    "open_till",
    "close_till",
    "create_order",
    "void_order",
    "refund_order",
    "award_loyalty_points",
    "cash_movement",
    "spot_report_view",
    "fire_open_ticket",
    "add_ticket_round",
    "settle_open_ticket",
    "void_open_ticket",
    "void_ticket_line",
    "bump_kitchen_item",
    "unbump_kitchen_item",
    "swap_tables",
    "create_table_transfer",
    "cancel_table_transfer",
    "fulfill_table_transfer",
    "clear_table",
    "hold_table",
    "release_table",
    "seat_booking",
    "no_show_booking",
    "create_customer",
    "attach_customer",
    "set_ticket_customer",
    "record_waste",
    "record_staff_drink",
];

/// Permanent aliases: `(old name, the op it means)` (POS v0.5.1 / v0.6.0).
pub const ALIASES: &[(&str, &str)] = &[("open_shift", "open_till"), ("close_shift", "close_till")];

/// One queued op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ReplayOp {
    #[serde(alias = "open_shift")]
    OpenTill {
        teller_id: Uuid,
        branch_id: Uuid,
        #[serde(default)]
        device_id: Option<Uuid>,
        #[serde(default)]
        device_code: Option<String>,
        #[serde(default)]
        verification: Option<String>,
        request: Value,
    },
    #[serde(alias = "close_shift")]
    CloseTill {
        teller_id: Uuid,
        #[serde(alias = "shift_id")]
        till_id: Uuid,
        #[serde(default)]
        device_id: Option<Uuid>,
        request: Value,
    },
    CreateOrder {
        teller_id: Uuid,
        #[serde(default)]
        device_id: Option<Uuid>,
        #[serde(default)]
        device_code: Option<String>,
        request: Value,
    },
    VoidOrder {
        teller_id: Uuid,
        order_id: Uuid,
        request: Value,
    },
    RefundOrder {
        teller_id: Uuid,
        request: Value,
    },
    AwardLoyaltyPoints {
        teller_id: Uuid,
        request: Value,
    },
    CashMovement {
        teller_id: Uuid,
        #[serde(alias = "shift_id")]
        till_id: Uuid,
        #[serde(default)]
        device_id: Option<Uuid>,
        request: Value,
    },
    SpotReportView {
        teller_id: Uuid,
        till_id: Uuid,
        #[serde(default)]
        device_id: Option<Uuid>,
        request: Value,
    },
    FireOpenTicket {
        teller_id: Uuid,
        request: Value,
        #[serde(default)]
        origin_device_id: Option<String>,
    },
    AddTicketRound {
        teller_id: Uuid,
        ticket_id: Uuid,
        request: Value,
        #[serde(default)]
        origin_device_id: Option<String>,
    },
    SettleOpenTicket {
        teller_id: Uuid,
        ticket_id: Uuid,
        request: Value,
    },
    VoidOpenTicket {
        teller_id: Uuid,
        ticket_id: Uuid,
        request: Value,
    },
    VoidTicketLine {
        teller_id: Uuid,
        ticket_id: Uuid,
        item_id: Uuid,
        request: Value,
    },
    BumpKitchenItem {
        teller_id: Uuid,
        item_id: Uuid,
    },
    UnbumpKitchenItem {
        teller_id: Uuid,
        item_id: Uuid,
    },
    SwapTables {
        teller_id: Uuid,
        request: Value,
    },
    CreateTableTransfer {
        teller_id: Uuid,
        request: Value,
    },
    CancelTableTransfer {
        teller_id: Uuid,
        transfer_id: Uuid,
    },
    FulfillTableTransfer {
        teller_id: Uuid,
        transfer_id: Uuid,
        request: Value,
    },
    ClearTable {
        teller_id: Uuid,
        table_id: Uuid,
        #[serde(default)]
        request: Value,
    },
    HoldTable {
        teller_id: Uuid,
        table_id: Uuid,
        #[serde(default)]
        request: Value,
    },
    ReleaseTable {
        teller_id: Uuid,
        table_id: Uuid,
        #[serde(default)]
        request: Value,
    },
    SeatBooking {
        teller_id: Uuid,
        booking_id: Uuid,
        #[serde(default)]
        request: Value,
    },
    NoShowBooking {
        teller_id: Uuid,
        booking_id: Uuid,
    },
    CreateCustomer {
        teller_id: Uuid,
        request: Value,
    },
    AttachCustomer {
        teller_id: Uuid,
        order_id: Uuid,
        #[serde(default)]
        customer_id: Option<Uuid>,
    },
    SetTicketCustomer {
        teller_id: Uuid,
        ticket_id: Uuid,
        #[serde(default)]
        customer_id: Option<Uuid>,
    },
    RecordWaste {
        teller_id: Uuid,
        request: Value,
    },
    RecordStaffDrink {
        teller_id: Uuid,
        request: Value,
    },
}

impl ReplayOp {
    /// The wire name of this op (the current one, never an alias).
    pub fn op(&self) -> &'static str {
        match self {
            ReplayOp::OpenTill { .. } => "open_till",
            ReplayOp::CloseTill { .. } => "close_till",
            ReplayOp::CreateOrder { .. } => "create_order",
            ReplayOp::VoidOrder { .. } => "void_order",
            ReplayOp::RefundOrder { .. } => "refund_order",
            ReplayOp::AwardLoyaltyPoints { .. } => "award_loyalty_points",
            ReplayOp::CashMovement { .. } => "cash_movement",
            ReplayOp::SpotReportView { .. } => "spot_report_view",
            ReplayOp::FireOpenTicket { .. } => "fire_open_ticket",
            ReplayOp::AddTicketRound { .. } => "add_ticket_round",
            ReplayOp::SettleOpenTicket { .. } => "settle_open_ticket",
            ReplayOp::VoidOpenTicket { .. } => "void_open_ticket",
            ReplayOp::VoidTicketLine { .. } => "void_ticket_line",
            ReplayOp::BumpKitchenItem { .. } => "bump_kitchen_item",
            ReplayOp::UnbumpKitchenItem { .. } => "unbump_kitchen_item",
            ReplayOp::SwapTables { .. } => "swap_tables",
            ReplayOp::CreateTableTransfer { .. } => "create_table_transfer",
            ReplayOp::CancelTableTransfer { .. } => "cancel_table_transfer",
            ReplayOp::FulfillTableTransfer { .. } => "fulfill_table_transfer",
            ReplayOp::ClearTable { .. } => "clear_table",
            ReplayOp::HoldTable { .. } => "hold_table",
            ReplayOp::ReleaseTable { .. } => "release_table",
            ReplayOp::SeatBooking { .. } => "seat_booking",
            ReplayOp::NoShowBooking { .. } => "no_show_booking",
            ReplayOp::CreateCustomer { .. } => "create_customer",
            ReplayOp::AttachCustomer { .. } => "attach_customer",
            ReplayOp::SetTicketCustomer { .. } => "set_ticket_customer",
            ReplayOp::RecordWaste { .. } => "record_waste",
            ReplayOp::RecordStaffDrink { .. } => "record_staff_drink",
        }
    }
}

/// The manager's approval an envelope carries, if any (the server reads it
/// beside the op: `void_order`, `refund_order`, `spot_report_view`,
/// `settle_open_ticket`, `record_waste`, `record_staff_drink`).
pub fn approval_of(envelope: &Value) -> Option<&Value> {
    envelope.get("approval").filter(|a| !a.is_null())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_op_name_round_trips_through_its_variant() {
        let doc: Value = serde_json::from_str(crate::vectors::REPLAY_CURRENT).unwrap();
        let envelopes = doc["envelopes"].as_array().expect("envelopes");
        let mut seen = std::collections::BTreeSet::new();
        for env in envelopes {
            let op = env["op"].as_str().unwrap();
            let parsed: ReplayOp = serde_json::from_value(env.clone()).unwrap_or_else(|e| {
                panic!("{op}: the current till's envelope does not decode: {e}")
            });
            assert_eq!(parsed.op(), op, "{op} landed on another variant");
            seen.insert(op.to_string());
        }
        let missing: Vec<&&str> = OPS.iter().filter(|o| !seen.contains(**o)).collect();
        assert!(missing.is_empty(), "the fixture lacks {missing:?}");
    }

    #[test]
    fn the_aliases_land_on_the_current_ops() {
        let t = "00000000-0000-4000-8000-000000000001";
        for (old, new) in ALIASES {
            let env = serde_json::json!({
                "op": old, "teller_id": t, "branch_id": t, "till_id": t, "request": {}
            });
            let parsed: ReplayOp = serde_json::from_value(env).unwrap();
            assert_eq!(parsed.op(), *new);
        }
    }
}
