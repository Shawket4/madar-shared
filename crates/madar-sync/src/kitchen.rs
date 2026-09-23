//! Deterministic kitchen ids. A fire's kitchen ticket and line ids are
//! DERIVED from the round's client idempotency key, so a device that fires
//! offline projects the ticket to its KDS with the ids the server will mint,
//! and the projection dedups against the server feed by id on reconnect.

use uuid::Uuid;

/// Fixed namespace for deterministic kitchen ids. "madar_kitchen_ns" as bytes.
pub const KITCHEN_ID_NS: Uuid = Uuid::from_u128(0x6d61_6461_725f_6b69_7463_6865_6e5f_6e73);

/// The kitchen-ticket id a fire will create, from the round's CLIENT
/// idempotency key.
pub fn derive_kitchen_ticket_id(round_idem: Uuid) -> Uuid {
    Uuid::new_v5(&KITCHEN_ID_NS, round_idem.as_bytes())
}

/// The kitchen-line id for the line at `index` within its (derived) kitchen
/// ticket.
pub fn derive_kitchen_item_id(kitchen_ticket_id: Uuid, index: usize) -> Uuid {
    Uuid::new_v5(&kitchen_ticket_id, &(index as u32).to_le_bytes())
}

#[cfg(test)]
mod id_tests {
    /// Moved from MadarRust `kitchen::id_tests` (madar-core pins the same ids in
    /// `kds::tests::kitchen_id_derivation_matches_backend`, through its string
    /// wrappers).
    #[test]
    fn kitchen_id_derivation_is_pinned() {
        let kt = super::derive_kitchen_ticket_id(uuid::Uuid::nil());
        assert_eq!(kt.to_string(), "e9b2a598-f8ea-5510-8382-927f5e218fff");
        assert_eq!(
            super::derive_kitchen_item_id(kt, 0).to_string(),
            "0b40ac60-7d15-5bef-858f-849b09850f69"
        );
        assert_eq!(
            super::derive_kitchen_item_id(kt, 1).to_string(),
            "50cef3f1-fced-57d3-bb6c-daa1c917a8b6"
        );
    }
}
