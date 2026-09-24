//! Order references.
//!
//! - A device mints `<BRANCH>-<YYMMDD>-<DEVICE>-<RRRR>` ([`device_ref`]): RRRR
//!   is the device's own per-business-day sequence, padded to 4, so the ref is
//!   globally unique with no shared counter and byte-identical at ring-up and
//!   reprint (madar-core `checkout::mint_order_ref`).
//! - A sale that arrives without one gets the server's
//!   `<BRANCH>-<YYMMDD>-<TILL6>-<NNN>` ([`server_ref`]: the till id's first six
//!   hex digits, upper-case; the till's order number padded to 3).
//! - When two devices shared a device code offline, the second ref is stored
//!   with `~` and the first four hex digits of its device id
//!   ([`with_device_suffix`], contract §3 R5).
//! - A device-numbered order's display number is read back out of its ref
//!   ([`display_number_from_ref`]).
//!
//! Moved from MadarRust `orders/handlers.rs` (the server's ref and the suffix)
//! and madar-core `checkout.rs` (the device ref and the read-back). Pinned by
//! `vectors/order_ref_vectors.json`.

/// The ref a device mints for its `seq`-th sale of the business day `yymmdd`.
pub fn device_ref(branch_code: &str, yymmdd: &str, device_code: &str, seq: i64) -> String {
    format!("{branch_code}-{yymmdd}-{device_code}-{seq:04}")
}

/// The server's fallback ref for a sale the till sent none with. `till_hex` is
/// the till id's simple (32 hex digit) form; its first six are used, upper-case.
pub fn server_ref(branch_code: &str, yymmdd: &str, till_hex: &str, order_number: i64) -> String {
    let till6: String = till_hex.chars().take(6).collect::<String>().to_uppercase();
    format!("{branch_code}-{yymmdd}-{till6}-{order_number:03}")
}

/// `order_ref` with the colliding device's `~XXXX` suffix: the first four hex
/// digits of its device id's simple form, upper-case.
pub fn with_device_suffix(order_ref: &str, device_hex: &str) -> String {
    let four: String = device_hex
        .chars()
        .take(4)
        .collect::<String>()
        .to_uppercase();
    format!("{order_ref}~{four}")
}

/// The display number of a device-numbered order: `<device_code>-<n>`, or the
/// bare number without a device code.
pub fn display_number(device_code: &str, order_number: i64) -> String {
    if device_code.is_empty() {
        order_number.to_string()
    } else {
        format!("{device_code}-{order_number}")
    }
}

/// The display number for an order known only by `order_number` and
/// `order_ref`: a device-numbered order's ref is `<BR>-<YYMMDD>-<DEV>-<RRRR>`
/// with `RRRR == order_number`, so the device code is read back from the ref
/// and a `~XXXX` suffix stays on the number (or two sales would read as the
/// same `36B-12`). The server's own ref pads to 3 digits, so a 3-digit tail is
/// never read as a device code.
pub fn display_number_from_ref(order_ref: Option<&str>, order_number: i64) -> String {
    if let Some(r) = order_ref {
        let (base, suffix) = match r.split_once('~') {
            Some((b, s)) if !s.is_empty() => (b, Some(s)),
            _ => (r, None),
        };
        let parts: Vec<&str> = base.split('-').collect();
        if parts.len() == 4
            && parts[3].len() >= 4
            && parts[3].parse::<i64>().ok() == Some(order_number)
        {
            let n = display_number(parts[2], order_number);
            return match suffix {
                Some(s) => format!("{n}~{s}"),
                None => n,
            };
        }
    }
    order_number.to_string()
}

pub mod vectors {
    //! Regenerate deliberately:
    //! `MADAR_REGENERATE_ORDER_REF_VECTORS=1 cargo test -p madar-ids order_ref_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Vectors {
        /// `[branch, yymmdd, device_code, seq, ref]`
        pub device_refs: Vec<(String, String, String, i64, String)>,
        /// `[branch, yymmdd, till_hex, order_number, ref]`
        pub server_refs: Vec<(String, String, String, i64, String)>,
        /// `[ref, device_hex, suffixed]`
        pub suffixed: Vec<(String, String, String)>,
        /// `[order_ref or null, order_number, display]`
        pub displays: Vec<(Option<String>, i64, String)>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/order_ref_vectors.json")
    }

    pub fn generate() -> Vectors {
        let mut device_refs = Vec::new();
        for &(b, d, dev, seq) in &[
            ("HQ", "260924", "36B", 1),
            ("HQ", "260924", "36B", 12),
            ("MAADI", "261231", "A", 9999),
            ("MAADI", "261231", "A", 10000),
            ("X", "270101", "", 7),
        ] {
            device_refs.push((
                b.into(),
                d.into(),
                dev.into(),
                seq,
                device_ref(b, d, dev, seq),
            ));
        }
        let mut server_refs = Vec::new();
        for &(b, d, till, n) in &[
            ("HQ", "260924", "eaf7cfecade953f79dd928a1ca7ef50b", 1),
            ("HQ", "260924", "eaf7cfecade953f79dd928a1ca7ef50b", 999),
            ("HQ", "260924", "eaf7cfecade953f79dd928a1ca7ef50b", 1000),
            ("VEC", "260101", "0e5d40c820505c55b1e2cdd166356911", 42),
        ] {
            server_refs.push((
                b.into(),
                d.into(),
                till.into(),
                n,
                server_ref(b, d, till, n),
            ));
        }
        let suffixed = [
            ("HQ-260924-36B-0012", "b3b0fe1057b35b73a8e72b99f97e30aa"),
            ("HQ-260924-36B-0001", "0f29b7a3e551587b8d966ed9a82d304a"),
        ]
        .iter()
        .map(|&(r, d)| (r.to_string(), d.to_string(), with_device_suffix(r, d)))
        .collect();
        let displays = [
            (Some("HQ-260924-36B-0012"), 12),
            (Some("HQ-260924-36B-0012~B3B0"), 12),
            (Some("HQ-260924-36B-0012~"), 12),
            (Some("HQ-260924-36B-0013"), 12),
            (Some("HQ-260924-EAF7CF-012"), 12),
            (Some("HQ-260924-EAF7CF-1000"), 1000),
            (Some("garbage"), 5),
            (Some("A-B-C-D-0005"), 5),
            (None, 77),
        ]
        .iter()
        .map(|&(r, n)| (r.map(str::to_string), n, display_number_from_ref(r, n)))
        .collect();
        Vectors {
            device_refs,
            server_refs,
            suffixed,
            displays,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn order_ref_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_ORDER_REF_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vectors = serde_json::from_str(crate::vectors::ORDER_REF).unwrap();
            assert_eq!(generated, expected, "order refs drifted");
        }
    }
}
