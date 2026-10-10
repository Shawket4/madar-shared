//! Inventory vectors: the transfer step table, receive checks and
//! replenishment suggestions. Regenerate deliberately:
//! `MADAR_REGENERATE_INVENTORY_VECTORS=1 cargo test -p madar-inventory inventory_vectors`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::replenish::{self, Input, Suggestion};
use crate::transfer::{self, Action, ReceiveRefusal, Side, TransferStatus};

/// The file, for consumer tests.
pub const INVENTORY: &str = include_str!("../vectors/inventory_vectors.json");
/// `purchase` (hand-computed).
pub const PURCHASE: &str = include_str!("../vectors/purchase_vectors.json");
/// `count` (hand-computed).
pub const COUNT: &str = include_str!("../vectors/count_vectors.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepVector {
    pub status: TransferStatus,
    pub action: Action,
    /// `None` = refused in this status.
    pub side: Option<Side>,
    pub cap: Option<String>,
    pub next: Option<TransferStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReceiveVector {
    pub qty_sent: f64,
    pub qty_received: f64,
    pub note: Option<String>,
    pub ok: Option<transfer::LineCheck>,
    pub refused: Option<ReceiveRefusal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplenishVector {
    pub input: Input,
    pub out: Suggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vectors {
    pub steps: Vec<StepVector>,
    pub receives: Vec<ReceiveVector>,
    pub replenish: Vec<ReplenishVector>,
}

pub fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/inventory_vectors.json")
}

pub fn generate() -> Vectors {
    use Action as A;
    use TransferStatus as S;
    let mut steps = Vec::new();
    for status in [
        S::Requested,
        S::Draft,
        S::Dispatched,
        S::Received,
        S::Cancelled,
    ] {
        for action in [
            A::Edit,
            A::Accept,
            A::Decline,
            A::Dispatch,
            A::Receive,
            A::Cancel,
        ] {
            let s = transfer::step(status, action);
            steps.push(StepVector {
                status,
                action,
                side: s.map(|s| s.side),
                cap: s.map(|s| s.cap.key().to_string()),
                next: s.map(|s| s.next),
            });
        }
    }

    let mut receives = Vec::new();
    for (sent, got) in [
        (10.0, 10.0),
        (10.0, 7.25),
        (10.0, 0.0),
        (10.0, 12.0),
        (0.3, 0.1 + 0.2),
        (1.0, 1.0004),
        (1.0, 1.0006),
        (1.0, -0.001),
        (2.5, 2.4995),
    ] {
        for note in [None, Some("  "), Some("miscounted at dispatch")] {
            let r = transfer::check_receive_line(sent, got, note);
            receives.push(ReceiveVector {
                qty_sent: sent,
                qty_received: got,
                note: note.map(str::to_string),
                ok: r.ok(),
                refused: r.err(),
            });
        }
    }

    let base = Input {
        on_hand: 2.0,
        par_min: 5.0,
        par_max: Some(20.0),
        in_transit: 0.0,
        open_inbound: 0.0,
        warehouse_on_hand: 100.0,
        warehouse_drafted_out: 0.0,
    };
    let inputs = [
        base,
        Input {
            par_max: None,
            ..base
        },
        Input {
            par_max: Some(3.0),
            ..base
        }, // a max under the min reads as the min
        Input {
            on_hand: 5.0,
            ..base
        }, // exactly at par: needs
        Input {
            on_hand: 5.001,
            ..base
        }, // just above: doesn't
        Input {
            on_hand: -4.5,
            ..base
        }, // sold past zero
        Input {
            par_min: 0.0,
            on_hand: -4.5,
            ..base
        }, // no par: never
        Input {
            in_transit: 10.0,
            ..base
        },
        Input {
            in_transit: 10.0,
            open_inbound: 9.0,
            ..base
        },
        Input {
            warehouse_on_hand: 7.5,
            ..base
        },
        Input {
            warehouse_drafted_out: 95.0,
            ..base
        },
        Input {
            warehouse_drafted_out: 120.0,
            ..base
        },
        Input {
            warehouse_on_hand: -2.0,
            ..base
        },
        Input {
            on_hand: 0.1 + 0.2,
            par_min: 0.3,
            par_max: Some(1.2),
            ..base
        },
    ];
    let replenish = inputs
        .into_iter()
        .map(|input| ReplenishVector {
            input,
            out: replenish::suggest(&input),
        })
        .collect();

    Vectors {
        steps,
        receives,
        replenish,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_vectors() {
        let generated = generate();
        if std::env::var("MADAR_REGENERATE_INVENTORY_VECTORS").is_ok() {
            std::fs::write(
                fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let expected: Vectors = serde_json::from_str(INVENTORY).unwrap();
        assert_eq!(
            generated.steps, expected.steps,
            "the transfer lifecycle drifted"
        );
        assert_eq!(
            generated.receives, expected.receives,
            "the receive check drifted"
        );
        assert_eq!(
            generated.replenish, expected.replenish,
            "replenishment drifted"
        );
    }
}
