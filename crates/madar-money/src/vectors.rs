//! The vector files this crate is pinned by, as bytes.
//!
//! They are this crate's own tests. A consumer test that needs the same
//! vectors for code that stays on its side — the backend's SQL
//! (`refund_share()`, the POS-metrics report), the till's bill assembly and
//! its offline metrics fold — reads them from here, so there is exactly one
//! copy of each file.

/// `tax::vectors`: 4096 bills (every policy × channel × discount).
pub const TAX: &str = include_str!("../vectors/tax_vectors.json");
/// `tax::refund_vectors`: the refund tax / service-charge split.
pub const REFUND_SPLIT: &str = include_str!("../vectors/refund_split_vectors.json");
/// `tax::negative_vectors`: the first negative figure of a bill.
pub const NEGATIVE_PART: &str = include_str!("../vectors/negative_part_vectors.json");
/// `loyalty::vectors`: reward cover through the bill.
pub const LOYALTY_REWARD: &str = include_str!("../vectors/loyalty_reward_vectors.json");
/// `staff_pool`: the pool decision and the business date (hand-authored spec).
pub const STAFF_POOL: &str = include_str!("../vectors/staff_pool_vectors.json");
/// `staff_comp`: what a staff drink is given free (hand-authored spec).
pub const STAFF_COMP: &str = include_str!("../vectors/staff_comp_vectors.json");
/// POS metrics: `/sync/pull` rows and the report the backend computed from
/// them. Produced by MadarRust `tests/reports_pos_metrics_tests.rs`.
pub const POS_METRICS: &str = include_str!("../vectors/pos_metrics_vectors.json");
/// `line::vectors`: what a sale line comes to.
pub const LINE_TOTAL: &str = include_str!("../vectors/line_total_vectors.json");
/// `bill::vectors`: bill assembly, table-bill previews and tenders.
pub const BILL: &str = include_str!("../vectors/bill_vectors.json");
/// `discount::vectors`: basis points, the discount ask and its figures.
pub const DISCOUNT: &str = include_str!("../vectors/discount_vectors.json");
