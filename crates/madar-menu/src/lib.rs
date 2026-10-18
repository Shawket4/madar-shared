//! # madar-menu
//!
//! A whole menu as one value, for the self-serve import (Madar
//! `MENU_IMPORT_PLAN.md`).
//!
//! - [`bulk`]: [`BulkMenu`], what a client sends to `POST /menu/bulk`, and
//!   [`validate`], the checks the server runs before it writes anything (refs
//!   resolve, no duplicates, limits that can be met, prices and sizes within
//!   bounds). The client runs the same checks for its preview.
//! - [`confirm`]: the sentence an owner types to replace a whole menu, shown
//!   by the dashboards and checked by the server.
//! - [`foodics`] (feature `foodics`): builds a [`BulkMenu`] from the CSV files
//!   a Foodics console exports, with a warning for every row it had to skip or
//!   change. No I/O: the caller reads the files and passes their text.
//!
//! The backend never turns on `foodics`; it only ever receives JSON.
//!
//! Pinned by `vectors/menu_import_vectors.json` ([`vectors`]).

pub mod bulk;
pub mod confirm;
#[cfg(feature = "foodics")]
pub mod foodics;
pub mod vectors;

pub use bulk::{
    validate, BulkCategory, BulkFamily, BulkGroup, BulkItem, BulkLink, BulkMenu, BulkOption,
    BulkOptional, BulkRefusal, BulkSize, BulkSummary, Limits, LIMITS, VERSION,
};
pub use confirm::{confirms_replace, replace_sentence};
