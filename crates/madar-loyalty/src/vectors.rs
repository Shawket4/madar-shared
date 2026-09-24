//! The vector files this crate is pinned by, as bytes, for consumer tests.

/// `plan::vectors`: carts, programmes and asks, with the till's trimmed plan,
/// the server's verdict and the replay's lines.
pub const PLAN: &str = include_str!("../vectors/loyalty_plan_vectors.json");
