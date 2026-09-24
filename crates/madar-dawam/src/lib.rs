//! # madar-dawam
//!
//! Dawam (staff attendance) rules the backend (MadarRust `staff/`) and the
//! staff app's core (madar-core `dawam.rs`) must agree on, in ONE copy:
//!
//! - [`geofence`]: the great-circle distance and the branch's effective
//!   radius (DW2: an unset radius is 200 m, a 0 radius is 0 m);
//! - [`pay`]: the pay-period window (start day clamped to 1–28) and a
//!   percentage of a salary (DW3, the server's rounding);
//! - [`shift`]: a shift's wall-clock times as instants, overnight and on DST
//!   days, as the server's SQL places them (DW1);
//! - [`presence`]: the low-battery line (DW6);
//! - [`stamp`]: the offline stamp a punch recorded offline carries, and the
//!   signed anchor's format (`v1.<epoch ms>.<64 hex>`). Signing and verifying
//!   the HMAC stay on the server — only the shape is shared.
//!
//! Shift pricing, lateness, overtime and payroll are server-only by design:
//! the phone shows the server's figures. Pinned by `vectors/dawam_vectors.json`.

pub mod geofence;
pub mod pay;
pub mod presence;
pub mod shift;
pub mod stamp;
pub mod vectors;
