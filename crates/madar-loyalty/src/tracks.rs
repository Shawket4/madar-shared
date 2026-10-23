//! The redemption planner for a card with several stamp tracks.
//!
//! A card may hold up to four tracks (madar-card `tracks`): track 1 is the
//! programme's own card, tracks 2–4 each reward one unit of any of their own
//! menu items at the track's card size. At tender a mixed basket may take
//! rewards from several tracks at once (POS_MULTITRACK_DESIGN.md §2):
//!
//! - one reward per line, across all tracks;
//! - each track spends only its own balance;
//! - the shop's per-order ceiling counts UNITS across all tracks (one shared
//!   cap, D4);
//! - an ask names its track (`None` = track 1).
//!
//! Server and till run this same function, strict and trimming, as they run
//! [`crate::plan`]. A sale whose asks are all on track 1 is planned by
//! [`crate::plan`] itself, unchanged, so a one-track card answers exactly as
//! before (a test runs every `loyalty_plan_vectors.json` case through both).
//! Pinned by `vectors/loyalty_tracks_vectors.json` (hand-written).

use serde::{Deserialize, Serialize};

use crate::plan::{plan, unit_cost, Ask, Line, Mode, Plan, Planned, Programme, Refusal, Trim};

/// One track's programme: its rewards (for tracks 2–4, each of its items at
/// the track's size) and the member's balance on it. Its own `max_per_order`
/// is ignored: the ceiling is the sale's, shared.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackProgramme {
    pub track: u8,
    pub programme: Programme,
}

/// A reward the sale asks for, on a track.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackAsk {
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub units: Option<i64>,
    /// `None` reads as track 1.
    #[serde(default)]
    pub track: Option<u8>,
}

impl TrackAsk {
    pub fn track(&self) -> u8 {
        self.track.unwrap_or(1)
    }
}

/// One track's share of the plan.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackPlanned {
    pub track: u8,
    pub plan: Plan,
}

/// The whole plan, per track (only tracks that took something), with the
/// units across all of them and the till's first trim.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracksPlan {
    pub tracks: Vec<TrackPlanned>,
    pub units: i64,
    #[serde(default)]
    pub trimmed: Option<Trim>,
}

impl TracksPlan {
    /// The track and units covering `line`, if any.
    pub fn cover(&self, line: usize) -> Option<(u8, i64)> {
        self.tracks.iter().find_map(|t| {
            let u = t.plan.units_for(line);
            (u > 0).then_some((t.track, u))
        })
    }
}

/// Why the server refuses, and on which track (`None` for the sale as a whole).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackRefusal {
    #[serde(default)]
    pub track: Option<u8>,
    pub refusal: TrackRefusalKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TrackRefusalKind {
    /// The ask names a track the card does not have.
    TrackUnknown,
    /// The single-track planner's refusal, on that track.
    Plan { reason: Refusal },
}

fn refused(track: Option<u8>, reason: Refusal) -> TrackRefusal {
    TrackRefusal {
        track,
        refusal: TrackRefusalKind::Plan { reason },
    }
}

fn cap(max: Option<i64>) -> Option<i64> {
    max.filter(|c| *c > 0)
}

/// Plan the asked rewards over `lines`, across tracks. `max_per_order` is the
/// shop's ceiling, shared by all tracks.
pub fn plan_tracks(
    lines: &[Line],
    programmes: &[TrackProgramme],
    asks: &[TrackAsk],
    max_per_order: Option<i64>,
    mode: Mode,
) -> Result<TracksPlan, TrackRefusal> {
    if asks.is_empty() {
        return Ok(TracksPlan::default());
    }
    // All on track 1: today's planner, untouched.
    if asks.iter().all(|a| a.track() == 1) {
        let Some(p1) = programmes.iter().find(|p| p.track == 1) else {
            return match mode {
                Mode::Server => Err(TrackRefusal {
                    track: Some(1),
                    refusal: TrackRefusalKind::TrackUnknown,
                }),
                Mode::Till => Ok(TracksPlan {
                    trimmed: Some(Trim::NotOnOffer),
                    ..TracksPlan::default()
                }),
            };
        };
        let programme = Programme {
            max_per_order,
            ..p1.programme.clone()
        };
        let one: Vec<Ask> = asks
            .iter()
            .map(|a| Ask {
                line: a.line,
                units: a.units,
            })
            .collect();
        let p = plan(lines, &programme, &one, mode).map_err(|r| refused(Some(1), r))?;
        return Ok(TracksPlan {
            units: p.units,
            trimmed: p.trimmed,
            tracks: if p.lines.is_empty() {
                Vec::new()
            } else {
                vec![TrackPlanned { track: 1, plan: p }]
            },
        });
    }
    match mode {
        Mode::Server => strict(lines, programmes, asks, max_per_order),
        Mode::Till => Ok(trimmed(lines, programmes, asks, max_per_order)),
    }
}

/// The server's order, as the single planner: each ask in turn (its track,
/// then the line checks), then the shared ceiling, then each track's balance.
fn strict(
    lines: &[Line],
    programmes: &[TrackProgramme],
    asks: &[TrackAsk],
    max_per_order: Option<i64>,
) -> Result<TracksPlan, TrackRefusal> {
    let mut seen: Vec<usize> = Vec::new();
    let mut per: Vec<TrackPlanned> = Vec::new();
    for a in asks {
        let t = a.track();
        let Some(tp) = programmes.iter().find(|p| p.track == t) else {
            return Err(TrackRefusal {
                track: Some(t),
                refusal: TrackRefusalKind::TrackUnknown,
            });
        };
        if a.line.is_some_and(|l| l < lines.len() && seen.contains(&l)) {
            return Err(refused(Some(t), Refusal::TwiceOnOneLine));
        }
        // The line checks of one ask, by the single planner with no ceiling
        // and an unlimited balance (those come after every ask).
        let open = Programme {
            max_per_order: None,
            balance: i64::MAX,
            ..tp.programme.clone()
        };
        let one = [Ask {
            line: a.line,
            units: a.units,
        }];
        let p = plan(lines, &open, &one, Mode::Server).map_err(|r| refused(Some(t), r))?;
        let planned: Planned = p.lines.into_iter().next().unwrap_or_default();
        seen.push(planned.line);
        match per.iter_mut().find(|x| x.track == t) {
            Some(x) => {
                x.plan.cost += planned.cost;
                x.plan.units += planned.units;
                x.plan.lines.push(planned);
            }
            None => per.push(TrackPlanned {
                track: t,
                plan: Plan {
                    cost: planned.cost,
                    units: planned.units,
                    lines: vec![planned],
                    trimmed: None,
                },
            }),
        }
    }
    let units: i64 = per.iter().map(|t| t.plan.units).sum();
    if let Some(max) = cap(max_per_order) {
        if units > max {
            return Err(refused(
                None,
                Refusal::OverCap {
                    max,
                    claimed: units,
                },
            ));
        }
    }
    for t in &per {
        let balance = programmes
            .iter()
            .find(|p| p.track == t.track)
            .map_or(0, |p| p.programme.balance);
        if t.plan.cost > balance {
            return Err(refused(
                Some(t.track),
                Refusal::BalanceShort {
                    balance,
                    spent: t.plan.cost,
                },
            ));
        }
    }
    Ok(TracksPlan {
        tracks: per,
        units,
        trimmed: None,
    })
}

/// The till's: asks honoured in order, each trimmed to its line, the shared
/// ceiling and its own track's balance; the first trim named.
fn trimmed(
    lines: &[Line],
    programmes: &[TrackProgramme],
    asks: &[TrackAsk],
    max_per_order: Option<i64>,
) -> TracksPlan {
    let ceiling = cap(max_per_order);
    let mut per: Vec<TrackPlanned> = Vec::new();
    let mut claimed = 0i64;
    let mut first: Option<Trim> = None;
    for a in asks {
        let Some((index, line)) = a.line.and_then(|i| lines.get(i).map(|l| (i, l))) else {
            first.get_or_insert(Trim::LineGone);
            continue;
        };
        if per.iter().any(|t| t.plan.units_for(index) > 0) {
            continue;
        }
        if line.in_combo {
            first.get_or_insert(Trim::InCombo);
            continue;
        }
        let t = a.track();
        let Some(tp) = programmes.iter().find(|p| p.track == t) else {
            first.get_or_insert(Trim::NotOnOffer);
            continue;
        };
        let Some(unit) = unit_cost(line, &tp.programme) else {
            first.get_or_insert(Trim::NotOnOffer);
            continue;
        };
        let asked = a.units.unwrap_or(1);
        let mut units = asked.clamp(0, line.quantity.max(0));
        if units < asked {
            first.get_or_insert(Trim::LineShrank);
        }
        if let Some(c) = ceiling {
            let room = (c - claimed).max(0);
            if units > room {
                units = room;
                first.get_or_insert(Trim::OverCap);
            }
        }
        let spent = per.iter().find(|x| x.track == t).map_or(0, |x| x.plan.cost);
        let affordable = (tp.programme.balance.max(0) - spent) / unit;
        if units > affordable {
            units = affordable.max(0);
            first.get_or_insert(Trim::BalanceShort);
        }
        if units <= 0 {
            continue;
        }
        claimed += units;
        let planned = Planned {
            line: index,
            menu_item_id: line.menu_item_id.clone().unwrap_or_default(),
            units,
            unit_cost: unit,
            cost: unit * units,
        };
        match per.iter_mut().find(|x| x.track == t) {
            Some(x) => {
                x.plan.cost += planned.cost;
                x.plan.units += planned.units;
                x.plan.lines.push(planned);
            }
            None => per.push(TrackPlanned {
                track: t,
                plan: Plan {
                    cost: planned.cost,
                    units: planned.units,
                    lines: vec![planned],
                    trimmed: None,
                },
            }),
        }
    }
    TracksPlan {
        tracks: per,
        units: claimed,
        trimmed: first,
    }
}

/// The tracks that could cover one unit of `line` with the member's whole
/// balance on each: one means "use it", two or more mean the till asks
/// (nothing preselected, D5), none means not a reward.
pub fn tracks_for_line(line: &Line, programmes: &[TrackProgramme]) -> Vec<u8> {
    programmes
        .iter()
        .filter(|p| unit_cost(line, &p.programme).is_some_and(|c| c <= p.programme.balance))
        .map(|p| p.track)
        .collect()
}

/// A track 2–4's programme: each of its items at the track's card size.
pub fn track_programme(track: u8, items: &[String], size: i64, balance: i64) -> TrackProgramme {
    TrackProgramme {
        track,
        programme: Programme {
            rewards: items
                .iter()
                .map(|i| crate::plan::Reward {
                    menu_item_id: i.clone(),
                    cost: size,
                })
                .collect(),
            any_item: false,
            any_item_cost: 0,
            max_per_order: None,
            balance,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::vectors::PlanVector;

    /// `loyalty_tracks_vectors.json`: hand-written baskets; the answers were
    /// written by `MADAR_REGENERATE_TRACKS_VECTORS=1 cargo test -p
    /// madar-loyalty tracks_vectors` and checked by hand (each `why`).
    #[test]
    fn tracks_vectors() {
        let original: serde_json::Value = serde_json::from_str(crate::vectors::TRACKS).unwrap();
        let mut got = original.clone();
        for c in got["cases"].as_array_mut().unwrap() {
            let lines: Vec<Line> = serde_json::from_value(c["lines"].clone()).unwrap();
            let progs: Vec<TrackProgramme> =
                serde_json::from_value(c["programmes"].clone()).unwrap();
            let asks: Vec<TrackAsk> = serde_json::from_value(c["asks"].clone()).unwrap();
            let cap: Option<i64> = serde_json::from_value(c["max_per_order"].clone()).unwrap();
            c["till"] =
                serde_json::to_value(plan_tracks(&lines, &progs, &asks, cap, Mode::Till)).unwrap();
            c["server"] =
                serde_json::to_value(plan_tracks(&lines, &progs, &asks, cap, Mode::Server))
                    .unwrap();
            c["choices"] = serde_json::to_value(
                lines
                    .iter()
                    .map(|l| tracks_for_line(l, &progs))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        }
        if std::env::var("MADAR_REGENERATE_TRACKS_VECTORS").is_ok() {
            let path = format!(
                "{}/vectors/loyalty_tracks_vectors.json",
                env!("CARGO_MANIFEST_DIR")
            );
            std::fs::write(path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
            return;
        }
        assert_eq!(got, original);
    }

    #[test]
    fn one_track_answers_as_the_single_planner_on_every_plan_vector() {
        let cases: Vec<PlanVector> = serde_json::from_str(crate::vectors::PLAN).unwrap();
        assert!(cases.len() > 20);
        for c in cases {
            let tp = [TrackProgramme {
                track: 1,
                programme: c.programme.clone(),
            }];
            let asks: Vec<TrackAsk> = c
                .asks
                .iter()
                .map(|a| TrackAsk {
                    line: a.line,
                    units: a.units,
                    track: None,
                })
                .collect();
            for (mode, want) in [
                (Mode::Till, Ok(c.till.clone())),
                (Mode::Server, c.server.clone()),
            ] {
                let got = plan_tracks(&c.lines, &tp, &asks, c.programme.max_per_order, mode);
                match (got, want) {
                    (Ok(g), Ok(w)) => {
                        assert_eq!(g.units, w.units, "{}", c.name);
                        assert_eq!(g.trimmed, w.trimmed, "{}", c.name);
                        let lines: Vec<Planned> =
                            g.tracks.into_iter().flat_map(|t| t.plan.lines).collect();
                        assert_eq!(lines, w.lines, "{}", c.name);
                    }
                    (Err(g), Err(w)) => assert_eq!(
                        g.refusal,
                        TrackRefusalKind::Plan { reason: w },
                        "{}",
                        c.name
                    ),
                    (g, w) => panic!("{}: {g:?} vs {w:?}", c.name),
                }
            }
        }
    }
}
