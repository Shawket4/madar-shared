//! Inside or outside a branch's fence.
//!
//! The server refuses a punch whose distance is above the radius (MadarRust
//! `staff/attendance.rs` `check_geofence`); the phone shows "inside" at or
//! under it (madar-core `dawam.rs`). The distance is the server's
//! `geo::osrm::haversine_meters` (the `atan2` form; the phone's `asin` form
//! differed in the last bits).

/// Mean Earth radius, metres.
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// A branch's radius when none is set.
pub const DEFAULT_RADIUS_M: i64 = 200;

/// Great-circle distance in metres between `(lat, lng)` points.
pub fn haversine_m(from: (f64, f64), to: (f64, f64)) -> f64 {
    let lat1 = from.0.to_radians();
    let lat2 = to.0.to_radians();
    let dlat = (to.0 - from.0).to_radians();
    let dlng = (to.1 - from.1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlng / 2.0).sin().powi(2);
    EARTH_RADIUS_M * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

/// The radius a punch is judged against: unset is 200 m, below zero is 0 m,
/// and 0 is 0 m (DW2 — the phone used to read 0 as 200).
pub fn effective_radius(geo_radius_meters: Option<i64>) -> i64 {
    geo_radius_meters.unwrap_or(DEFAULT_RADIUS_M).max(0)
}

/// Whether `distance_m` is inside a fence of `radius_m` (on the line is inside).
pub fn inside(distance_m: f64, radius_m: i64) -> bool {
    distance_m <= radius_m as f64
}

/// Whether coordinates are on the globe at all.
pub fn in_range(lat: f64, lng: f64) -> bool {
    (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng)
}
