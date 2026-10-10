//! The vector files this crate is pinned by, as bytes, for consumer tests.

/// `plan::vectors`: carts, programmes and asks, with the till's trimmed plan,
/// the server's verdict and the replay's lines.
pub const PLAN: &str = include_str!("../vectors/loyalty_plan_vectors.json");

/// `card`: a balance's rewards earned, progress and stamps (hand-computed).
pub const CARD: &str = include_str!("../vectors/loyalty_card_vectors.json");
