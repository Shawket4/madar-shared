//! The offline PIN (argon2id) a device checks a typed PIN against with no
//! network (discovery A5).
//!
//! The server derives the PHC string (MadarRust `auth::offline::
//! hash_offline_pin`, which needs randomness for the salt and stays there) and
//! ships it in the offline-auth bundle; the device verifies against it
//! (madar-core `session.rs`). The parameters ride in the PHC string, so the
//! verify is the same code on both sides. Pinned by [`TEST_PHC`].

use argon2::password_hash::PasswordHash;
use argon2::{Argon2, PasswordVerifier};

/// Whether `pin` matches the argon2id PHC string `phc`. A string that is not
/// a PHC hash matches nothing.
pub fn verify_offline_pin(pin: &str, phc: &str) -> bool {
    PasswordHash::new(phc)
        .map(|h| {
            Argon2::default()
                .verify_password(pin.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

/// The PHC string the server's `hash_offline_pin` derives (argon2id,
/// `Argon2::default()`) for PIN `1234` under the fixed salt
/// `madar-shared-pin`: the one string both sides' tests verify against.
pub const TEST_PHC: &str = "$argon2id$v=19$m=19456,t=2,p=1$bWFkYXItc2hhcmVkLXBpbg$2aICuKC8kLGwQ16E9tSy8T1jQotZfajJdQBJAGqIcJU";
/// The PIN [`TEST_PHC`] was derived from.
pub const TEST_PIN: &str = "1234";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_phc_string_verifies_its_pin_and_nothing_else() {
        assert!(TEST_PHC.starts_with("$argon2id$"));
        assert!(verify_offline_pin(TEST_PIN, TEST_PHC));
        assert!(!verify_offline_pin("9999", TEST_PHC));
        assert!(!verify_offline_pin(TEST_PIN, "not a hash"));
    }
}
