//! The vector files this crate is pinned by, as bytes.
//!
//! - `card_design_vectors.json`: designs with the shop they are checked
//!   against and the problem codes [`crate::validate`] answers; where elements
//!   land on Apple's strip and Google's hero ([`crate::render::layout`]).
//! - `card_rules_vectors.json`: tracks, per-track cards, audiences, messages
//!   and the stepper text.
//!
//! Inputs are hand-written; the answers were written by
//! `MADAR_REGENERATE_CARD_VECTORS=1 cargo test -p madar-card vectors` and
//! reviewed by hand (each case's `why` holds the working). The dashboard (via
//! madar-web's `card` package) answers every case the same.
pub const DESIGN: &str = include_str!("../vectors/card_design_vectors.json");
pub const RULES: &str = include_str!("../vectors/card_rules_vectors.json");

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use crate::audience::{validate_audience, Audience};
    use crate::design::CardDesign;
    use crate::grid::{Canvas, Store};
    use crate::message::{validate_message, Message};
    use crate::render::layout;
    use crate::text::{earned_line, progress_line};
    use crate::tracks::{track_card, validate_tracks, Mode, Track};
    use crate::validate::{validate, Ctx};

    fn codes(p: Vec<crate::validate::Problem>) -> Value {
        json!(p
            .into_iter()
            .map(|p| json!([p.code, p.path]))
            .collect::<Vec<_>>())
    }

    fn design_answers(v: &mut Value) {
        for case in v["validate"].as_array_mut().unwrap() {
            let d: CardDesign = serde_json::from_value(case["design"].clone()).unwrap();
            let ctx: Ctx = serde_json::from_value(case["ctx"].clone()).unwrap();
            case["expected"] = codes(validate(&d, &ctx));
        }
        for case in v["layout"].as_array_mut().unwrap() {
            let d: CardDesign = serde_json::from_value(case["design"].clone()).unwrap();
            let store: Store = serde_json::from_value(case["store"].clone()).unwrap();
            let canvas = Canvas::of(
                store,
                u32::try_from(case["scale"].as_u64().unwrap()).unwrap(),
            );
            let placed = layout(
                &d,
                canvas,
                case["arabic"].as_bool().unwrap(),
                case["primary_field"].as_bool().unwrap(),
            );
            case["expected"] = serde_json::to_value(placed).unwrap();
        }
    }

    fn rules_answers(v: &mut Value) {
        for case in v["tracks"].as_array_mut().unwrap() {
            let t: Vec<Track> = serde_json::from_value(case["tracks"].clone()).unwrap();
            let mode: Mode = serde_json::from_value(case["mode"].clone()).unwrap();
            case["expected"] = codes(validate_tracks(&t, mode));
        }
        for case in v["track_card"].as_array_mut().unwrap() {
            let b = i32::try_from(case["balance"].as_i64().unwrap()).unwrap();
            let s = i32::try_from(case["size"].as_i64().unwrap()).unwrap();
            case["expected"] = serde_json::to_value(track_card(b, s)).unwrap();
        }
        for case in v["audience"].as_array_mut().unwrap() {
            let a: Audience = serde_json::from_value(case["audience"].clone()).unwrap();
            let n = u8::try_from(case["track_count"].as_u64().unwrap()).unwrap();
            case["expected"] = codes(validate_audience(&a, n));
        }
        for case in v["message"].as_array_mut().unwrap() {
            let m: Message = serde_json::from_value(case["message"].clone()).unwrap();
            case["expected"] = codes(validate_message(&m));
        }
        for case in v["stepper"].as_array_mut().unwrap() {
            let b = i32::try_from(case["balance"].as_i64().unwrap()).unwrap();
            let s = i32::try_from(case["size"].as_i64().unwrap()).unwrap();
            case["expected"] =
                json!({"progress": progress_line(b, s), "earned": earned_line(b, s)});
        }
    }

    fn check(file: &str, bytes: &str, answer: fn(&mut Value)) {
        let original: Value = serde_json::from_str(bytes).unwrap();
        let mut got = original.clone();
        answer(&mut got);
        if std::env::var("MADAR_REGENERATE_CARD_VECTORS").is_ok() {
            let path = format!("{}/vectors/{file}", env!("CARGO_MANIFEST_DIR"));
            std::fs::write(path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
            return;
        }
        assert!(
            approx(&got, &original),
            "{file}: an answer moved (regenerate only for a deliberate change)\n{got:#}"
        );
    }

    /// Equal, with numbers to a millionth (serde_json's default float parse
    /// may land one ulp off the value that was printed).
    fn approx(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
                (Some(x), Some(y)) => (x - y).abs() < 1e-6,
                _ => x == y,
            },
            (Value::Array(x), Value::Array(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(x, y)| approx(x, y))
            }
            (Value::Object(x), Value::Object(y)) => {
                x.len() == y.len()
                    && x.iter()
                        .all(|(k, v)| y.get(k).is_some_and(|w| approx(v, w)))
            }
            _ => a == b,
        }
    }

    #[test]
    fn card_design_vectors() {
        check("card_design_vectors.json", super::DESIGN, design_answers);
    }

    #[test]
    fn card_rules_vectors() {
        check("card_rules_vectors.json", super::RULES, rules_answers);
    }
}
