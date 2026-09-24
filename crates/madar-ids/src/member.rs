//! The loyalty member card token a pass barcode encodes: `M` + the 16 bytes of
//! a v4 UUID in unpadded base64url (22 characters).
//!
//! Deliberately NOT the customer's id: a member QR must not be guessable from
//! another's. The server mints it (MadarRust `loyalty::mint_member_token`,
//! which draws the 16 bytes); the till recognises one in a scan buffer
//! (madar-core `loyalty::classify_scan_input`). Pinned by
//! `vectors/member_vectors.json`.

/// `M` + 22 characters.
pub const TOKEN_LEN: usize = 23;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// The token for these 16 (random) bytes.
pub fn member_token(bytes: &[u8; 16]) -> String {
    let mut out = String::with_capacity(TOKEN_LEN);
    out.push('M');
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = chunk.len() + 1; // 3 bytes → 4 chars, 1 byte → 2 chars
        for i in 0..chars {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

/// Whether `s` (already trimmed) has the shape of a member token: exact
/// length, the `M` sentinel and a base64url tail. A half-typed buffer is
/// never mistaken for a whole card.
pub fn is_member_token(s: &str) -> bool {
    s.len() == TOKEN_LEN
        && s.starts_with('M')
        && s[1..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub mod vectors {
    //! Regenerate deliberately:
    //! `MADAR_REGENERATE_MEMBER_VECTORS=1 cargo test -p madar-ids member_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Vectors {
        /// `[16 bytes as hex, token]`
        pub tokens: Vec<(String, String)>,
        /// `[input, is a token]`
        pub shapes: Vec<(String, bool)>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/member_vectors.json")
    }

    fn hex(b: &[u8; 16]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    pub fn generate() -> Vectors {
        let inputs: [[u8; 16]; 4] = [
            [0; 16],
            [0xff; 16],
            *b"\x9f\x1c\x02\xa8\x7b\x33\x4d\x10\x81\x22\xde\xad\xbe\xef\x00\x01",
            [
                0xfb, 0xff, 0xbf, 0x3e, 0x00, 0x7f, 0x80, 0x01, 0xfe, 0xef, 0x10, 0x20, 0x30, 0x40,
                0x50, 0xfc,
            ],
        ];
        let tokens = inputs.iter().map(|b| (hex(b), member_token(b))).collect();
        let shapes = [
            "MAAAAAAAAAAAAAAAAAAAAAA",
            "M_-__-__-__-__-__-__-_A",
            "MAAAAAAAAAAAAAAAAAAAAA",
            "MAAAAAAAAAAAAAAAAAAAAAAA",
            "XAAAAAAAAAAAAAAAAAAAAAA",
            "MAAAAAAAAAAAAAAAAAAAA+A",
            "M٠AAAAAAAAAAAAAAAAAAAA",
            "",
        ]
        .iter()
        .map(|s| (s.to_string(), is_member_token(s)))
        .collect();
        Vectors { tokens, shapes }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn member_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_MEMBER_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vectors = serde_json::from_str(crate::vectors::MEMBER).unwrap();
            assert_eq!(generated, expected, "member tokens drifted");
            assert!(generated.tokens.iter().all(|(_, t)| is_member_token(t)));
        }
    }
}
