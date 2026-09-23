//! POS metrics rules both sides run: the figures the till shows offline must
//! be the ones `GET /reports/branches/{id}/pos-metrics` answers.
//!
//! v1 holds `average_ticket` and the report's constants. The fold itself is
//! SQL on the server and SQLite rows on the till; both are pinned by
//! `vectors/pos_metrics_vectors.json` (produced by the backend's SQL scenario).

/// The longest window one call may ask for (days).
pub const MAX_DAYS: i64 = 366;
/// How many items the leaderboard carries.
pub const TOP_ITEMS: i64 = 10;

/// Rounded half up; 0 when there is nothing to divide by.
pub fn average_ticket(net_sales: i64, order_count: i64) -> i64 {
    if order_count <= 0 {
        0
    } else {
        (2 * net_sales + order_count).div_euclid(2 * order_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every window the backend's SQL scenario recorded: its average ticket is
    /// this function of its own net sales and order count.
    #[test]
    fn every_recorded_window_averages_as_this_does() {
        let doc: serde_json::Value = serde_json::from_str(crate::vectors::POS_METRICS).unwrap();
        let windows = doc["expected"].as_array().unwrap();
        assert!(windows.len() >= 4);
        for w in windows {
            assert_eq!(
                average_ticket(
                    w["net_sales"].as_i64().unwrap(),
                    w["order_count"].as_i64().unwrap()
                ),
                w["average_ticket"].as_i64().unwrap(),
                "{} .. {}",
                w["from"],
                w["to"]
            );
            assert!(w["top_items"].as_array().unwrap().len() as i64 <= TOP_ITEMS);
        }
    }

    #[test]
    fn rounds_half_up_and_divides_nothing_by_nothing() {
        assert_eq!(average_ticket(0, 0), 0);
        assert_eq!(average_ticket(1000, 0), 0);
        assert_eq!(average_ticket(1000, -1), 0);
        assert_eq!(average_ticket(5, 2), 3);
        assert_eq!(average_ticket(7, 3), 2);
        assert_eq!(average_ticket(8, 3), 3);
        assert_eq!(average_ticket(-5, 2), -2);
    }
}
