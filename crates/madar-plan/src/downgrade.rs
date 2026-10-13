//! A paid organisation drops to Free: which till each branch keeps selling on.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One till (a POS device) and when it last sold or held a session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TillUse {
    pub branch_id: String,
    pub device_id: String,
    /// When the till was last used, in any increasing unit (epoch ms). `None`:
    /// never used.
    pub last_used: Option<i64>,
}

/// The owner's choice of a branch's active till (Settings > Plan).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pick {
    pub branch_id: String,
    pub device_id: String,
}

/// The till a branch stays active on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keep {
    pub branch_id: String,
    pub device_id: String,
}

/// Free keeps one till per branch. In each branch that has a till: the owner's
/// pick when it names one of the branch's tills, else the most recently used
/// till (a till never used ranks last; equal times go to the smaller device id,
/// so the answer never depends on input order). Branches in branch id order.
pub fn downgrade_keeps(tills: &[TillUse], picks: &[Pick]) -> Vec<Keep> {
    let mut best: BTreeMap<&str, &TillUse> = BTreeMap::new();
    for t in tills {
        let slot = best.entry(&t.branch_id).or_insert(t);
        // Larger last_used wins; None < Some; ties go to the smaller device id.
        if (t.last_used, std::cmp::Reverse(&t.device_id))
            > (slot.last_used, std::cmp::Reverse(&slot.device_id))
        {
            *slot = t;
        }
    }
    best.into_iter()
        .map(|(branch, recent)| {
            let picked = picks
                .iter()
                .rev() // a later pick for the same branch wins
                .filter(|p| p.branch_id == branch)
                .find(|p| {
                    tills
                        .iter()
                        .any(|t| t.branch_id == branch && t.device_id == p.device_id)
                });
            Keep {
                branch_id: branch.to_string(),
                device_id: picked.map_or(&recent.device_id, |p| &p.device_id).clone(),
            }
        })
        .collect()
}
