//! The vector files this crate is pinned by, as bytes, for consumer tests.

/// `plan::vectors`: carts, programmes and asks, with the till's trimmed plan,
/// the server's verdict and the replay's lines.
pub const PLAN: &str = include_str!("../vectors/loyalty_plan_vectors.json");

/// `card`: a balance's rewards earned, progress and stamps (hand-computed).
pub const CARD: &str = include_str!("../vectors/loyalty_card_vectors.json");

/// `tracks::plan_tracks`: mixed baskets across stamp tracks (hand-written
/// inputs, answers reviewed).
pub const TRACKS: &str = include_str!("../vectors/loyalty_tracks_vectors.json");

/// `gift::plan_gift`: which line may take the birthday gift (hand-written
/// inputs and answers).
pub const GIFT: &str = include_str!("../vectors/loyalty_gift_vectors.json");
