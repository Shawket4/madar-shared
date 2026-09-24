//! The vector files this crate is pinned by, as bytes, for consumer tests (the
//! backend's SQL `phone_canonical`).

/// `{"valid": [[raw, canonical], …], "invalid": [raw, …]}`.
pub const PHONE: &str = include_str!("../vectors/phone_vectors.json");
/// `order_ref::vectors`: minting and reading order refs.
pub const ORDER_REF: &str = include_str!("../vectors/order_ref_vectors.json");
/// `member::vectors`: card tokens from fixed bytes, and the shape check.
pub const MEMBER: &str = include_str!("../vectors/member_vectors.json");
