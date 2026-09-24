//! Presence while on shift: what the server and the staff app both judge a
//! phone's report by (DW6).

/// At or under this battery percentage the employee is told to charge, and a
/// silence reads "phone likely died" (CL-12). MadarRust `staff::dawam::
/// presence::LOW_BATTERY`, and the staff app's "charge your phone" banner.
pub const LOW_BATTERY: i16 = 15;

/// Whether a reported battery percentage is low.
pub fn low_battery(percent: i64) -> bool {
    percent <= i64::from(LOW_BATTERY)
}

#[cfg(test)]
mod tests {
    #[test]
    fn fifteen_is_low_sixteen_is_not() {
        assert!(super::low_battery(15));
        assert!(!super::low_battery(16));
        assert!(super::low_battery(0));
    }
}
