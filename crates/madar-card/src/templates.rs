//! Ready-made designs: the one-click starting points of the editor.
//!
//! Each is built for the shop as it stands (its colours, mode, tracks and
//! whether it has a photograph) and is valid for it by construction — a test
//! below runs every template through [`crate::validate`]. The default is
//! `classic`, today's look (a bar above the footer, the balance top right), or
//! `multi` when the shop runs more than one track.

use crate::design::{
    AppleFields, CardDesign, Colours, Element, ElementKind, EmptyStyle, FieldSource, GoogleFields,
    Ground, GroundKind, StampShape, StampStyle, TextAlign, TextSize, DESIGN_VERSION,
};
use crate::grid::{Canvas, Cell, COLS};
use crate::render::stamp_grid;
use crate::tracks::{Mode, Track};
use crate::validate::{Ctx, MAX_STAMPS, MIN_STAMP_PT};

/// Every template, in the order the editor offers them.
pub const TEMPLATE_IDS: [&str; 5] = ["classic", "stamps", "photo", "bold", "multi"];

fn base(id: &str, colours: &Colours) -> CardDesign {
    CardDesign {
        version: DESIGN_VERSION,
        template: id.into(),
        colours: colours.clone(),
        ground: Ground {
            kind: GroundKind::Colour,
            pattern: None,
        },
        elements: Vec::new(),
        stamp: StampStyle {
            shape: StampShape::Circle,
            empty: EmptyStyle::Outline,
        },
        apple: AppleFields {
            header: vec![FieldSource::Balance { track: 1 }],
            primary: None,
            secondary: vec![
                FieldSource::Rewards { track: 1 },
                FieldSource::Progress { track: 1 },
            ],
            auxiliary: Vec::new(),
        },
        google: GoogleFields {
            points: FieldSource::Progress { track: 1 },
            secondary: Some(FieldSource::Rewards { track: 1 }),
        },
    }
}

fn el(kind: ElementKind, col: u8, row: u8, w: u8, h: u8) -> Element {
    Element {
        kind,
        cell: Cell { col, row, w, h },
    }
}

/// Whether a track of `size` can be drawn as stamps in `cell`.
fn stamps_fit(ctx: &Ctx, position: u8, size: i32, cell: Cell) -> bool {
    !(ctx.mode == Mode::Points && position == 1)
        && size <= MAX_STAMPS
        && stamp_grid(size, Canvas::apple(1).rect(cell, false)).diameter >= MIN_STAMP_PT
}

/// The track's progress in `cell`: stamps when they fit, else a bar.
fn progress_el(ctx: &Ctx, position: u8, cell: Cell) -> Element {
    let size = ctx.track(position).map_or(1, |t| t.size);
    let kind = if stamps_fit(ctx, position, size, cell) {
        ElementKind::Stamps { track: position }
    } else {
        ElementKind::Bar { track: position }
    };
    Element { kind, cell }
}

/// Up to `max_chars` characters of `s` (a label cut to its box).
fn cut(s: &str, max_chars: usize) -> String {
    s.trim().chars().take(max_chars).collect()
}

/// The template `id` for this shop, or `None` when it cannot be built (a
/// photo template without a photograph, multi with one track, an unknown id).
pub fn template(id: &str, colours: &Colours, ctx: &Ctx, tracks: &[Track]) -> Option<CardDesign> {
    let size1 = ctx.track(1).map_or(1, |t| t.size);
    let mut d = base(id, colours);
    match id {
        "classic" => {
            d.elements = vec![el(ElementKind::Bar { track: 1 }, 0, 4, COLS, 1)];
        }
        "stamps" => {
            let cell = Cell {
                col: 0,
                row: 0,
                w: COLS,
                h: 4,
            };
            if !stamps_fit(ctx, 1, size1, cell) {
                return None;
            }
            d.elements = vec![el(ElementKind::Stamps { track: 1 }, 0, 0, COLS, 4)];
        }
        "photo" => {
            if !ctx.has_photo {
                return None;
            }
            d.ground.kind = GroundKind::Photo;
            d.elements = vec![el(ElementKind::Bar { track: 1 }, 0, 4, COLS, 1)];
            d.apple.header = Vec::new();
            d.apple.primary = Some(FieldSource::Balance { track: 1 });
        }
        "bold" => {
            d.elements = vec![
                el(ElementKind::Count { track: 1 }, 0, 0, 6, 4),
                el(ElementKind::Bar { track: 1 }, 0, 4, COLS, 1),
            ];
            d.apple.secondary = vec![FieldSource::Rewards { track: 1 }];
        }
        "multi" => {
            let n = ctx.tracks.len();
            if n < 2 {
                return None;
            }
            let h: u8 = if n <= 2 { 2 } else { 1 };
            let name = |p: u8, arabic: bool| {
                tracks
                    .iter()
                    .find(|t| t.position == p)
                    .map(|t| {
                        if arabic {
                            t.name_ar.clone()
                        } else {
                            t.name_en.clone()
                        }
                    })
                    .unwrap_or_default()
            };
            d.elements.clear();
            for (i, t) in ctx.tracks.iter().enumerate() {
                let row = u8::try_from(i).unwrap_or(0) * h;
                // A 3-column label (about 11 Latin characters at 12 pt), the
                // track across the other nine.
                let (en, ar) = (
                    cut(&name(t.position, false), 11),
                    cut(&name(t.position, true), 13),
                );
                if !en.is_empty() || !ar.is_empty() {
                    d.elements.push(el(
                        ElementKind::Text {
                            en,
                            ar,
                            size: TextSize::Small,
                            align: TextAlign::Start,
                        },
                        0,
                        row,
                        3,
                        h,
                    ));
                }
                d.elements.push(progress_el(
                    ctx,
                    t.position,
                    Cell {
                        col: 3,
                        row,
                        w: 9,
                        h,
                    },
                ));
            }
            d.apple.header = vec![FieldSource::Balance { track: 1 }];
            d.apple.secondary = ctx
                .tracks
                .iter()
                .take(3)
                .map(|t| FieldSource::Rewards { track: t.position })
                .collect();
            d.google = GoogleFields {
                points: FieldSource::Progress { track: 1 },
                secondary: Some(FieldSource::Progress { track: 2 }),
            };
        }
        _ => return None,
    }
    Some(d)
}

/// Every template this shop can use.
pub fn templates(colours: &Colours, ctx: &Ctx, tracks: &[Track]) -> Vec<CardDesign> {
    TEMPLATE_IDS
        .iter()
        .filter_map(|id| template(id, colours, ctx, tracks))
        .collect()
}

/// The design a shop starts from: today's look, or one row per track.
pub fn default_design(colours: &Colours, ctx: &Ctx, tracks: &[Track]) -> CardDesign {
    let id = if ctx.tracks.len() > 1 {
        "multi"
    } else {
        "classic"
    };
    template(id, colours, ctx, tracks)
        .or_else(|| template("classic", colours, ctx, tracks))
        .unwrap_or_else(|| base("classic", colours))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracks::TrackRef;
    use crate::validate::{is_valid, validate};

    fn colours() -> Colours {
        Colours {
            background: "#0D6273".into(),
            foreground: "#EFF3F4".into(),
            label: "#B9DCE3".into(),
        }
    }

    fn named(p: u8, en: &str, ar: &str, size: Option<i32>) -> Track {
        Track {
            position: p,
            name_en: en.into(),
            name_ar: ar.into(),
            size,
            shape: None,
            colour: None,
            items: Vec::new(),
            per_item: true,
            cap: None,
        }
    }

    #[test]
    fn every_template_is_valid_for_the_shop_it_was_built_for() {
        let shops = [
            (Mode::Visits, vec![5], true),
            (Mode::Visits, vec![12], false),
            (Mode::Visits, vec![30], true),
            (Mode::Points, vec![500], true),
            (Mode::Visits, vec![8, 6], true),
            (Mode::Visits, vec![10, 6, 4], false),
            (Mode::Visits, vec![10, 6, 4, 20], true),
        ];
        for (mode, sizes, has_photo) in shops {
            let ctx = Ctx {
                mode,
                tracks: sizes
                    .iter()
                    .enumerate()
                    .map(|(i, s)| TrackRef {
                        position: u8::try_from(i + 1).unwrap(),
                        size: *s,
                    })
                    .collect(),
                has_photo,
            };
            let tracks: Vec<Track> = (1..=ctx.tracks.len())
                .map(|p| {
                    let p = u8::try_from(p).unwrap();
                    named(p, "Iced coffee drinks", "قهوة مثلجة", (p > 1).then_some(6))
                })
                .collect();
            let all = templates(&colours(), &ctx, &tracks);
            assert!(!all.is_empty());
            for d in all {
                let problems = validate(&d, &ctx);
                assert!(
                    is_valid(&problems),
                    "{} on {ctx:?}: {problems:?}",
                    d.template
                );
            }
            let def = default_design(&colours(), &ctx, &tracks);
            assert_eq!(
                def.template,
                if ctx.tracks.len() > 1 {
                    "multi"
                } else {
                    "classic"
                }
            );
        }
    }
}
