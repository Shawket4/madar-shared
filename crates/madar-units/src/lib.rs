//! # madar-units
//!
//! Inventory units: `g`, `kg`, `ml`, `l`, `pcs` (the backend's
//! `inventory_unit` enum), their families (mass, volume, count) and the
//! conversion between them, rounded to 3 decimals like `numeric(12,3)`
//! storage. A mass↔volume conversion is bridged only by a positive density
//! (grams per millilitre); a count never bridges.
//!
//! Moved from MadarRust `src/units.rs`; madar-core `waste.rs` carried a copy
//! without the density bridge. The error messages are the server's, word for
//! word (it returns them as a 400).
//!
//! Pinned by `vectors/unit_vectors.json`.

use core::fmt;

/// `(family, factor to the family's canonical unit)`. Canonical per family:
/// grams for mass, millilitres for volume, pcs for count. Case and
/// surrounding whitespace are ignored.
pub fn unit_spec(unit: &str) -> Option<(&'static str, f64)> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "g" => Some(("mass", 1.0)),
        "kg" => Some(("mass", 1000.0)),
        "ml" => Some(("volume", 1.0)),
        "l" => Some(("volume", 1000.0)),
        "pcs" => Some(("count", 1.0)),
        _ => None,
    }
}

/// True iff `unit` is a recognized inventory unit.
pub fn is_valid_unit(unit: &str) -> bool {
    unit_spec(unit).is_some()
}

/// The units a quantity of something stocked in `base` may be typed in (the
/// base unit's family; an unknown base only in itself).
pub fn units_of(base: &str) -> Vec<String> {
    match unit_spec(base).map(|(f, _)| f) {
        Some("mass") => vec!["g".into(), "kg".into()],
        Some("volume") => vec!["ml".into(), "l".into()],
        Some(_) => vec!["pcs".into()],
        None => vec![base.to_string()],
    }
}

/// Why a quantity could not be converted.
#[derive(Debug, Clone, PartialEq)]
pub enum UnitError {
    Unknown {
        unit: String,
    },
    /// Different families and no bridge between them.
    Incompatible {
        from: String,
        to: String,
        from_family: &'static str,
        to_family: &'static str,
    },
    /// Mass↔volume needs a positive density.
    NeedsDensity {
        from: String,
        to: String,
    },
    /// A count never bridges.
    NotBridged {
        from: String,
        to: String,
    },
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnitError::Unknown { unit } => write!(f, "Unknown unit '{unit}'"),
            UnitError::Incompatible {
                from,
                to,
                from_family,
                to_family,
            } => write!(
                f,
                "Cannot convert '{from}' to '{to}': incompatible unit families ({from_family} vs {to_family})"
            ),
            UnitError::NeedsDensity { from, to } => write!(
                f,
                "Cannot convert '{from}' to '{to}': set a density (g/ml) on the ingredient to convert between weight and volume."
            ),
            UnitError::NotBridged { from, to } => write!(
                f,
                "Cannot convert '{from}' to '{to}': only weight↔volume is bridged by density."
            ),
        }
    }
}

impl std::error::Error for UnitError {}

fn spec(unit: &str) -> Result<(&'static str, f64), UnitError> {
    unit_spec(unit).ok_or_else(|| UnitError::Unknown {
        unit: unit.to_string(),
    })
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// `qty` in `from_unit` expressed in `to_unit`, rounded to 3 decimals. Across
/// families it is an error.
pub fn convert(qty: f64, from_unit: &str, to_unit: &str) -> Result<f64, UnitError> {
    let (ff, fk) = spec(from_unit)?;
    let (tf, tk) = spec(to_unit)?;
    if ff != tf {
        return Err(UnitError::Incompatible {
            from: from_unit.to_string(),
            to: to_unit.to_string(),
            from_family: ff,
            to_family: tf,
        });
    }
    Ok(round3(qty * fk / tk))
}

/// Like [`convert`], but mass↔volume is allowed with a positive density
/// (grams per millilitre). A count never bridges.
pub fn convert_with_density(
    qty: f64,
    from_unit: &str,
    to_unit: &str,
    density_g_per_ml: Option<f64>,
) -> Result<f64, UnitError> {
    let (ff, fk) = spec(from_unit)?;
    let (tf, tk) = spec(to_unit)?;
    if ff == tf {
        return Ok(round3(qty * fk / tk));
    }
    let density = match density_g_per_ml {
        Some(d) if d > 0.0 => d,
        _ => {
            return Err(UnitError::NeedsDensity {
                from: from_unit.to_string(),
                to: to_unit.to_string(),
            })
        }
    };
    let from_canonical = qty * fk; // grams if mass, millilitres if volume
    let to_canonical = match (ff, tf) {
        ("mass", "volume") => from_canonical / density,
        ("volume", "mass") => from_canonical * density,
        _ => {
            return Err(UnitError::NotBridged {
                from: from_unit.to_string(),
                to: to_unit.to_string(),
            })
        }
    };
    Ok(round3(to_canonical / tk))
}

pub mod vectors {
    //! Conversion vectors. Regenerate deliberately:
    //! `MADAR_REGENERATE_UNIT_VECTORS=1 cargo test -p madar-units unit_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::convert_with_density;

    /// The file, for consumer tests.
    pub const UNITS: &str = include_str!("../vectors/unit_vectors.json");

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct UnitVector {
        pub qty: f64,
        pub from: String,
        pub to: String,
        pub density: Option<f64>,
        /// `Ok` value, or the error message.
        pub result: Option<f64>,
        pub error: Option<String>,
        /// Whether plain `convert` (no density) gives the same answer.
        pub same_without_density: bool,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/unit_vectors.json")
    }

    pub fn generate() -> Vec<UnitVector> {
        let units = ["g", "kg", "ml", "l", "pcs", "KG", " l ", "cups"];
        let qtys = [0.0, 0.0004, 0.0005, 1.0, 250.0, 920.0, 1234.5678, -2.0];
        let densities = [None, Some(1.0), Some(0.92), Some(0.0), Some(-1.0)];
        let mut out = Vec::new();
        for from in units {
            for to in units {
                for qty in qtys {
                    for density in densities {
                        let r = convert_with_density(qty, from, to, density);
                        let plain = super::convert(qty, from, to);
                        out.push(UnitVector {
                            qty,
                            from: from.into(),
                            to: to.into(),
                            density,
                            same_without_density: plain.as_ref().ok() == r.as_ref().ok(),
                            result: r.as_ref().ok().copied(),
                            error: r.err().map(|e| e.to_string()),
                        });
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
        fn unit_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_UNIT_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vec<UnitVector> = serde_json::from_str(UNITS).unwrap();
            assert_eq!(
                generated, expected,
                "unit conversion drifted from its vectors"
            );
        }
    }
}

// Moved from MadarRust `src/units.rs` with the rule (the `normalize_to_base`
// test stays there with that helper).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(convert(1.0, "kg", "g").unwrap(), 1000.0);
        assert_eq!(convert(250.0, "g", "kg").unwrap(), 0.25);
        assert_eq!(convert(2.0, "l", "ml").unwrap(), 2000.0);
        assert_eq!(convert(5.0, "g", "g").unwrap(), 5.0);
        assert_eq!(convert(3.0, "pcs", "pcs").unwrap(), 3.0);
    }

    #[test]
    fn cross_family_is_rejected() {
        assert!(convert(5.0, "g", "pcs").is_err());
        assert!(convert(5.0, "ml", "g").is_err());
        assert!(convert(5.0, "bogus", "g").is_err());
    }

    #[test]
    fn rounds_to_three_dp() {
        // 0.0001 g → kg is 0.0000001, rounds to 0.000.
        assert_eq!(convert(0.0001, "g", "kg").unwrap(), 0.0);
    }

    #[test]
    fn validity() {
        assert!(is_valid_unit("kg"));
        assert!(is_valid_unit("PCS"));
        assert!(!is_valid_unit("cups"));
    }

    #[test]
    fn density_bridges_mass_and_volume() {
        assert_eq!(
            convert_with_density(250.0, "ml", "g", Some(1.0)).unwrap(),
            250.0
        );
        assert_eq!(
            convert_with_density(1.0, "l", "g", Some(0.92)).unwrap(),
            920.0
        );
        assert_eq!(
            convert_with_density(920.0, "g", "ml", Some(0.92)).unwrap(),
            1000.0
        );
        assert_eq!(convert_with_density(1.0, "kg", "g", None).unwrap(), 1000.0);
        assert!(convert_with_density(5.0, "ml", "g", None).is_err());
        assert!(convert_with_density(5.0, "pcs", "g", Some(1.0)).is_err());
    }

    #[test]
    fn density_must_be_positive() {
        assert!(convert_with_density(250.0, "ml", "g", Some(0.0)).is_err());
        assert!(convert_with_density(250.0, "ml", "g", Some(-1.0)).is_err());
        assert!(convert_with_density(250.0, "g", "ml", Some(0.0)).is_err());
    }

    #[test]
    fn convert_with_density_divides_by_target_factor() {
        assert_eq!(convert_with_density(250.0, "g", "kg", None).unwrap(), 0.25);
        assert_eq!(
            convert_with_density(1000.0, "ml", "kg", Some(1.0)).unwrap(),
            1.0
        );
    }

    #[test]
    fn the_servers_messages_word_for_word() {
        assert_eq!(
            convert(5.0, "g", "pcs").unwrap_err().to_string(),
            "Cannot convert 'g' to 'pcs': incompatible unit families (mass vs count)"
        );
        assert_eq!(
            convert(5.0, "cups", "g").unwrap_err().to_string(),
            "Unknown unit 'cups'"
        );
    }
}
