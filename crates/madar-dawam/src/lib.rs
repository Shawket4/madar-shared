//! # madar-dawam
//!
//! Dawam (staff attendance) rules the backend (MadarRust `staff/`) and the
//! staff app's core (madar-core `dawam.rs`) must agree on, in ONE copy:
//!
//! - [`geofence`]: the great-circle distance and the branch's effective
//!   radius (DW2: an unset radius is 200 m, a 0 radius is 0 m);
//! - [`pay`]: the pay-period window (start day clamped to 1–28);
//! - [`stamp`]: the offline stamp a punch recorded offline carries, and the
//!   signed anchor's format (`v1.<epoch ms>.<64 hex>`). Signing and verifying
//!   the HMAC stay on the server — only the shape is shared.
//!
//! Shift pricing, lateness, overtime and payroll are server-only by design:
//! the phone shows the server's figures. Pinned by `vectors/dawam_vectors.json`.

pub mod geofence;
pub mod pay;
pub mod shift;
pub mod stamp;
pub mod vectors;
