//! Vectors for [`negative_part`]: which figure of a priced bill is the first
//! negative one, if any.
//!
//! Both sides refuse a sale with a negative figure (the till before queueing
//! it, the server live and at replay), and the part named is what the teller
//! reads and the owner looks for. Every combination of each checked figure
//! below, at and above zero, generated from the rule as it stood when it moved
//! here. Regenerate deliberately:
//! `MADAR_REGENERATE_NEGATIVE_VECTORS=1 cargo test -p madar-money negative_part_vectors`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{negative_part, Breakdown, Minor};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NegativeVector {
    pub subtotal: Minor,
    pub discount: Minor,
    pub service_charge: Minor,
    pub tax: Minor,
    pub total: Minor,
    pub net: Minor,
    /// `NegativePart::as_str` of the first negative figure; `null` for none.
    pub part: Option<String>,
}

pub fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/negative_part_vectors.json")
}

pub fn generate() -> Vec<NegativeVector> {
    let values: [Minor; 3] = [-1, 0, 1];
    let mut out = Vec::new();
    for subtotal in values {
        for discount in values {
            for service_charge in values {
                for tax in values {
                    for total in values {
                        // `net` is not a checked figure; vary it against total
                        // so a rule that started reading it would show.
                        let net = -total;
                        let b = Breakdown {
                            subtotal,
                            discount,
                            service_charge,
                            tax,
                            total,
                            net,
                        };
                        out.push(NegativeVector {
                            subtotal,
                            discount,
                            service_charge,
                            tax,
                            total,
                            net,
                            part: negative_part(&b).map(|p| p.as_str().to_string()),
                        });
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_part_vectors() {
        let generated = generate();
        if std::env::var("MADAR_REGENERATE_NEGATIVE_VECTORS").is_ok() {
            std::fs::write(
                fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let expected: Vec<NegativeVector> =
            serde_json::from_str(crate::vectors::NEGATIVE_PART).unwrap();
        assert_eq!(generated.len(), 243);
        assert_eq!(
            generated, expected,
            "negative_part drifted from its vectors"
        );
    }
}
