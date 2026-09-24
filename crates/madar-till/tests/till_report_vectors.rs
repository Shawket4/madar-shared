//! The fold against the backend's own figures.
//!
//! `till_report_vectors.json` (and `till_edge_vectors.json`) are produced by
//! MadarRust `tests/tills_report_vectors_tests.rs`: SQL scenarios, the rows as
//! `/sync/pull` projects them, and what the backend computed in SQL. Here the
//! rows are read the way the POS core stores them (madar-core
//! `ledger::write_row`) and folded; the core's own test runs the same file
//! through its SQLite ledger.

use madar_till::report::{self, Figures, Leg, Method, MovementRow, OrderRefund, Refund, Sale};
use serde_json::Value;

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}
fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}
fn rows<'a>(sc: &'a Value, ty: &str) -> &'a [Value] {
    sc["rows"][ty].as_array().map(Vec::as_slice).unwrap_or(&[])
}

/// The tax and service charge each refund of `order_id` took back, by refund id.
fn splits_of(sc: &Value, order_id: &str) -> Vec<(String, (i64, i64))> {
    let Some(order) = rows(sc, "order")
        .iter()
        .find(|o| s(o, "id").as_deref() == Some(order_id))
    else {
        return Vec::new();
    };
    let mut refunds: Vec<&Value> = rows(sc, "refund")
        .iter()
        .filter(|r| s(r, "order_id").as_deref() == Some(order_id))
        .collect();
    refunds.sort_by_key(|r| (s(r, "issued_at"), s(r, "id")));
    let queued: Vec<OrderRefund> = refunds
        .iter()
        .map(|r| OrderRefund {
            key: s(r, "id").unwrap(),
            amount: i(r, "amount"),
            known: match (
                r.get("tax_amount").and_then(Value::as_i64),
                r.get("service_charge_amount").and_then(Value::as_i64),
            ) {
                (Some(t), Some(c)) => Some((t, c)),
                _ => None,
            },
        })
        .collect();
    report::refund_splits(
        i(order, "total_amount"),
        i(order, "tax_amount"),
        i(order, "service_charge_amount"),
        &queued,
    )
}

fn figures_of(sc: &Value, till_id: &str) -> (Figures, Vec<String>) {
    let till = rows(sc, "till")
        .iter()
        .find(|t| s(t, "id").as_deref() == Some(till_id))
        .expect("the till row");
    let sales: Vec<Sale> = rows(sc, "order")
        .iter()
        .filter(|o| s(o, "till_id").as_deref() == Some(till_id))
        .map(|o| {
            let key = s(o, "id").unwrap();
            let taken = splits_of(sc, &key)
                .into_iter()
                .fold((0, 0), |a, (_, (t, c))| (a.0 + t, a.1 + c));
            Sale {
                status: s(o, "status").unwrap_or_else(|| "completed".into()),
                payment_method: s(o, "payment_method").unwrap_or_default(),
                total: i(o, "total_amount"),
                tip: i(o, "tip_amount"),
                tip_method: s(o, "tip_payment_method"),
                tip_is_cash: o.get("tip_is_cash").and_then(Value::as_bool),
                legs: o["payment_legs"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .map(|l| {
                        let method = s(l, "method").unwrap_or_default();
                        Leg {
                            is_cash: l
                                .get("is_cash")
                                .and_then(Value::as_bool)
                                .unwrap_or(method == "cash"),
                            amount: i(l, "amount"),
                            method,
                        }
                    })
                    .collect(),
                tax: i(o, "tax_amount"),
                service_charge: i(o, "service_charge_amount"),
                waived: s(o, "service_charge_waived_by").is_some(),
                waived_amount: i(o, "service_charge_waived_amount"),
                unsent: false,
                refunded_tax: taken.0,
                refunded_service_charge: taken.1,
                key,
            }
        })
        .collect();
    let moves = report::movements(
        rows(sc, "cash_movement")
            .iter()
            .filter(|m| s(m, "till_id").as_deref() == Some(till_id))
            .map(|m| MovementRow {
                key: s(m, "id").unwrap(),
                server_id: s(m, "id"),
                amount: i(m, "amount"),
                kind: s(m, "kind").unwrap_or_default(),
                corrects_id: s(m, "corrects_id"),
                created_at: s(m, "created_at").unwrap_or_default(),
                note: s(m, "note").unwrap_or_default(),
                moved_by_name: s(m, "moved_by_name").unwrap_or_default(),
            })
            .collect(),
    );
    let refunds: Vec<Refund> = rows(sc, "refund")
        .iter()
        .filter(|r| s(r, "till_id").as_deref() == Some(till_id))
        .map(|r| {
            let id = s(r, "id").unwrap();
            let (tax, service_charge) = splits_of(sc, &s(r, "order_id").unwrap_or_default())
                .into_iter()
                .find(|(k, _)| *k == id)
                .map(|(_, split)| split)
                .unwrap_or((0, 0));
            Refund {
                amount: i(r, "amount"),
                method: s(r, "method").unwrap_or_default(),
                is_cash: r.get("is_cash").and_then(Value::as_bool).unwrap_or(false),
                tax,
                service_charge,
            }
        })
        .collect();
    let methods: Vec<Method> = rows(sc, "payment_method")
        .iter()
        .map(|m| Method {
            id: s(m, "id").unwrap_or_default(),
            name: s(m, "name").unwrap_or_default(),
            is_cash: m.get("is_cash").and_then(Value::as_bool).unwrap_or(false),
            is_active: m.get("is_active").and_then(Value::as_bool).unwrap_or(true),
            created_at: s(m, "created_at"),
        })
        .collect();
    let f = report::fold(
        i(till, "opening_cash"),
        till.get("closing_cash_system").and_then(Value::as_i64),
        &sales,
        &moves,
        &refunds,
        &methods,
    );
    (f, moves.into_iter().map(|m| m.id).collect())
}

fn check(file: &str, raw: &str) -> usize {
    let doc: Value = serde_json::from_str(raw).unwrap();
    let mut n = 0;
    for sc in doc["scenarios"].as_array().unwrap() {
        let name = sc["name"].as_str().unwrap();
        for (till_id, want) in sc["expected"].as_object().unwrap() {
            let (f, movement_ids) = figures_of(sc, till_id);
            let got = serde_json::json!({
                "system_cash": f.system_cash,
                "expected_cash": f.expected_cash,
                "payment_summary": f.payment_summary.iter().map(|p| serde_json::json!({
                    "payment_method": p.payment_method, "is_cash": p.is_cash, "total": p.total, "order_count": p.order_count,
                })).collect::<Vec<_>>(),
                "total_payments": f.total_payments,
                "voided_amount": f.voided_amount,
                "net_payments": f.net_payments,
                "total_tips": f.total_tips,
                "cash_tips": f.cash_tips,
                "non_cash_tips": f.non_cash_tips,
                "cash_movements_in": f.cash_movements_in,
                "cash_movements_out": f.cash_movements_out,
                "safe_drops": f.safe_drops,
                "cash_adjustments": f.cash_adjustments,
                "cash_movements_net": f.cash_movements_net,
                "cash_movement_ids": movement_ids,
                "refunds_issued_count": f.refunds_issued_count,
                "refunds_issued_amount": f.refunds_issued_amount,
                "refunds_issued_cash": f.refunds_issued_cash,
                "cash_in_refunded_sales": f.cash_in_refunded_sales,
                "total_tax": f.total_tax,
                "total_service_charge": f.total_service_charge,
                "refunds_issued_tax": f.refunds_issued_tax,
                "refunds_issued_service_charge": f.refunds_issued_service_charge,
                "service_charge_waived_count": f.service_charge_waived_count,
                "service_charge_waived_amount": f.service_charge_waived_amount,
                "close_methods": f.close_methods.iter().map(|m| serde_json::json!({
                    "method": m.method, "is_cash": m.is_cash, "system_total": m.system_total, "order_count": m.order_count,
                    "payment_method_id": m.payment_method_id,
                })).collect::<Vec<_>>(),
            });
            for (k, g) in got.as_object().unwrap() {
                assert_eq!(Some(g), want.get(k), "{file} / {name} / {till_id}: {k}");
            }
            n += 1;
        }
    }
    n
}

#[test]
fn the_fold_agrees_with_the_backends_till_report() {
    let n = check("till_report_vectors.json", madar_till::vectors::TILL_REPORT);
    assert!(n >= 10, "only {n} tills checked");
}

#[test]
fn the_fold_takes_the_servers_reading_of_the_edges() {
    check("till_edge_vectors.json", madar_till::vectors::TILL_EDGE);
}
