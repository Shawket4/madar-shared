//! The vector file this crate is pinned by, as bytes.
//!
//! `menu_import_vectors.json`: [`crate::foodics::parse`] over Foodics exports
//! and [`crate::validate`] over bulk menus. The inputs are hand-written
//! (`src/foodics/tests.rs`); the expected answers are this crate's, written by
//! `MADAR_REGENERATE_MENU_VECTORS=1 cargo test -p madar-menu --features foodics
//! menu_import_vectors` and reviewed by hand. The web dashboard (through
//! madar-web) and the Flutter dashboard (through its core) answer every case
//! the same.
pub const MENU_IMPORT: &str = include_str!("../vectors/menu_import_vectors.json");
