//! A track's figures as the OS-drawn fields print them.
//!
//! The same rule as the passes' `wallet::google::{stepper, progress_line,
//! earned_line}` (MadarRust), so a designed card's field reads exactly like a
//! classic one: `●─●─○─○` joined up to six steps, `●●●○○○○○○` up to twelve, and
//! the figures alone ("9 / 20") past that. The backend checks the two agree.

use madar_loyalty::card::{card, stamps};

/// Past this many steps the row is drawn without joins.
pub const CONNECTED_UP_TO: i32 = 6;

/// `filled` of `steps` as a stamp row, or `None` past the countable cap.
pub fn stepper(filled: i32, steps: i32) -> Option<String> {
    let filled = stamps(filled, steps)?;
    let step = |i: i32| if i < filled { "●" } else { "○" };
    let mut out = String::from(step(0));
    for i in 1..steps {
        if steps <= CONNECTED_UP_TO {
            out.push('\u{2500}');
        }
        out.push_str(step(i));
    }
    Some(out)
}

/// Where the balance is on its CURRENT card: the row, else "p / n".
pub fn progress_line(balance: i32, size: i32) -> String {
    let progress = card(balance, size).progress_to_next;
    stepper(progress, size).unwrap_or_else(|| format!("{progress} / {size}"))
}

/// The rewards already earned, as a full row (and "×n" for more than one);
/// `None` when there are none.
pub fn earned_line(balance: i32, size: i32) -> Option<String> {
    let earned = card(balance, size).rewards_ready;
    if earned <= 0 {
        return None;
    }
    let full = stepper(size, size).unwrap_or_default();
    Some(if earned == 1 {
        full
    } else if full.is_empty() {
        format!("×{earned}")
    } else {
        format!("{full}  ×{earned}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows() {
        assert_eq!(stepper(3, 5).as_deref(), Some("●─●─●─○─○"));
        assert_eq!(stepper(2, 8).as_deref(), Some("●●○○○○○○"));
        assert_eq!(stepper(2, 13), None);
        assert_eq!(progress_line(6, 5), "●─○─○─○─○");
        assert_eq!(progress_line(9, 20), "9 / 20");
        assert_eq!(earned_line(4, 5), None);
        assert_eq!(earned_line(11, 5).as_deref(), Some("●─●─●─●─●  ×2"));
        assert_eq!(earned_line(40, 20).as_deref(), Some("×2"));
    }
}
