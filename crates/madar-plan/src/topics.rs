//! The realtime topics a role subscribes to, moved from madar-core
//! (`realtime::topics_for_role`) unchanged for Paid; none on Free.

use serde::{Deserialize, Serialize};

use crate::{entitlements, Plan};

/// A realtime topic: one stream of a branch's events (`/realtime/stream`).
/// The wire name is [`Topic::as_str`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Delivery,
    Tickets,
    Kitchen,
    Orders,
    /// The live floor: table geometry, table status, and the transfer waitlist.
    Floor,
    /// Table bookings: `booking.created|changed|arriving`.
    Bookings,
    /// Till lifecycle: `till.*`.
    Tills,
    /// `sync.changed { branch_id }`: the branch's changefeed moved.
    Sync,
    /// `payment_methods.availability_changed`.
    PaymentMethods,
}

impl Topic {
    /// Every topic, for the "subscribe to all I'm allowed to read" default.
    pub const ALL: [Topic; 9] = [
        Topic::Delivery,
        Topic::Tickets,
        Topic::Kitchen,
        Topic::Orders,
        Topic::Floor,
        Topic::Bookings,
        Topic::Tills,
        Topic::Sync,
        Topic::PaymentMethods,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Topic::Delivery => "delivery",
            Topic::Tickets => "tickets",
            Topic::Kitchen => "kitchen",
            Topic::Orders => "orders",
            Topic::Floor => "floor",
            Topic::Bookings => "bookings",
            Topic::Tills => "tills",
            Topic::Sync => "sync",
            Topic::PaymentMethods => "payment_methods",
        }
    }

    pub fn parse(s: &str) -> Option<Topic> {
        Topic::ALL.into_iter().find(|t| t.as_str() == s.trim())
    }
}

/// The topics a signed-in role needs on its one session-level subscription.
/// `role` is the work kind the core derives from the session (`kitchen`,
/// `waiter`, `teller`, `branch_manager`, `org_admin`, …).
///
/// Paid is exactly what madar-core's `topics_for_role` returned: a KDS sees the
/// kitchen; a waiter its tickets, the kitchen, the floor and bookings; every
/// other (operating) role the full set; every role also hears `sync`. Free
/// holds no stream, so no topics: its tills pull after each sale and once a
/// minute when idle.
pub fn realtime_topics(plan: Plan, role: &str) -> Vec<Topic> {
    if !entitlements(plan).realtime {
        return vec![];
    }
    match role {
        "kitchen" => vec![Topic::Kitchen, Topic::Sync],
        "waiter" => vec![
            Topic::Tickets,
            Topic::Kitchen,
            Topic::Floor,
            Topic::Bookings,
            Topic::Sync,
        ],
        _ => vec![
            Topic::Delivery,
            Topic::Kitchen,
            Topic::Tickets,
            Topic::Orders,
            Topic::Floor,
            Topic::Bookings,
            Topic::Tills,
            Topic::Sync,
        ],
    }
}
