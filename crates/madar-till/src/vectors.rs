//! The vector files this crate is pinned by, as bytes, for consumer tests that
//! pin their own code (the backend's SQL, the core's SQLite loads) to the same
//! file.

/// The backend's till scenarios: `/sync/pull` rows and the report the backend
/// computed from them. Produced by MadarRust
/// `tests/tills_report_vectors_tests.rs` (`MADAR_WRITE_TILL_VECTORS=1`).
pub const TILL_REPORT: &str = include_str!("../vectors/till_report_vectors.json");
/// The edges where the fold takes the server's reading (a blank-looking method
/// name, the fallback cash method's order), produced the same way.
pub const TILL_EDGE: &str = include_str!("../vectors/till_edge_vectors.json");
/// `carryover::vectors`: the drawer's last declared close.
pub const CARRYOVER: &str = include_str!("../vectors/carryover_vectors.json");
