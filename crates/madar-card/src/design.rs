//! The card design an owner builds in the dashboard (version 1).
//!
//! A wallet pass is not a canvas. Apple and Google draw every field in the
//! system font and the barcode themselves; what a shop controls is the
//! colours, WHICH data each OS-drawn field shows, and the images — Apple's
//! strip and Google's hero, which the server renders. So a design is exactly
//! those three things:
//!
//! - [`Colours`] and the [`Ground`] under the strip;
//! - [`Element`]s placed on the [`crate::grid`] — stamps, bars and counts of a
//!   track, static text, the logo — which become the strip and the hero;
//! - [`AppleFields`] and [`GoogleFields`]: which [`FieldSource`] each slot shows.
//!
//! Nothing here is a free-form drawing: every element is a kind the server
//! knows how to draw, and [`crate::validate`] refuses what would not render.

use serde::{Deserialize, Serialize};

use crate::grid::Cell;

/// The schema's version. A design of another version is refused, never
/// guessed at.
pub const DESIGN_VERSION: u32 = 1;

/// The whole design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CardDesign {
    pub version: u32,
    /// The template it started from (informational: `classic`, `stamps`, …).
    #[serde(default)]
    pub template: String,
    pub colours: Colours,
    pub ground: Ground,
    /// What the strip (Apple) and the hero (Google) show, on the grid.
    #[serde(default)]
    pub elements: Vec<Element>,
    pub stamp: StampStyle,
    pub apple: AppleFields,
    pub google: GoogleFields,
}

/// The pass's colours, `#RRGGBB`. Apple writes field values in `foreground`
/// and labels in `label` on `background`; Google takes `background` only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Colours {
    pub background: String,
    pub foreground: String,
    pub label: String,
}

/// What lies under the elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum GroundKind {
    /// The card's background colour.
    Colour,
    /// The shop's card photograph (Settings › Brand), veiled at the bottom so
    /// the bar and the footer read on it.
    Photo,
    /// A quiet pattern in the foreground colour over the background.
    Pattern,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    Dots,
    Lines,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Ground {
    pub kind: GroundKind,
    /// Which pattern, when `kind` is `pattern`.
    #[serde(default)]
    pub pattern: Option<Pattern>,
}

/// One thing on the strip, and where.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Element {
    pub kind: ElementKind,
    pub cell: Cell,
}

/// The kinds of thing a strip can hold. Each is drawn by [`crate::render`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementKind {
    /// A track's stamps, filled up to its progress on the current card.
    Stamps { track: u8 },
    /// A track's progress as a bar (segmented up to 12 steps).
    Bar { track: u8 },
    /// A track's progress as figures, "3/8", baked with the bundled font.
    Count { track: u8 },
    /// Static words, EN and AR, baked with the bundled font. The Arabic is
    /// drawn on the Arabic strip, the English on every other; one of them may
    /// be empty (the other is used).
    Text {
        #[serde(default)]
        en: String,
        #[serde(default)]
        ar: String,
        size: TextSize,
        align: TextAlign,
    },
    /// The shop's logo (Settings › Brand), else the Rewards present.
    Logo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    Small,
    Medium,
    Large,
}

impl TextSize {
    /// The font size in Apple points at 1×.
    pub fn points(self) -> f64 {
        match self {
            TextSize::Small => 12.0,
            TextSize::Medium => 16.0,
            TextSize::Large => 22.0,
        }
    }
}

/// Alignment in reading order (`start` is the right edge on an Arabic pass).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Start,
    Centre,
    End,
}

/// The stamp's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum StampShape {
    Circle,
    Square,
    Star,
    Heart,
    Cup,
    Gift,
    Check,
}

/// How a stamp not yet earned looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum EmptyStyle {
    /// Its outline.
    Outline,
    /// The shape, faint.
    Faint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StampStyle {
    pub shape: StampShape,
    pub empty: EmptyStyle,
}

/// What an OS-drawn field shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldSource {
    /// The track's balance, as a number (track 1: points or stamps).
    Balance { track: u8 },
    /// Where the track is on its current card: the stamp row, or "3 / 8".
    Progress { track: u8 },
    /// The rewards the track has earned (no row while there are none).
    Rewards { track: u8 },
    /// The member's name.
    Member,
}

impl FieldSource {
    /// The track it reads, if any.
    pub fn track(self) -> Option<u8> {
        match self {
            FieldSource::Balance { track }
            | FieldSource::Progress { track }
            | FieldSource::Rewards { track } => Some(track),
            FieldSource::Member => None,
        }
    }
}

/// Apple's store-card field groups. The back of the card is not designed: it
/// carries how it works, branches, terms and links, as today.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AppleFields {
    /// Top right beside the logo, and the only thing a stacked wallet shows.
    #[serde(default)]
    pub header: Vec<FieldSource>,
    /// Written over the strip's leading top two thirds.
    #[serde(default)]
    pub primary: Option<FieldSource>,
    #[serde(default)]
    pub secondary: Vec<FieldSource>,
    #[serde(default)]
    pub auxiliary: Vec<FieldSource>,
}

/// Google's two slots on the face of the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GoogleFields {
    /// `loyaltyPoints`, the largest text on the card.
    pub points: FieldSource,
    /// `secondaryLoyaltyPoints`.
    #[serde(default)]
    pub secondary: Option<FieldSource>,
}

impl CardDesign {
    /// Every field source the design shows, on either wallet.
    pub fn field_sources(&self) -> Vec<FieldSource> {
        let mut out: Vec<FieldSource> = self.apple.header.clone();
        out.extend(self.apple.primary);
        out.extend(self.apple.secondary.iter().copied());
        out.extend(self.apple.auxiliary.iter().copied());
        out.push(self.google.points);
        out.extend(self.google.secondary);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_shape() {
        let e = Element {
            kind: ElementKind::Stamps { track: 2 },
            cell: Cell {
                col: 0,
                row: 1,
                w: 12,
                h: 2,
            },
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"kind": {"type": "stamps", "track": 2}, "cell": {"col": 0, "row": 1, "w": 12, "h": 2}})
        );
        let f: FieldSource = serde_json::from_value(serde_json::json!({"type": "member"})).unwrap();
        assert_eq!(f, FieldSource::Member);
    }
}
