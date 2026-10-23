//! # madar-card
//!
//! The Rewards by Madar card as data, for the dashboard's card designer, the
//! server that renders the wallet passes, and (later) the POS core.
//!
//! - [`tracks`]: a card holds 1 to 4 **stamp tracks** ("One card, several
//!   rewards"): coffee on one row, sandwiches on another. Track 1 is the
//!   programme's own counter, exactly as before; tracks 2–4 are new. Their
//!   validation and per-track card arithmetic ([`tracks::track_card`], the
//!   card's own rule from `madar-loyalty`).
//! - [`design`]: the card design an owner builds — colours, what fills the
//!   strip (stamps, bars, counts, text, logo) placed on a snapping [`grid`],
//!   which data each wallet's OS-drawn fields show — and [`templates`], the
//!   ready-made starting points. The default design is today's look.
//! - [`validate`]: the editor may only save a design that can really render:
//!   inside the grid, no overlaps, nothing under the text Apple draws on the
//!   strip, stamps big enough to see, text that fits its box, a readable
//!   foreground, field counts each wallet accepts.
//! - [`render`]: where every element lands on Apple's strip and Google's hero,
//!   and the strip as an SVG — the server rasterises it (with its bundled
//!   fonts), the dashboard shows it live. Raster layers (the photograph, the
//!   logo, the footer) are the caller's.
//! - [`text`]: the stamp row as text for the OS-drawn fields (the passes'
//!   stepper rule).
//! - [`audience`]: who a dashboard message goes to (all members, or rules
//!   over balance, last visit, birthday month, …), checked the same on both
//!   sides.
//! - [`message`]: what a dashboard message may say and how often a shop may
//!   send one.
//!
//! Pinned by `vectors/card_design_vectors.json` and
//! `vectors/card_rules_vectors.json` ([`vectors`]), hand-written inputs with
//! the answers reviewed by hand.

pub mod audience;
pub mod design;
pub mod grid;
pub mod message;
pub mod render;
pub mod templates;
pub mod text;
pub mod tracks;
pub mod validate;
pub mod vectors;

pub use design::{
    AppleFields, CardDesign, Colours, Element, ElementKind, EmptyStyle, FieldSource, GoogleFields,
    Ground, GroundKind, Pattern, StampShape, StampStyle, TextAlign, TextSize, DESIGN_VERSION,
};
pub use grid::{Cell, Store, COLS, ROWS};
pub use tracks::{Track, TrackRef};
pub use validate::{validate, Ctx, Problem, Severity};
