//! Where every element lands, and the strip as an SVG.
//!
//! One drawing for both renderers: the server rasterises [`svg`] with resvg
//! (only the bundled Manrope and Cairo faces loaded) over the photograph, and
//! the dashboard shows the same SVG live. Raster layers are the caller's: the
//! photograph (`opts.photo_href`, or composited beneath by the server), the
//! logo and the footer (an `<image>` when the caller passes an href, else only
//! their place, [`layout`], for the server to composite).
//!
//! Text is measured with fixed advance estimates of the two faces (as the QR
//! card's layout does), so validation and drawing agree on what fits.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

use crate::design::{
    CardDesign, ElementKind, EmptyStyle, GroundKind, Pattern, StampShape, TextAlign, TextSize,
};
use crate::grid::{Canvas, Rect, APPLE_PRIMARY_ZONE, FOOTER_CELL};
use crate::tracks::track_card;

/// Manrope SemiBold's average advance (figures and lower case), as a share of
/// the em. Slightly generous, so "fits" here fits on the card.
pub const LATIN_ADVANCE: f64 = 0.6;
/// Cairo Medium's, for Arabic words.
pub const ARABIC_ADVANCE: f64 = 0.52;
/// The footer files at 1× (`rewards-by-madar-*-12.png`, `mukafaat-min-madar-*-19.png`).
pub const FOOTER_LATIN_1X: (f64, f64) = (163.0, 19.0);
pub const FOOTER_ARABIC_1X: (f64, f64) = (108.0, 19.0);

/// A track as the strip draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct TrackState {
    pub position: u8,
    pub size: i32,
    pub balance: i32,
    #[serde(default)]
    pub colour: Option<String>,
    #[serde(default)]
    pub shape: Option<StampShape>,
}

/// The member's card as the strip shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct CardState {
    pub tracks: Vec<TrackState>,
    /// The Arabic strip (`ar.lproj`): mirrored, Arabic words.
    pub arabic: bool,
}

/// What the caller draws itself, and where its pictures are.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct SvgOpts {
    /// The photograph under a `photo` ground (the dashboard); `None` leaves
    /// that ground transparent for the server to composite beneath.
    #[serde(default)]
    pub photo_href: Option<String>,
    #[serde(default)]
    pub logo_href: Option<String>,
    #[serde(default)]
    pub footer_href: Option<String>,
    /// Approximate the server's measured scrim under Apple's primary field (the
    /// dashboard's live preview; the server scrims the photograph itself).
    #[serde(default)]
    pub scrim_preview: bool,
    /// Apple's primary field is written on this strip (Google's hero has none).
    #[serde(default)]
    pub primary_field: bool,
}

/// How a track's stamps are arranged in their box.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct StampGrid {
    pub rows: u32,
    pub per_row: u32,
    /// Each stamp's width, in the canvas's pixels.
    pub diameter: f64,
}

/// The arrangement of `n` stamps in `rect` that makes them biggest (one to
/// four rows).
pub fn stamp_grid(n: i32, rect: Rect) -> StampGrid {
    let n = u32::try_from(n.max(1)).unwrap_or(1);
    let mut best = StampGrid {
        rows: 1,
        per_row: n,
        diameter: 0.0,
    };
    for rows in 1..=4u32.min(n) {
        let per_row = n.div_ceil(rows);
        let d = (rect.w / f64::from(per_row)).min(rect.h / f64::from(rows)) * 0.82;
        if d > best.diameter + 1e-9 {
            best = StampGrid {
                rows,
                per_row,
                diameter: d,
            };
        }
    }
    best
}

/// The font size a track's "progress/size" is drawn at in `rect`: as tall as
/// the box allows, narrowed to fit its widest figures.
pub fn count_font(size: i32, rect: Rect) -> f64 {
    let widest = format!("{size}/{size}").chars().count();
    #[allow(clippy::cast_precision_loss)] // a handful of characters
    let by_width = rect.w / (widest as f64 * LATIN_ADVANCE);
    (rect.h * 0.72).min(by_width)
}

/// Whether `text` at `size` fits `rect` on Apple's 1× strip (`rect` measured
/// at 1×).
pub fn text_fits(text: &str, arabic: bool, size: TextSize, rect: Rect) -> bool {
    if text.is_empty() {
        return true;
    }
    let fs = size.points();
    let advance = if arabic {
        ARABIC_ADVANCE
    } else {
        LATIN_ADVANCE
    };
    #[allow(clippy::cast_precision_loss)] // at most TEXT_MAX characters
    let w = text.chars().count() as f64 * advance * fs;
    w <= rect.w && fs * 1.2 <= rect.h
}

/// What [`layout`] places, for a caller that composites raster layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum Placed {
    Element,
    Logo,
    Footer,
    PrimaryZone,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct Placement {
    pub what: Placed,
    /// The element's index, for `element` and `logo`.
    pub index: Option<usize>,
    pub rect: Rect,
}

/// Every element's rectangle on `canvas`, then the footer's, then (when
/// Apple writes a primary field on the strip) the zone it writes in.
pub fn layout(
    design: &CardDesign,
    canvas: Canvas,
    arabic: bool,
    primary_field: bool,
) -> Vec<Placement> {
    let mut out: Vec<Placement> = design
        .elements
        .iter()
        .enumerate()
        .filter(|(_, e)| e.cell.within())
        .map(|(i, e)| Placement {
            what: if e.kind == ElementKind::Logo {
                Placed::Logo
            } else {
                Placed::Element
            },
            index: Some(i),
            rect: canvas.rect(e.cell, arabic),
        })
        .collect();
    out.push(Placement {
        what: Placed::Footer,
        index: None,
        rect: footer_rect(canvas, arabic),
    });
    if primary_field {
        out.push(Placement {
            what: Placed::PrimaryZone,
            index: None,
            rect: canvas.rect(APPLE_PRIMARY_ZONE, arabic),
        });
    }
    out
}

/// The footer's own box: its file's size at the canvas's scale, at the
/// leading edge of the footer cell, centred down it.
pub fn footer_rect(canvas: Canvas, arabic: bool) -> Rect {
    let cell = canvas.rect(FOOTER_CELL, arabic);
    let (fw, fh) = if arabic {
        FOOTER_ARABIC_1X
    } else {
        FOOTER_LATIN_1X
    };
    let (w, h) = (fw * canvas.scale, fh * canvas.scale);
    let x = if arabic { cell.x + cell.w - w } else { cell.x };
    Rect {
        x,
        y: cell.y + (cell.h - h) / 2.0,
        w,
        h,
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o
}

fn n(v: f64) -> String {
    // Two decimals is a hundredth of a pixel: byte-stable output, no noise.
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        format!("{r:.0}")
    } else {
        format!("{r}")
    }
}

/// A stamp's outline in a 24×24 box.
fn shape_path(shape: StampShape) -> &'static str {
    match shape {
        StampShape::Circle => "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z",
        StampShape::Square => "M6 2h12a4 4 0 0 1 4 4v12a4 4 0 0 1-4 4H6a4 4 0 0 1-4-4V6a4 4 0 0 1 4-4z",
        StampShape::Star => {
            "M12 1.8l3.1 6.4 7 1-5.1 4.9 1.2 7L12 17.8l-6.2 3.3 1.2-7L1.9 9.2l7-1z"
        }
        StampShape::Heart => {
            "M12 21.2l-1.4-1.3C5.4 15.2 2 12.1 2 8.3 2 5.2 4.4 2.8 7.5 2.8c1.7 0 3.4.8 4.5 2.1 1.1-1.3 2.8-2.1 4.5-2.1 3.1 0 5.5 2.4 5.5 5.5 0 3.8-3.4 6.9-8.6 11.6z"
        }
        StampShape::Cup => {
            "M3 7h14v6a6 6 0 0 1-6 6H9a6 6 0 0 1-6-6zM17 9h1.5a3 3 0 0 1 0 6H17v-2h1.5a1 1 0 0 0 0-2H17zM2 21h17v1.5H2z"
        }
        StampShape::Gift => {
            "M3 9h18v4H3zM4.5 13h15v9h-15zM11 9h2v13h-2zM12 9c-1.5-3-5-4.5-6-2.5S8 9 12 9zM12 9c1.5-3 5-4.5 6-2.5S16 9 12 9z"
        }
        StampShape::Check => "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z",
    }
}

/// The track's colour, else the foreground.
fn ink<'a>(t: &'a TrackState, fg: &'a str) -> &'a str {
    t.colour
        .as_deref()
        .filter(|c| crate::tracks::is_hex(c))
        .unwrap_or(fg)
}

/// The strip (Apple) or the hero (Google) as an SVG document of
/// `canvas.w`×`canvas.h` pixels.
pub fn svg(design: &CardDesign, state: &CardState, canvas: Canvas, opts: &SvgOpts) -> String {
    let (w, h) = (f64::from(canvas.w), f64::from(canvas.h));
    let s = canvas.scale;
    let c = &design.colours;
    let mut o = String::with_capacity(4096);
    let _ = write!(
        o,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" viewBox="0 0 {} {}">"#,
        canvas.w, canvas.h, canvas.w, canvas.h
    );

    // The ground.
    match design.ground.kind {
        GroundKind::Colour => {
            let _ = write!(
                o,
                r#"<rect width="{}" height="{}" fill="{}"/>"#,
                n(w),
                n(h),
                esc(&c.background)
            );
        }
        GroundKind::Pattern => {
            let _ = write!(
                o,
                r#"<rect width="{}" height="{}" fill="{}"/>"#,
                n(w),
                n(h),
                esc(&c.background)
            );
            let step = 12.0 * s;
            let mark = match design.ground.pattern.unwrap_or(Pattern::Dots) {
                Pattern::Dots => format!(
                    r#"<circle cx="{}" cy="{}" r="{}"/>"#,
                    n(step / 2.0),
                    n(step / 2.0),
                    n(1.2 * s)
                ),
                Pattern::Lines => format!(
                    r#"<path d="M0 {} L{} 0" stroke-width="{}" stroke="{}" fill="none"/>"#,
                    n(step),
                    n(step),
                    n(0.8 * s),
                    esc(&c.foreground)
                ),
                Pattern::Grid => format!(
                    r#"<path d="M{} 0V{}M0 {}H{}" stroke-width="{}" stroke="{}" fill="none"/>"#,
                    n(step),
                    n(step),
                    n(step),
                    n(step),
                    n(0.6 * s),
                    esc(&c.foreground)
                ),
            };
            let _ = write!(
                o,
                r#"<defs><pattern id="g" width="{}" height="{}" patternUnits="userSpaceOnUse"><g fill="{}">{mark}</g></pattern></defs><rect width="{}" height="{}" fill="url(#g)" opacity="0.14"/>"#,
                n(step),
                n(step),
                esc(&c.foreground),
                n(w),
                n(h)
            );
        }
        GroundKind::Photo => {
            if let Some(href) = &opts.photo_href {
                let _ = write!(
                    o,
                    r#"<image href="{}" xlink:href="{}" width="{}" height="{}" preserveAspectRatio="xMidYMid slice"/>"#,
                    esc(href),
                    esc(href),
                    n(w),
                    n(h)
                );
            }
            // The veil the bar and the footer sit on (look.rs `finish`: from 45 %
            // down, towards the card's ground, 0.8 at the bottom).
            let _ = write!(
                o,
                r#"<defs><linearGradient id="v" x1="0" y1="0" x2="0" y2="1"><stop offset="0.45" stop-color="{bg}" stop-opacity="0"/><stop offset="1" stop-color="{bg}" stop-opacity="0.8"/></linearGradient></defs><rect width="{}" height="{}" fill="url(#v)"/>"#,
                n(w),
                n(h),
                bg = esc(&c.background)
            );
            if opts.scrim_preview && opts.primary_field {
                let z = canvas.rect(APPLE_PRIMARY_ZONE, state.arabic);
                let (g1, g2) = if state.arabic { ("1", "0") } else { ("0", "1") };
                let veil = if crate::validate::luminance(&c.foreground).unwrap_or(1.0) > 0.5 {
                    "#000000"
                } else {
                    "#FFFFFF"
                };
                let (left, right) = if state.arabic {
                    (z.x - 0.2 * w, w)
                } else {
                    (0.0, z.x + z.w + 0.2 * w)
                };
                let _ = write!(
                    o,
                    r#"<defs><linearGradient id="s" x1="{g1}" y1="0" x2="{g2}" y2="0"><stop offset="0.75" stop-color="{veil}" stop-opacity="0.45"/><stop offset="1" stop-color="{veil}" stop-opacity="0"/></linearGradient></defs><rect x="{}" y="0" width="{}" height="{}" fill="url(#s)"/>"#,
                    n(left),
                    n(right - left),
                    n(h)
                );
            }
        }
    }

    let track = |p: u8| state.tracks.iter().find(|t| t.position == p);
    for e in &design.elements {
        if !e.cell.within() {
            continue;
        }
        let r = canvas.rect(e.cell, state.arabic);
        match &e.kind {
            ElementKind::Stamps { track: p } => {
                let Some(t) = track(*p) else { continue };
                let size = t.size.max(1);
                let filled = track_card(t.balance, size).progress_to_next;
                let g = stamp_grid(size, r);
                let shape = t.shape.unwrap_or(design.stamp.shape);
                let colour = ink(t, &c.foreground);
                let cw = r.w / f64::from(g.per_row);
                let ch = r.h / f64::from(g.rows);
                let k = g.diameter / 24.0;
                for i in 0..u32::try_from(size).unwrap_or(0) {
                    let (row, col) = (i / g.per_row, i % g.per_row);
                    // A short last row is centred under the full ones.
                    let in_row = if row + 1 == g.rows {
                        u32::try_from(size).unwrap_or(0) - row * g.per_row
                    } else {
                        g.per_row
                    };
                    let lead = (r.w - f64::from(in_row) * cw) / 2.0;
                    let cx_ltr = r.x + lead + (f64::from(col) + 0.5) * cw;
                    let cx = if state.arabic {
                        r.x + r.w - (cx_ltr - r.x)
                    } else {
                        cx_ltr
                    };
                    let cy = r.y + (f64::from(row) + 0.5) * ch;
                    let on = i64::from(i) < i64::from(filled);
                    let (fill, extra) = if on {
                        (format!(r#"fill="{}""#, esc(colour)), String::new())
                    } else {
                        match design.stamp.empty {
                            EmptyStyle::Outline => (
                                format!(
                                    r#"fill="none" stroke="{}" stroke-opacity="0.6" stroke-width="{}""#,
                                    esc(colour),
                                    n(1.6)
                                ),
                                String::new(),
                            ),
                            EmptyStyle::Faint => (
                                format!(r#"fill="{}" fill-opacity="0.25""#, esc(colour)),
                                String::new(),
                            ),
                        }
                    };
                    let _ = write!(
                        o,
                        r#"<g transform="translate({} {}) scale({})"><path d="{}" {fill}/>{extra}"#,
                        n(cx - g.diameter / 2.0),
                        n(cy - g.diameter / 2.0),
                        n(k),
                        shape_path(shape)
                    );
                    if shape == StampShape::Check && on {
                        let _ = write!(
                            o,
                            r#"<path d="M7 12.5l3.2 3.2L17.2 8.6" fill="none" stroke="{}" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/>"#,
                            esc(&c.background)
                        );
                    }
                    o.push_str("</g>");
                }
            }
            ElementKind::Bar { track: p } => {
                let Some(t) = track(*p) else { continue };
                let size = t.size.max(1);
                let filled = track_card(t.balance, size).progress_to_next.clamp(0, size);
                let colour = ink(t, &c.foreground);
                let th = (6.0 * s).min(r.h);
                let y = r.y + (r.h - th) / 2.0;
                let seg = |x0: f64, x1: f64, on: bool, o: &mut String| {
                    let (x0, x1) = if state.arabic {
                        (r.x + r.w - (x1 - r.x), r.x + r.w - (x0 - r.x))
                    } else {
                        (x0, x1)
                    };
                    let _ = write!(
                        o,
                        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}"{}/>"#,
                        n(x0),
                        n(y),
                        n((x1 - x0).max(0.0)),
                        n(th),
                        n(th / 2.0),
                        esc(colour),
                        if on { "" } else { r#" fill-opacity="0.3""# }
                    );
                };
                if size <= madar_loyalty::card::MAX_STEPS {
                    let gap = 4.0 * s;
                    let seg_w = (r.w - gap * f64::from(size - 1)) / f64::from(size);
                    for i in 0..size {
                        let x0 = r.x + f64::from(i) * (seg_w + gap);
                        seg(x0, x0 + seg_w, i < filled, &mut o);
                    }
                } else {
                    let cut = r.x + r.w * f64::from(filled) / f64::from(size);
                    seg(r.x, r.x + r.w, false, &mut o);
                    if filled > 0 {
                        seg(r.x, cut, true, &mut o);
                    }
                }
            }
            ElementKind::Count { track: p } => {
                let Some(t) = track(*p) else { continue };
                let size = t.size.max(1);
                let progress = track_card(t.balance, size).progress_to_next;
                let fs = count_font(size, r);
                let (x, anchor) = if state.arabic {
                    (r.x + r.w, "end")
                } else {
                    (r.x, "start")
                };
                let _ = write!(
                    o,
                    r#"<text x="{}" y="{}" font-family="Manrope" font-weight="600" font-size="{}" fill="{}" text-anchor="{anchor}" direction="ltr">{}/{}</text>"#,
                    n(x),
                    n(r.y + r.h / 2.0 + fs * 0.36),
                    n(fs),
                    esc(ink(t, &c.foreground)),
                    progress,
                    size
                );
            }
            ElementKind::Text {
                en,
                ar,
                size,
                align,
            } => {
                let (en, ar) = (en.trim(), ar.trim());
                let (words, arabic_words) = match (state.arabic, en.is_empty(), ar.is_empty()) {
                    (true, _, false) | (false, true, false) => (ar, true),
                    _ => (en, false),
                };
                if words.is_empty() {
                    continue;
                }
                let fs = size.points() * s;
                // Physical side: alignment is in reading order, so it flips on an
                // Arabic strip.
                let physical = match (align, state.arabic) {
                    (TextAlign::Centre, _) => 0,
                    (TextAlign::Start, false) | (TextAlign::End, true) => -1,
                    _ => 1,
                };
                let x = match physical {
                    -1 => r.x,
                    0 => r.x + r.w / 2.0,
                    _ => r.x + r.w,
                };
                // Anchored physically and drawn as an LTR paragraph by both
                // renderers (resvg ignores `direction` when anchoring; a browser
                // would inherit `rtl` from an Arabic page and flip it). An Arabic
                // run still reads right to left inside it (bidi).
                let anchor = match physical {
                    0 => "middle",
                    -1 => "start",
                    _ => "end",
                };
                let (family, weight) = if arabic_words {
                    ("Cairo", "500")
                } else {
                    ("Manrope", "600")
                };
                let _ = write!(
                    o,
                    r#"<text x="{}" y="{}" font-family="{family}" font-weight="{weight}" font-size="{}" fill="{}" text-anchor="{anchor}" direction="ltr">{}</text>"#,
                    n(x),
                    n(r.y + r.h / 2.0 + fs * 0.36),
                    n(fs),
                    esc(&c.foreground),
                    esc(words)
                );
            }
            ElementKind::Logo => {
                if let Some(href) = &opts.logo_href {
                    let _ = write!(
                        o,
                        r#"<image href="{}" xlink:href="{}" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="xMidYMid meet"/>"#,
                        esc(href),
                        esc(href),
                        n(r.x),
                        n(r.y),
                        n(r.w),
                        n(r.h)
                    );
                }
            }
        }
    }

    if let Some(href) = &opts.footer_href {
        let f = footer_rect(canvas, state.arabic);
        let _ = write!(
            o,
            r#"<image href="{}" xlink:href="{}" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="xMinYMid meet"/>"#,
            esc(href),
            esc(href),
            n(f.x),
            n(f.y),
            n(f.w),
            n(f.h)
        );
    }
    o.push_str("</svg>");
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Canvas;

    #[test]
    fn stamps_take_the_rows_that_make_them_biggest() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            w: 343.0,
            h: 42.0,
        };
        // 10 in one row: 34.3 wide, 42 tall → 34.3; in two rows: 68.6 × 21 → 21.
        let g = stamp_grid(10, r);
        assert_eq!((g.rows, g.per_row), (1, 10));
        assert!((g.diameter - 34.3 * 0.82).abs() < 1e-9);
        let tall = Rect {
            x: 0.0,
            y: 0.0,
            w: 120.0,
            h: 120.0,
        };
        let g = stamp_grid(9, tall);
        assert_eq!((g.rows, g.per_row), (3, 3));
    }

    #[test]
    fn text_is_escaped() {
        assert_eq!(esc("a<b & \"c\""), "a&lt;b &amp; &quot;c&quot;");
        assert_eq!(n(2.0), "2");
        assert_eq!(n(2.345), "2.35");
    }

    #[test]
    fn the_footer_sits_at_the_leading_edge() {
        let c = Canvas::apple(1);
        let l = footer_rect(c, false);
        assert!((l.x - 16.0).abs() < 1e-9);
        let r = footer_rect(c, true);
        assert!((r.x + r.w - (375.0 - 16.0)).abs() < 1e-9);
    }
}
