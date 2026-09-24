//! Close-till reconciliation: one line per payment method used on a till.
//!
//! Moved from MadarRust `tills/reconcile.rs` (`plan_lines`, `rollup_status`,
//! the statuses and error codes); the till validates its close inputs with
//! the same codes and shows the lines it will store offline with the same
//! planner (madar-core `till.rs`).
//!
//! Closing is NEVER blocked by reconciliation. A live close validates the
//! input (a disagreement needs an amount and a note); a replayed close never
//! fails on it (a missing note is stored as `(no note)`, a missing amount as
//! the system total, an unknown status as `unreviewed`).
//!
//! Generic over the payment-method id type (`Uuid` on the server, `String` on
//! the till); it is only carried through.

pub const STATUS_CLEAN: &str = "clean";
pub const STATUS_DISAGREED: &str = "disagreed";
pub const STATUS_UNREVIEWED: &str = "unreviewed";
pub const REPLAY_MISSING_NOTE: &str = "(no note)";
pub const CODE_NOTE_REQUIRED: &str = "RECONCILIATION_NOTE_REQUIRED";
pub const CODE_AMOUNT_REQUIRED: &str = "RECONCILIATION_AMOUNT_REQUIRED";

const CASH_FALLBACK_NAME: &str = "cash";

/// What the system says one method took on a till.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodTotal<Id> {
    pub method: String,
    pub payment_method_id: Option<Id>,
    pub is_cash: bool,
    pub system_total: i64,
    pub order_count: i64,
}

/// What the teller said about one method at close.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Input<'a> {
    pub method: &'a str,
    /// `checked` | `disagreed`
    pub status: &'a str,
    pub declared_amount: Option<i32>,
    pub note: Option<&'a str>,
}

/// A line to store, before it has a timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedLine<Id> {
    pub method: String,
    pub payment_method_id: Option<Id>,
    pub is_cash: bool,
    pub system_total: i32,
    pub order_count: i32,
    pub status: &'static str,
    pub declared_amount: Option<i32>,
    pub note: Option<String>,
}

/// Why a LIVE close's inputs were refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// [`CODE_AMOUNT_REQUIRED`]: a disagreement without the amount seen.
    AmountRequired { method: String },
    /// [`CODE_NOTE_REQUIRED`]: a disagreement without a note.
    NoteRequired { method: String },
    /// A status that is neither `checked` nor `disagreed`.
    InvalidStatus { method: String, status: String },
}

impl PlanError {
    /// The stable code, when the refusal has one.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            PlanError::AmountRequired { .. } => Some(CODE_AMOUNT_REQUIRED),
            PlanError::NoteRequired { .. } => Some(CODE_NOTE_REQUIRED),
            PlanError::InvalidStatus { .. } => None,
        }
    }
}

fn clamp_i32(v: i64) -> i32 {
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// A note that is blank after trimming is no note.
pub fn blank_to_none(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Rollup: any disagreed → `disagreed`, else any unreviewed → `unreviewed`,
/// else `clean`.
pub fn rollup_status<'a>(statuses: impl IntoIterator<Item = &'a str>) -> &'static str {
    let mut unreviewed = false;
    for s in statuses {
        match s {
            "disagreed" => return STATUS_DISAGREED,
            "unreviewed" => unreviewed = true,
            _ => {}
        }
    }
    if unreviewed {
        STATUS_UNREVIEWED
    } else {
        STATUS_CLEAN
    }
}

/// Validate a close's inputs against the methods used and plan the lines:
/// the cash line first (its status from the count), then every non-cash
/// method used, then inputs naming a method not used on the till (system
/// total 0).
pub fn plan_lines<Id: Clone>(
    totals: &[MethodTotal<Id>],
    closing_cash_declared: i32,
    closing_cash_system: i32,
    cash_note: Option<&str>,
    inputs: &[Input<'_>],
    replay: bool,
) -> Result<Vec<PlannedLine<Id>>, PlanError> {
    let mut lines = Vec::with_capacity(totals.len() + 1);
    let cash = totals.iter().find(|t| t.is_cash);
    lines.push(PlannedLine {
        method: cash
            .map(|c| c.method.clone())
            .unwrap_or_else(|| CASH_FALLBACK_NAME.into()),
        payment_method_id: cash.and_then(|c| c.payment_method_id.clone()),
        is_cash: true,
        system_total: closing_cash_system,
        order_count: cash.map(|c| clamp_i32(c.order_count)).unwrap_or(0),
        status: if closing_cash_declared == closing_cash_system {
            "checked"
        } else {
            "disagreed"
        },
        declared_amount: Some(closing_cash_declared),
        note: blank_to_none(cash_note),
    });
    let cash_method = lines[0].method.clone();

    let find_input = |method: &str| inputs.iter().find(|i| i.method.trim() == method);
    let plan_one = |method: &str,
                    pmid: Option<Id>,
                    total: i64,
                    count: i64|
     -> Result<PlannedLine<Id>, PlanError> {
        let system_total = clamp_i32(total);
        let base = PlannedLine {
            method: method.to_string(),
            payment_method_id: pmid,
            is_cash: false,
            system_total,
            order_count: clamp_i32(count),
            status: "unreviewed",
            declared_amount: None,
            note: None,
        };
        let Some(input) = find_input(method) else {
            return Ok(base);
        };
        match input.status.trim() {
            "checked" => Ok(PlannedLine {
                status: "checked",
                note: blank_to_none(input.note),
                ..base
            }),
            "disagreed" => {
                let amount = match input.declared_amount {
                    Some(a) => a,
                    None if replay => system_total,
                    None => {
                        return Err(PlanError::AmountRequired {
                            method: method.to_string(),
                        })
                    }
                };
                let note = match blank_to_none(input.note) {
                    Some(n) => n,
                    None if replay => REPLAY_MISSING_NOTE.to_string(),
                    None => {
                        return Err(PlanError::NoteRequired {
                            method: method.to_string(),
                        })
                    }
                };
                Ok(PlannedLine {
                    status: "disagreed",
                    declared_amount: Some(amount),
                    note: Some(note),
                    ..base
                })
            }
            _ if replay => Ok(base),
            other => Err(PlanError::InvalidStatus {
                method: method.to_string(),
                status: other.to_string(),
            }),
        }
    };

    for t in totals.iter().filter(|t| !t.is_cash) {
        lines.push(plan_one(
            &t.method,
            t.payment_method_id.clone(),
            t.system_total,
            t.order_count,
        )?);
    }
    // Inputs naming a method not used on the till: stored with system total 0.
    let mut extra: Vec<&Input<'_>> = Vec::new();
    for i in inputs {
        let m = i.method.trim();
        if m.is_empty()
            || m == cash_method
            || lines.iter().any(|l| l.method == m)
            || extra.iter().any(|e| e.method.trim() == m)
        {
            continue;
        }
        extra.push(i);
    }
    for i in extra {
        lines.push(plan_one(i.method.trim(), None, 0, 0)?);
    }
    Ok(lines)
}

/// Force-close planning: no line was counted or checked by anyone.
pub fn force_close_lines<Id>(planned: Vec<PlannedLine<Id>>) -> Vec<PlannedLine<Id>> {
    planned
        .into_iter()
        .map(|l| PlannedLine {
            status: STATUS_UNREVIEWED,
            declared_amount: None,
            note: None,
            ..l
        })
        .collect()
}
