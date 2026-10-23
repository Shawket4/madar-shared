//! Only designs that can really render.
//!
//! The editor runs this on every change and the server runs it before it
//! saves or publishes, so the two cannot disagree about what is allowed. An
//! ERROR blocks saving; a WARNING is shown and does not.

use serde::{Deserialize, Serialize};

use crate::design::{CardDesign, ElementKind, FieldSource, GroundKind, DESIGN_VERSION};
use crate::grid::{Canvas, Cell, APPLE_PRIMARY_ZONE, FOOTER_CELL};
use crate::render::{count_font, stamp_grid, text_fits};
use crate::tracks::{is_hex, Mode, TrackRef};

/// At most this many elements on a strip.
pub const MAX_ELEMENTS: usize = 12;
/// At most this many stamps in one stamps element; past it, use a bar.
pub const MAX_STAMPS: i32 = 20;
/// A stamp must be at least this wide on Apple's 1× strip, in points.
pub const MIN_STAMP_PT: f64 = 12.0;
/// Baked figures smaller than this are unreadable on a phone.
pub const MIN_COUNT_PT: f64 = 10.0;
/// A text element's words, per language.
pub const TEXT_MAX: usize = 30;
/// The foreground on the background: WCAG's large-text bar (the OS writes the
/// balance in it, large).
pub const MIN_CONTRAST: f64 = 3.0;
/// Apple's header fields share the top line with the logo.
pub const MAX_HEADER: usize = 2;
/// Apple allows four secondary and auxiliary fields together; one auxiliary
/// slot is kept for the notice a message puts on the card.
pub const MAX_SECONDARY_AND_AUXILIARY: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// One thing wrong, by a stable code the dashboards translate, and where.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Problem {
    pub code: String,
    /// `elements[2]`, `apple.header`, `colours.foreground`, `tracks[1]`, …
    pub path: String,
    pub severity: Severity,
}

impl Problem {
    pub fn error(code: &str, path: &str) -> Problem {
        Problem {
            code: code.into(),
            path: path.into(),
            severity: Severity::Error,
        }
    }

    pub fn warning(code: &str, path: &str) -> Problem {
        Problem {
            code: code.into(),
            path: path.into(),
            severity: Severity::Warning,
        }
    }
}

/// True when nothing in `problems` blocks saving.
pub fn is_valid(problems: &[Problem]) -> bool {
    problems.iter().all(|p| p.severity == Severity::Warning)
}

/// What the design is checked against: the shop's programme as it stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Ctx {
    pub mode: Mode,
    /// Every track with its card size (track 1's is the programme's).
    pub tracks: Vec<TrackRef>,
    /// The shop has a card photograph uploaded.
    pub has_photo: bool,
}

impl Ctx {
    pub fn track(&self, position: u8) -> Option<TrackRef> {
        self.tracks.iter().copied().find(|t| t.position == position)
    }
}

/// WCAG relative luminance of `#RRGGBB`.
pub fn luminance(hex: &str) -> Option<f64> {
    if !is_hex(hex) {
        return None;
    }
    let ch = |i: usize| -> Option<f64> {
        let v = f64::from(u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()?) / 255.0;
        Some(if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        })
    };
    Some(0.2126 * ch(1)? + 0.7152 * ch(3)? + 0.0722 * ch(5)?)
}

/// The WCAG contrast ratio of two colours.
pub fn contrast(a: &str, b: &str) -> Option<f64> {
    let (la, lb) = (luminance(a)?, luminance(b)?);
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    Some((hi + 0.05) / (lo + 0.05))
}

/// The problems with `design` for the shop described by `ctx`; empty (or
/// warnings only) when it may be saved and published.
pub fn validate(design: &CardDesign, ctx: &Ctx) -> Vec<Problem> {
    let mut out = Vec::new();
    if design.version != DESIGN_VERSION {
        out.push(Problem::error("design_version", "version"));
        return out;
    }

    // Colours.
    let c = &design.colours;
    for (path, v) in [
        ("colours.background", &c.background),
        ("colours.foreground", &c.foreground),
        ("colours.label", &c.label),
    ] {
        if !is_hex(v) {
            out.push(Problem::error("colour_invalid", path));
        }
    }
    if let Some(r) = contrast(&c.foreground, &c.background) {
        if r < MIN_CONTRAST {
            out.push(Problem::error("contrast_low", "colours.foreground"));
        }
    }
    if let Some(r) = contrast(&c.label, &c.background) {
        if r < MIN_CONTRAST {
            out.push(Problem::warning("label_contrast_low", "colours.label"));
        }
    }

    // Ground.
    match design.ground.kind {
        GroundKind::Photo if !ctx.has_photo => {
            out.push(Problem::error("photo_missing", "ground"));
        }
        GroundKind::Pattern if design.ground.pattern.is_none() => {
            out.push(Problem::error("pattern_missing", "ground"));
        }
        _ => {}
    }

    // Elements on the grid.
    if design.elements.len() > MAX_ELEMENTS {
        out.push(Problem::error("too_many_elements", "elements"));
    }
    if design.elements.is_empty() {
        out.push(Problem::warning("strip_empty", "elements"));
    }
    let canvas = Canvas::apple(1);
    let primary = design.apple.primary.is_some();
    for (i, e) in design.elements.iter().enumerate() {
        let path = format!("elements[{i}]");
        if !e.cell.within() {
            out.push(Problem::error("outside_grid", &path));
            continue;
        }
        if e.cell.overlaps(&FOOTER_CELL) {
            out.push(Problem::error("covers_footer", &path));
        }
        if primary && e.cell.overlaps(&APPLE_PRIMARY_ZONE) {
            out.push(Problem::error("under_primary_field", &path));
        }
        for (j, other) in design.elements.iter().enumerate().take(i) {
            if other.cell.within() && e.cell.overlaps(&other.cell) {
                out.push(Problem::error("overlap", &format!("{path}~elements[{j}]")));
            }
        }
        let rect = canvas.rect(e.cell, false);
        match &e.kind {
            ElementKind::Stamps { track } => match ctx.track(*track) {
                None => out.push(Problem::error("track_unknown", &path)),
                Some(t) => {
                    if ctx.mode == Mode::Points && *track == 1 {
                        out.push(Problem::error("stamps_on_points", &path));
                    } else if t.size > MAX_STAMPS {
                        out.push(Problem::error("too_many_stamps", &path));
                    } else if stamp_grid(t.size, rect).diameter < MIN_STAMP_PT {
                        out.push(Problem::error("stamps_too_small", &path));
                    }
                }
            },
            ElementKind::Bar { track } => {
                if ctx.track(*track).is_none() {
                    out.push(Problem::error("track_unknown", &path));
                }
                if e.cell.w < 3 {
                    out.push(Problem::error("bar_too_short", &path));
                }
            }
            ElementKind::Count { track } => match ctx.track(*track) {
                None => out.push(Problem::error("track_unknown", &path)),
                Some(t) => {
                    if count_font(t.size, rect) < MIN_COUNT_PT {
                        out.push(Problem::error("count_too_small", &path));
                    }
                }
            },
            ElementKind::Text { en, ar, size, .. } => {
                let (en_n, ar_n) = (en.trim().chars().count(), ar.trim().chars().count());
                if en_n == 0 && ar_n == 0 {
                    out.push(Problem::error("text_empty", &path));
                }
                if en_n > TEXT_MAX || ar_n > TEXT_MAX {
                    out.push(Problem::error("text_long", &path));
                } else if !text_fits(en.trim(), false, *size, rect)
                    || !text_fits(ar.trim(), true, *size, rect)
                {
                    out.push(Problem::error("text_does_not_fit", &path));
                }
                if [en, ar].iter().any(|s| s.chars().any(char::is_control)) {
                    out.push(Problem::error("text_control", &path));
                }
            }
            ElementKind::Logo => {
                if e.cell.h < 2 || e.cell.w < 2 {
                    out.push(Problem::error("logo_too_small", &path));
                }
            }
        }
    }

    // Fields.
    let a = &design.apple;
    if a.header.len() > MAX_HEADER {
        out.push(Problem::error("too_many_header_fields", "apple.header"));
    }
    if a.secondary.len() + a.auxiliary.len() > MAX_SECONDARY_AND_AUXILIARY {
        out.push(Problem::error("too_many_fields", "apple.secondary"));
    }
    let sources: Vec<(String, FieldSource)> = a
        .header
        .iter()
        .map(|f| ("apple.header".to_string(), *f))
        .chain(a.primary.map(|f| ("apple.primary".to_string(), f)))
        .chain(
            a.secondary
                .iter()
                .map(|f| ("apple.secondary".to_string(), *f)),
        )
        .chain(
            a.auxiliary
                .iter()
                .map(|f| ("apple.auxiliary".to_string(), *f)),
        )
        .chain([("google.points".to_string(), design.google.points)])
        .chain(
            design
                .google
                .secondary
                .map(|f| ("google.secondary".to_string(), f)),
        )
        .collect();
    for (path, f) in &sources {
        if let Some(t) = f.track() {
            if ctx.track(t).is_none() {
                out.push(Problem::error("track_unknown", path));
            }
        }
    }
    // Every track the shop runs should be visible somewhere on the card.
    for (i, t) in ctx.tracks.iter().enumerate() {
        let on_strip = design.elements.iter().any(|e| match e.kind {
            ElementKind::Stamps { track }
            | ElementKind::Bar { track }
            | ElementKind::Count { track } => track == t.position,
            _ => false,
        });
        let in_fields = design
            .field_sources()
            .iter()
            .any(|f| f.track() == Some(t.position));
        if !on_strip && !in_fields {
            out.push(Problem::warning("track_hidden", &format!("tracks[{i}]")));
        }
    }
    out
}

/// A cell the footer and the primary zone leave free, for the editor's
/// "add element" (the first free spot of `w`×`h`, reading order).
pub fn first_free(design: &CardDesign, w: u8, h: u8) -> Option<Cell> {
    for row in 0..crate::grid::ROWS {
        for col in 0..crate::grid::COLS {
            let c = Cell { col, row, w, h };
            if !c.within() || c.overlaps(&FOOTER_CELL) {
                continue;
            }
            if design.apple.primary.is_some() && c.overlaps(&APPLE_PRIMARY_ZONE) {
                continue;
            }
            if design.elements.iter().any(|e| e.cell.overlaps(&c)) {
                continue;
            }
            return Some(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_is_wcags() {
        let r = contrast("#FFFFFF", "#000000").unwrap();
        assert!((r - 21.0).abs() < 1e-9);
        assert!((contrast("#777777", "#777777").unwrap() - 1.0).abs() < 1e-9);
        assert!(contrast("#12345", "#000000").is_none());
    }
}
