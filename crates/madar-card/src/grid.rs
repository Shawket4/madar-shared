//! The snapping grid every design is laid out on, and how it maps onto each
//! wallet's image.
//!
//! One layout serves both wallets: Apple's strip (375×144 pt, about 2.6:1) and
//! Google's hero (1032×336 px, about 3.1:1) are both wide bands, so the same
//! 12×6 grid is stretched over each. Cells are measured inside a margin (the
//! bar and the footer of today's look sit 16 pt from the edge), so nothing is
//! drawn where a rounded corner or Google's mask could cut it.
//!
//! The grid is in reading order: on an Arabic pass every cell is mirrored, as
//! the bar already fills from the right on `ar.lproj/strip*.png`.

use serde::{Deserialize, Serialize};

/// Columns and rows of the grid.
pub const COLS: u8 = 12;
pub const ROWS: u8 = 6;

/// Apple's strip at 1×, in points (×2 and ×3 for the other files).
pub const APPLE_STRIP: (u32, u32) = (375, 144);
/// Google's hero image, in pixels.
pub const GOOGLE_HERO: (u32, u32) = (1032, 336);

/// The margin around the grid, as a share of the image (Apple 16 pt of 375
/// across, 8 pt of 144 down).
pub const MARGIN_X: f64 = 16.0 / 375.0;
pub const MARGIN_Y: f64 = 8.0 / 144.0;

/// Where iOS writes a store card's PRIMARY field over the strip: the leading
/// two thirds, the upper two thirds. A design that shows a primary field keeps
/// this free (the server scrims a photograph under it); nothing else may be
/// drawn there.
pub const APPLE_PRIMARY_ZONE: Cell = Cell {
    col: 0,
    row: 0,
    w: 8,
    h: 4,
};

/// The "rewards by madar" footer: always on the card, on the bottom row at
/// the leading edge (the board's look, 2026-10-19). Fixed, so no design can
/// drop or cover the endorsement.
pub const FOOTER_CELL: Cell = Cell {
    col: 0,
    row: 5,
    w: 6,
    h: 1,
};

/// A wallet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Store {
    Apple,
    Google,
}

/// A rectangle of whole cells: `col`/`row` from the leading top corner,
/// `w`×`h` cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Cell {
    pub col: u8,
    pub row: u8,
    pub w: u8,
    pub h: u8,
}

impl Cell {
    /// Inside the grid, at least one cell big.
    pub fn within(&self) -> bool {
        self.w >= 1
            && self.h >= 1
            && u16::from(self.col) + u16::from(self.w) <= u16::from(COLS)
            && u16::from(self.row) + u16::from(self.h) <= u16::from(ROWS)
    }

    /// Share at least one cell.
    pub fn overlaps(&self, o: &Cell) -> bool {
        let (a0, a1) = (u16::from(self.col), u16::from(self.col) + u16::from(self.w));
        let (b0, b1) = (u16::from(o.col), u16::from(o.col) + u16::from(o.w));
        let (c0, c1) = (u16::from(self.row), u16::from(self.row) + u16::from(self.h));
        let (d0, d1) = (u16::from(o.row), u16::from(o.row) + u16::from(o.h));
        a0 < b1 && b0 < a1 && c0 < d1 && d0 < c1
    }

    /// The same cell read from the other edge (an Arabic pass).
    pub fn mirrored(&self) -> Cell {
        Cell {
            col: COLS.saturating_sub(self.col.saturating_add(self.w)),
            ..*self
        }
    }
}

/// A pixel rectangle on an image.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// An image to draw on: its size in pixels, and how many pixels make one of
/// Apple's points (1, 2 or 3 for the strip; Google's hero counts as 2.75,
/// its width over the strip's).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    pub scale: f64,
}

impl Canvas {
    /// Apple's strip at `scale` (1, 2 or 3).
    pub fn apple(scale: u32) -> Canvas {
        let s = scale.clamp(1, 3);
        Canvas {
            w: APPLE_STRIP.0 * s,
            h: APPLE_STRIP.1 * s,
            scale: f64::from(s),
        }
    }

    /// Google's hero.
    pub fn google() -> Canvas {
        Canvas {
            w: GOOGLE_HERO.0,
            h: GOOGLE_HERO.1,
            scale: f64::from(GOOGLE_HERO.0) / f64::from(APPLE_STRIP.0),
        }
    }

    pub fn of(store: Store, scale: u32) -> Canvas {
        match store {
            Store::Apple => Canvas::apple(scale),
            Store::Google => Canvas::google(),
        }
    }

    /// The grid's area: the image less its margin.
    pub fn inner(&self) -> Rect {
        let (w, h) = (f64::from(self.w), f64::from(self.h));
        let mx = (w * MARGIN_X).round();
        let my = (h * MARGIN_Y).round();
        Rect {
            x: mx,
            y: my,
            w: w - 2.0 * mx,
            h: h - 2.0 * my,
        }
    }

    /// Where `cell` lands, mirrored when `rtl`.
    pub fn rect(&self, cell: Cell, rtl: bool) -> Rect {
        let c = if rtl { cell.mirrored() } else { cell };
        let inner = self.inner();
        let cw = inner.w / f64::from(COLS);
        let ch = inner.h / f64::from(ROWS);
        Rect {
            x: inner.x + f64::from(c.col) * cw,
            y: inner.y + f64::from(c.row) * ch,
            w: f64::from(c.w) * cw,
            h: f64::from(c.h) * ch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells() {
        let a = Cell {
            col: 0,
            row: 0,
            w: 4,
            h: 2,
        };
        assert!(a.within());
        assert!(!Cell {
            col: 10,
            row: 0,
            w: 3,
            h: 1
        }
        .within());
        assert!(!Cell {
            col: 0,
            row: 0,
            w: 0,
            h: 1
        }
        .within());
        assert!(a.overlaps(&Cell {
            col: 3,
            row: 1,
            w: 2,
            h: 2
        }));
        assert!(
            !a.overlaps(&Cell {
                col: 4,
                row: 0,
                w: 2,
                h: 2
            }),
            "edges touch, no overlap"
        );
        assert_eq!(
            a.mirrored(),
            Cell {
                col: 8,
                row: 0,
                w: 4,
                h: 2
            }
        );
        assert_eq!(FOOTER_CELL.mirrored().col, 6);
    }

    #[test]
    fn the_apple_strip_at_1x() {
        let c = Canvas::apple(1);
        let inner = c.inner();
        // 16 pt each side, 8 pt top and bottom.
        assert_eq!(
            (inner.x, inner.y, inner.w, inner.h),
            (16.0, 8.0, 343.0, 128.0)
        );
        let r = c.rect(
            Cell {
                col: 0,
                row: 5,
                w: 12,
                h: 1,
            },
            false,
        );
        assert!((r.y - (8.0 + 128.0 * 5.0 / 6.0)).abs() < 1e-9);
        let l = c.rect(
            Cell {
                col: 0,
                row: 0,
                w: 1,
                h: 1,
            },
            true,
        );
        assert!(
            (l.x - (16.0 + 343.0 * 11.0 / 12.0)).abs() < 1e-9,
            "mirrored to the far edge"
        );
    }
}
