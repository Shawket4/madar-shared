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

pub mod vectors {
    //! The LIVE close check (`replay = false`): what [`super::plan_lines`]
    //! plans, or the refusal and its code, for a close's inputs. The dashboard's
    //! close dialog re-implements the check and is pinned to this file.
    //! Regenerate deliberately:
    //! `MADAR_REGENERATE_RECONCILE_VECTORS=1 cargo test -p madar-till reconcile_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{plan_lines, Input, MethodTotal, PlanError};

    /// A [`MethodTotal`], as the file carries it.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct TotalCase {
        pub method: String,
        pub payment_method_id: Option<String>,
        pub is_cash: bool,
        pub system_total: i64,
        pub order_count: i64,
    }

    /// An [`Input`], as the file carries it.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct InputCase {
        pub method: String,
        pub status: String,
        pub declared_amount: Option<i32>,
        pub note: Option<String>,
    }

    /// A [`super::PlannedLine`], as the file carries it.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct LineCase {
        pub method: String,
        pub payment_method_id: Option<String>,
        pub is_cash: bool,
        pub system_total: i32,
        pub order_count: i32,
        pub status: String,
        pub declared_amount: Option<i32>,
        pub note: Option<String>,
    }

    /// A [`PlanError`]: its code (`null` for an invalid status) and method.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Refusal {
        pub code: Option<String>,
        pub method: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ReconcileVector {
        pub name: String,
        pub totals: Vec<TotalCase>,
        pub closing_cash_declared: i32,
        pub closing_cash_system: i32,
        pub cash_note: Option<String>,
        pub inputs: Vec<InputCase>,
        /// `{"Ok": [lines]}` or `{"Err": {code, method}}`.
        pub expected: Result<Vec<LineCase>, Refusal>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/reconcile_vectors.json")
    }

    fn total(method: &str, pmid: Option<&str>, is_cash: bool, sys: i64, n: i64) -> TotalCase {
        TotalCase {
            method: method.into(),
            payment_method_id: pmid.map(str::to_string),
            is_cash,
            system_total: sys,
            order_count: n,
        }
    }

    fn input(method: &str, status: &str, amount: Option<i32>, note: Option<&str>) -> InputCase {
        InputCase {
            method: method.into(),
            status: status.into(),
            declared_amount: amount,
            note: note.map(str::to_string),
        }
    }

    fn plan(
        totals: &[TotalCase],
        declared: i32,
        system: i32,
        cash_note: Option<&str>,
        inputs: &[InputCase],
    ) -> Result<Vec<LineCase>, Refusal> {
        let totals: Vec<MethodTotal<String>> = totals
            .iter()
            .map(|t| MethodTotal {
                method: t.method.clone(),
                payment_method_id: t.payment_method_id.clone(),
                is_cash: t.is_cash,
                system_total: t.system_total,
                order_count: t.order_count,
            })
            .collect();
        let inputs: Vec<Input<'_>> = inputs
            .iter()
            .map(|i| Input {
                method: &i.method,
                status: &i.status,
                declared_amount: i.declared_amount,
                note: i.note.as_deref(),
            })
            .collect();
        match plan_lines(&totals, declared, system, cash_note, &inputs, false) {
            Ok(lines) => Ok(lines
                .into_iter()
                .map(|l| LineCase {
                    method: l.method,
                    payment_method_id: l.payment_method_id,
                    is_cash: l.is_cash,
                    system_total: l.system_total,
                    order_count: l.order_count,
                    status: l.status.into(),
                    declared_amount: l.declared_amount,
                    note: l.note,
                })
                .collect()),
            Err(e) => Err(Refusal {
                code: e.code().map(str::to_string),
                method: match e {
                    PlanError::AmountRequired { method }
                    | PlanError::NoteRequired { method }
                    | PlanError::InvalidStatus { method, .. } => method,
                },
            }),
        }
    }

    pub fn generate() -> Vec<ReconcileVector> {
        // Amounts in piastres.
        let cash = || total("cash", None, true, 50_000, 5);
        let card = || total("card", None, false, 30_000, 2);
        type Case = (
            &'static str,
            Vec<TotalCase>,
            i32,
            i32,
            Option<&'static str>,
            Vec<InputCase>,
        );
        let cases: Vec<Case> =
            vec![
            (
                "matching: cash counted equal, card checked",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "checked", None, None)],
            ),
            (
                "cash short: the cash line disagrees and keeps its note",
                vec![cash(), card()],
                49_500,
                50_000,
                Some("  5 EGP short  "),
                vec![input("card", "checked", None, None)],
            ),
            (
                "cash over without a note: the cash line is never refused",
                vec![cash(), card()],
                50_700,
                50_000,
                Some("   "),
                vec![input("card", "checked", None, None)],
            ),
            (
                "card short: disagreed with the amount seen and a note",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", Some(29_000), Some("one slip missing"))],
            ),
            (
                "card over: disagreed with the amount seen and a note",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", Some(31_000), Some("double swipe"))],
            ),
            (
                "missing amount: RECONCILIATION_AMOUNT_REQUIRED",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", None, Some("terminal down"))],
            ),
            (
                "missing note: RECONCILIATION_NOTE_REQUIRED",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", Some(29_000), None)],
            ),
            (
                "whitespace note is no note: RECONCILIATION_NOTE_REQUIRED",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", Some(29_000), Some(" \t\n "))],
            ),
            (
                "missing both: the amount is checked first",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", None, None)],
            ),
            (
                "the note is stored trimmed",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", Some(29_000), Some("  terminal down  "))],
            ),
            (
                "an amount of 0 is an amount",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", Some(0), Some("terminal never settled"))],
            ),
            (
                "disagreed at the system's own amount stays disagreed",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "disagreed", Some(30_000), Some("checked twice"))],
            ),
            (
                "no input for a method: unreviewed",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![],
            ),
            (
                "status neither checked nor disagreed: refused with no code",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![input("card", "Checked", None, None)],
            ),
            (
                "several methods: cash first, then the till's order, then methods not used on it",
                vec![
                    total("Cash", Some("pm-cash"), true, 80_000, 9),
                    total("card", Some("pm-card"), false, 30_000, 2),
                    total("instapay", Some("pm-instapay"), false, 12_500, 1),
                    total("talabat", Some("pm-talabat"), false, 7_000, 1),
                ],
                80_000,
                80_000,
                None,
                vec![
                    input("voucher", "disagreed", Some(1_000), Some("paper voucher")),
                    input(" talabat ", "checked", None, Some("  ")),
                    input("instapay", "disagreed", Some(12_000), Some("refund pending")),
                    input("card", "checked", None, Some("ok")),
                    // Ignored: blank, the cash method, a second card input.
                    input("", "disagreed", None, None),
                    input("Cash", "disagreed", None, None),
                    input("card", "disagreed", None, None),
                ],
            ),
            (
                "several methods: the first refusal in line order, not input order",
                vec![
                    cash(),
                    card(),
                    total("instapay", None, false, 12_500, 1),
                ],
                50_000,
                50_000,
                None,
                vec![
                    input("instapay", "disagreed", None, Some("missing")),
                    input("card", "disagreed", Some(29_000), None),
                ],
            ),
            (
                "a method not used on the till is refused like any other",
                vec![cash(), card()],
                50_000,
                50_000,
                None,
                vec![
                    input("card", "checked", None, None),
                    input("voucher", "disagreed", Some(1_000), None),
                ],
            ),
            (
                "no cash taken: the cash line is still first, named cash",
                vec![card()],
                0,
                0,
                None,
                vec![input("card", "checked", None, None)],
            ),
        ];
        cases
            .into_iter()
            .map(
                |(name, totals, declared, system, cash_note, inputs)| ReconcileVector {
                    name: name.into(),
                    expected: plan(&totals, declared, system, cash_note, &inputs),
                    totals,
                    closing_cash_declared: declared,
                    closing_cash_system: system,
                    cash_note: cash_note.map(str::to_string),
                    inputs,
                },
            )
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reconcile_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_RECONCILE_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vec<ReconcileVector> =
                serde_json::from_str(crate::vectors::RECONCILE).unwrap();
            assert_eq!(generated, expected, "the close reconciliation drifted");
        }

        /// Worked by hand: a disagreement needs an amount (checked first),
        /// then a note that is not blank; the cash line is never refused.
        #[test]
        fn reconcile_vectors_say_the_rule() {
            let v: Vec<ReconcileVector> = serde_json::from_str(crate::vectors::RECONCILE).unwrap();
            let refusal = |name: &str| {
                let c = v.iter().find(|c| c.name.starts_with(name)).unwrap();
                c.expected
                    .as_ref()
                    .err()
                    .map(|r| (r.code.clone(), r.method.clone()))
            };
            let code = |c: &str, m: &str| Some((Some(c.to_string()), m.to_string()));
            assert_eq!(
                refusal("missing amount"),
                code(super::super::CODE_AMOUNT_REQUIRED, "card")
            );
            assert_eq!(
                refusal("missing note"),
                code(super::super::CODE_NOTE_REQUIRED, "card")
            );
            assert_eq!(
                refusal("whitespace note"),
                code("RECONCILIATION_NOTE_REQUIRED", "card")
            );
            assert_eq!(
                refusal("missing both"),
                code("RECONCILIATION_AMOUNT_REQUIRED", "card")
            );
            assert_eq!(
                refusal("several methods: the first"),
                code("RECONCILIATION_NOTE_REQUIRED", "card")
            );
            assert_eq!(refusal("status neither"), Some((None, "card".into())));
            for ok in [
                "matching",
                "cash short",
                "cash over",
                "card short",
                "card over",
                "an amount of 0",
            ] {
                assert_eq!(refusal(ok), None, "{ok}");
            }
            let lines = |name: &str| {
                let c = v.iter().find(|c| c.name.starts_with(name)).unwrap();
                c.expected.clone().unwrap()
            };
            let over = &lines("cash over")[0];
            assert_eq!(
                (
                    over.status.as_str(),
                    over.declared_amount,
                    over.note.as_deref()
                ),
                ("disagreed", Some(50_700), None)
            );
            let card = &lines("the note is stored trimmed")[1];
            assert_eq!(card.note.as_deref(), Some("terminal down"));
            let several: Vec<_> = lines("several methods: cash first")
                .iter()
                .map(|l| (l.method.clone(), l.system_total, l.status.clone()))
                .collect();
            assert_eq!(
                several,
                [
                    ("Cash".to_string(), 80_000, "checked".to_string()),
                    ("card".into(), 30_000, "checked".into()),
                    ("instapay".into(), 12_500, "disagreed".into()),
                    ("talabat".into(), 7_000, "checked".into()),
                    ("voucher".into(), 0, "disagreed".into()),
                ]
            );
        }
    }
}
