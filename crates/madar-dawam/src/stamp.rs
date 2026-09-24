//! What a punch or ping recorded offline carries, so the server can date it
//! without trusting the phone's wall clock (MadarRust `staff/dawam/clock.rs`,
//! madar-core `dawam.rs` `stamp`).
//!
//! The anchor is the last `X-Dawam-Time` header the phone saw: the server's
//! time, signed for that phone's device row, `v1.<epoch ms>.<64 hex>`. The
//! signature is HMAC-SHA256 and is made and checked on the server only; this
//! module only shapes and reads it.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The response header every staff-app response carries.
pub const ANCHOR_HEADER: &str = "x-dawam-time";
/// The anchor format's version tag.
pub const ANCHOR_VERSION: &str = "v1";

// With the `utoipa` feature this is also the backend's OpenAPI schema
// `OfflineStamp` (its doc comment is the schema's description).
/// What a punch or ping recorded offline carries, so the server can date it
/// without trusting the phone's wall clock.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OfflineStamp {
    /// The last server time the phone saw. Only a guide: a valid `anchor`
    /// replaces it, and without one the punch is marked unverified.
    pub server_time: DateTime<Utc>,
    /// Time-since-boot elapsed from `server_time` to the event, in ms.
    pub elapsed_ms: i64,
    /// The phone restarted after `server_time`, so `elapsed_ms` means nothing.
    #[serde(default)]
    pub rebooted: bool,
    /// The GPS fix's own satellite time, when the platform gives one (Android's
    /// GNSS provider; iOS gives none).
    #[serde(default)]
    pub gps_time: Option<DateTime<Utc>>,
    /// The `X-Dawam-Time` value of the last response the phone saw (signed).
    #[serde(default)]
    pub anchor: Option<String>,
}

/// A parsed anchor: the server time it carries and its 32-byte tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub ms: i64,
    pub tag: [u8; 32],
}

/// Read an anchor: `v1.<ms>.<64 hex>` (surrounding whitespace ignored).
/// `None` for any other shape. Whether the tag is RIGHT is the server's to say.
pub fn parse_anchor(anchor: &str) -> Option<Anchor> {
    let mut parts = anchor.trim().splitn(3, '.');
    if parts.next()? != ANCHOR_VERSION {
        return None;
    }
    let ms: i64 = parts.next()?.parse().ok()?;
    let hex = parts.next()?;
    if hex.len() != 64 {
        return None;
    }
    let mut tag = [0u8; 32];
    for (i, b) in tag.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(Anchor { ms, tag })
}

/// The anchor for `ms` with `tag`: `v1.<ms>.<hex>`.
pub fn format_anchor(ms: i64, tag: &[u8]) -> String {
    let hex: String = tag.iter().map(|b| format!("{b:02x}")).collect();
    format!("{ANCHOR_VERSION}.{ms}.{hex}")
}

#[cfg(all(test, feature = "utoipa"))]
mod schema {
    use super::OfflineStamp;

    /// The schema the backend's OpenAPI spec carries: the two fields a stamp
    /// cannot go without are required, the rest optional.
    #[test]
    fn the_openapi_schema_requires_what_the_wire_requires() {
        let schema = <OfflineStamp as utoipa::PartialSchema>::schema();
        let v = serde_json::to_value(&schema).unwrap();
        assert_eq!(
            v["required"],
            serde_json::json!(["server_time", "elapsed_ms"])
        );
        let props = v["properties"].as_object().unwrap();
        let mut names: Vec<&str> = props.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "anchor",
                "elapsed_ms",
                "gps_time",
                "rebooted",
                "server_time"
            ]
        );
        assert_eq!(props["server_time"]["format"], "date-time");
        assert_eq!(props["elapsed_ms"]["format"], "int64");
        assert_eq!(<OfflineStamp as utoipa::ToSchema>::name(), "OfflineStamp");
    }
}
