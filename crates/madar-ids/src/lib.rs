//! # madar-ids
//!
//! Identifiers and formats the backend (MadarRust) and the POS core
//! (madar-core) must read and write the same way, in ONE copy:
//!
//! - [`phone`]: the canonical phone (E.164 digits without `+`), the key
//!   customers, loyalty, delivery and bookings share. Pinned by
//!   `vectors/phone_vectors.json`, which the backend's SQL `phone_canonical`
//!   also runs (and the dashboard's TypeScript copy mirrors);
//! - [`order_ref`]: the order reference a device mints and the server's
//!   fallback, the `~XXXX` collision suffix, and reading a device code back
//!   out of a ref;
//! - [`member`]: the loyalty member card token (`M` + 22 base64url).
//!
//! Nothing here does I/O, reads a clock or draws randomness: the caller passes
//! the random bytes of a new token in.

pub mod member;
pub mod order_ref;
pub mod phone;
pub mod vectors;
