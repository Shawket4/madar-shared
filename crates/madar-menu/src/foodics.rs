//! A [`BulkMenu`] from a Foodics console export.
//!
//! The caller passes each file's text; which file is which is read from its
//! header row, so the names the owner saved them under do not matter. Only
//! `products.csv` is required. Columns are read by name, so a Foodics export
//! that adds columns still reads.
//!
//! The mapping is the one MadarRust's `import-foodics-menu.sql` (now in
//! madar-admin) runs in its additive mode, with its quiet faults fixed: every
//! key is trimmed, the first of two rows with one name wins (with a warning)
//! instead of an arbitrary one, a category with no name sends its products to
//! Uncategorized, and a value that is not a number is a warning instead of a
//! failed import.
//!
//! - Categories, items, groups and options are keyed by trimmed name; an item
//!   is found by its `sku`, a group by its `reference` or else its name.
//! - Prices are EGP in the file and piastres in the menu.
//! - A group's own limits are the ones most of its products use (ties: the
//!   lower minimum, then the lower maximum, "no maximum" last).
//! - A group is the item's sizes when its reference is `size`, or when its
//!   name says size and it looks like sizes everywhere it is used (pick one,
//!   options named like sizes). A size's price is the product's price plus the
//!   option's. A group named like sizes that fails the test stays a group,
//!   with a warning.
//! - BulkFamily: reference `milk_type` / `coffee_type` / `extra` is taken as is;
//!   otherwise a name with milk is milk, one with beans is beans, the rest
//!   extras.
//! - Missing Arabic names stay empty.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::bulk::{
    name_key, BulkCategory, BulkFamily, BulkGroup, BulkItem, BulkLink, BulkMenu, BulkOption,
    BulkOptional, BulkSize, VERSION,
};

/// The most text one file may hold, and the most rows.
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_FILE_ROWS: usize = 20_000;

const UNCATEGORIZED: &str = "Uncategorized";
const UNCATEGORIZED_AR: &str = "غير مصنف";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum FoodicsFile {
    Categories,
    Products,
    ProductModifiers,
    ModifierGroups,
    ModifierOptions,
    /// Madar's own file of per-product extras (not a Foodics export).
    ProductOptionals,
}

/// Each file's header as Foodics writes it. A file is the kind whose whole
/// header it carries; with several, the one with the most columns.
const HEADERS: [(FoodicsFile, &[&str]); 6] = [
    (
        FoodicsFile::Categories,
        &["id", "name", "name_localized", "reference", "image"],
    ),
    (
        FoodicsFile::Products,
        &[
            "id",
            "name",
            "sku",
            "category_reference",
            "is_active",
            "price",
            "description",
            "name_localized",
            "description_localized",
        ],
    ),
    (
        FoodicsFile::ProductModifiers,
        &[
            "product_sku",
            "modifier_name",
            "modifier_name_localized",
            "modifier_reference",
            "minimum_options",
            "maximum_options",
        ],
    ),
    (
        FoodicsFile::ModifierGroups,
        &["id", "reference", "name", "name_localized"],
    ),
    (
        FoodicsFile::ModifierOptions,
        &[
            "id",
            "modifier_reference",
            "modifier_name",
            "name",
            "sku",
            "price",
            "name_localized",
            "is_active",
        ],
    ),
    (
        FoodicsFile::ProductOptionals,
        &[
            "product_sku",
            "product_name",
            "name",
            "name_localized",
            "price",
        ],
    ),
];

/// Why the files cannot be read at all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum FoodicsError {
    /// File `index` (0-based, in the order passed) is not a file this reads.
    UnknownFile {
        index: usize,
    },
    DuplicateFile {
        kind: FoodicsFile,
    },
    MissingProducts,
    TooBig {
        index: usize,
        max_bytes: usize,
        max_rows: usize,
    },
    BadCsv {
        kind: FoodicsFile,
        line: u64,
        message: String,
    },
}

impl std::fmt::Display for FoodicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFile { index } => {
                write!(f, "file {} is not a Foodics menu export", index + 1)
            }
            Self::DuplicateFile { kind } => write!(f, "two files are both {kind:?}"),
            Self::MissingProducts => write!(f, "the products file is missing"),
            Self::TooBig {
                index,
                max_bytes,
                max_rows,
            } => {
                write!(
                    f,
                    "file {} is larger than {max_bytes} bytes or {max_rows} rows",
                    index + 1
                )
            }
            Self::BadCsv {
                kind,
                line,
                message,
            } => write!(f, "{kind:?} line {line}: {message}"),
        }
    }
}

impl std::error::Error for FoodicsError {}

/// Something the reader skipped or changed. `line` is the file's line
/// (the header is line 1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct FoodicsWarning {
    pub file: Option<FoodicsFile>,
    pub line: Option<u64>,
    pub issue: FoodicsIssue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[serde(tag = "issue", rename_all = "snake_case")]
pub enum FoodicsIssue {
    /// An optional file was not given.
    MissingFile { kind: FoodicsFile },
    /// A row with no name; skipped.
    BlankName,
    /// A second row with a name already read; skipped.
    Duplicate { name: String },
    /// A product whose SKU an earlier product has; the later one is skipped.
    DuplicateSku { sku: String },
    /// A product with no category, or one not in the categories file.
    Uncategorized { item: String },
    /// A value that should be a number; the row is skipped.
    NotANumber { column: String, value: String },
    /// A yes/no column with another value; read as yes.
    NotYesNo { column: String, value: String },
    /// A row naming a product SKU no product has; skipped.
    UnknownSku { sku: String },
    /// An option of a group no product and no group row names; skipped.
    UnknownGroup { group: String },
    /// A product linked to one group twice; the later link is skipped.
    DuplicateLink { item: String, group: String },
    /// A group with no options; left out, with its links.
    EmptyGroup { group: String },
    /// A product's minimum above the group's active options, lowered to
    /// their number.
    MinLowered {
        group: String,
        item: String,
        from: u32,
        to: u32,
    },
    /// A group read as the products' sizes because of its name.
    SizesByName { group: String },
    /// A group named like sizes that does not look like sizes; kept as a group.
    LooksLikeSizes { group: String },
    /// A product linked to a second sizes group; that link is skipped.
    SecondSizes { item: String, group: String },
    /// A second group with a name already read (another reference); skipped.
    DuplicateGroup { name: String },
}

/// The menu read from the files, every warning, and which files were read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
pub struct FoodicsParsed {
    pub menu: BulkMenu,
    pub files: Vec<FoodicsFile>,
    pub warnings: Vec<FoodicsWarning>,
}

/// Which file `text` is, from its header row.
pub fn detect(text: &str) -> Option<FoodicsFile> {
    let first = strip_bom(text).lines().next()?;
    let mut r = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(first.as_bytes());
    let header: HashSet<String> = r
        .records()
        .next()?
        .ok()?
        .iter()
        .map(|c| c.trim().to_lowercase())
        .collect();
    HEADERS
        .iter()
        .filter(|(_, cols)| cols.iter().all(|c| header.contains(*c)))
        .max_by_key(|(_, cols)| cols.len())
        .map(|(k, _)| *k)
}

fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

/// One CSV row, its cells by column name.
struct Row {
    line: u64,
    cells: HashMap<String, String>,
}

impl Row {
    fn get(&self, col: &str) -> &str {
        self.cells.get(col).map_or("", String::as_str)
    }

    fn opt(&self, col: &str) -> Option<String> {
        Some(self.get(col))
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }
}

fn read(kind: FoodicsFile, index: usize, text: &str) -> Result<Vec<Row>, FoodicsError> {
    let too_big = FoodicsError::TooBig {
        index,
        max_bytes: MAX_FILE_BYTES,
        max_rows: MAX_FILE_ROWS,
    };
    if text.len() > MAX_FILE_BYTES {
        return Err(too_big);
    }
    let bad = |line: u64, e: csv::Error| FoodicsError::BadCsv {
        kind,
        line,
        message: e.to_string(),
    };
    let mut r = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(strip_bom(text).as_bytes());
    let header: Vec<String> = r
        .headers()
        .map_err(|e| bad(1, e))?
        .iter()
        .map(|c| c.trim().to_lowercase())
        .collect();
    let mut rows = Vec::new();
    for rec in r.records() {
        let rec = rec.map_err(|e| bad(e.position().map_or(0, csv::Position::line), e))?;
        if rows.len() == MAX_FILE_ROWS {
            return Err(too_big);
        }
        if rec.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let cells = header
            .iter()
            .zip(rec.iter())
            .map(|(h, c)| (h.clone(), c.trim().to_string()))
            .collect();
        rows.push(Row {
            line: rec.position().map_or(0, csv::Position::line),
            cells,
        });
    }
    Ok(rows)
}

/// EGP text ("110", "12.5") as piastres, rounded half away from zero like
/// the SQL's `round(price * 100)`. Blank is 0.
fn piastres(s: &str) -> Option<i64> {
    if s.is_empty() {
        return Some(0);
    }
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty() && frac.is_empty() {
        return None;
    }
    let digits = |p: &str| p.chars().all(|c| c.is_ascii_digit());
    if !digits(whole) || !digits(frac) || whole.len() > 9 {
        return None;
    }
    let w: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let f = frac.as_bytes();
    let d = |i: usize| f.get(i).map_or(0, |b| i64::from(b - b'0'));
    let up = i64::from(d(2) >= 5);
    Some(w * 100 + d(0) * 10 + d(1) + up)
}

fn yes_no(row: &Row, col: &str, kind: FoodicsFile, warnings: &mut Vec<FoodicsWarning>) -> bool {
    match row.get(col).to_lowercase().as_str() {
        "" | "yes" | "true" | "1" => true,
        "no" | "false" | "0" => false,
        other => {
            warnings.push(FoodicsWarning {
                file: Some(kind),
                line: Some(row.line),
                issue: FoodicsIssue::NotYesNo {
                    column: col.into(),
                    value: other.into(),
                },
            });
            true
        }
    }
}

/// A min/max cell: blank min is 0, blank (or 0) max is no maximum.
fn whole(s: &str) -> Option<u32> {
    if s.is_empty() {
        return Some(0);
    }
    s.parse::<u32>().ok().or_else(|| {
        // Foodics writes "1.0" on some exports.
        s.strip_suffix(".0").and_then(|w| w.parse().ok())
    })
}

const SIZE_GROUP_NAMES: &[&str] = &[
    "size",
    "sizes",
    "cup size",
    "drink size",
    "الحجم",
    "الأحجام",
    "الاحجام",
    "حجم",
    "المقاس",
    "مقاس",
];
const SIZE_WORDS: &[&str] = &[
    "small",
    "medium",
    "large",
    "regular",
    "xs",
    "s",
    "m",
    "l",
    "xl",
    "xxl",
    "mini",
    "short",
    "tall",
    "grande",
    "venti",
    "kids",
    "single",
    "double",
    "صغير",
    "وسط",
    "متوسط",
    "كبير",
    "عادي",
    "ميني",
    "سنجل",
    "دبل",
];

fn size_like(option: &str) -> bool {
    let k = name_key(option);
    k.contains("oz")
        || k.contains("ml")
        || k.contains("أونصة")
        || k.split(' ').any(|w| SIZE_WORDS.contains(&w))
}

struct Group {
    key: String,
    name: String,
    name_ar: Option<String>,
    options: Vec<BulkOption>,
    /// (min, max) of each product's link, in file order.
    uses: Vec<(u32, Option<u32>)>,
}

struct Link {
    item: usize,
    group: String,
    min: u32,
    max: Option<u32>,
    line: u64,
}

/// Reads the files (each one's text, in any order) into a [`BulkMenu`].
pub fn parse(files: &[&str]) -> Result<FoodicsParsed, FoodicsError> {
    let mut by_kind: HashMap<FoodicsFile, Vec<Row>> = HashMap::new();
    let mut kinds = Vec::new();
    for (index, text) in files.iter().enumerate() {
        let kind = detect(text).ok_or(FoodicsError::UnknownFile { index })?;
        if by_kind.contains_key(&kind) {
            return Err(FoodicsError::DuplicateFile { kind });
        }
        by_kind.insert(kind, read(kind, index, text)?);
        kinds.push(kind);
    }
    let products = by_kind
        .remove(&FoodicsFile::Products)
        .ok_or(FoodicsError::MissingProducts)?;
    let mut warnings = Vec::new();
    let mut take = |kind: FoodicsFile| by_kind.remove(&kind).unwrap_or_default();
    let categories_rows = take(FoodicsFile::Categories);
    let links_rows = take(FoodicsFile::ProductModifiers);
    let groups_rows = take(FoodicsFile::ModifierGroups);
    let options_rows = take(FoodicsFile::ModifierOptions);
    let optionals_rows = take(FoodicsFile::ProductOptionals);

    // Categories: one per name; a reference finds it.
    let mut categories: Vec<BulkCategory> = Vec::new();
    let mut category_by_name: HashMap<String, usize> = HashMap::new();
    let mut category_by_ref: HashMap<String, usize> = HashMap::new();
    use FoodicsFile as K;
    for row in &categories_rows {
        let name = row.get("name");
        if name.is_empty() {
            warn(
                &mut warnings,
                K::Categories,
                row.line,
                FoodicsIssue::BlankName,
            );
            continue;
        }
        let i = match category_by_name.get(&name_key(name)) {
            Some(&i) => {
                warn(
                    &mut warnings,
                    K::Categories,
                    row.line,
                    FoodicsIssue::Duplicate { name: name.into() },
                );
                i
            }
            None => {
                categories.push(BulkCategory {
                    key: format!("c{}", categories.len() + 1),
                    name: name.into(),
                    name_ar: row.opt("name_localized"),
                });
                category_by_name.insert(name_key(name), categories.len() - 1);
                categories.len() - 1
            }
        };
        if let Some(r) = row.opt("reference") {
            category_by_ref.entry(r).or_insert(i);
        }
    }
    let mut uncategorized = None;

    // Items: one per name; a SKU finds it (a duplicate's SKU finds the first).
    let mut items: Vec<BulkItem> = Vec::new();
    let mut item_by_name: HashMap<String, usize> = HashMap::new();
    let mut item_by_sku: HashMap<String, usize> = HashMap::new();
    for row in &products {
        let name = row.get("name");
        if name.is_empty() {
            warn(
                &mut warnings,
                K::Products,
                row.line,
                FoodicsIssue::BlankName,
            );
            continue;
        }
        let sku = row.get("sku");
        if let Some(&i) = item_by_name.get(&name_key(name)) {
            warn(
                &mut warnings,
                K::Products,
                row.line,
                FoodicsIssue::Duplicate { name: name.into() },
            );
            if !sku.is_empty() {
                item_by_sku.entry(sku.into()).or_insert(i);
            }
            continue;
        }
        if !sku.is_empty() && item_by_sku.contains_key(sku) {
            warn(
                &mut warnings,
                K::Products,
                row.line,
                FoodicsIssue::DuplicateSku { sku: sku.into() },
            );
            continue;
        }
        let Some(price) = piastres(row.get("price")) else {
            warn(
                &mut warnings,
                K::Products,
                row.line,
                FoodicsIssue::NotANumber {
                    column: "price".into(),
                    value: row.get("price").into(),
                },
            );
            continue;
        };
        let category = match category_by_ref.get(row.get("category_reference")) {
            Some(&c) => categories[c].key.clone(),
            None => {
                warn(
                    &mut warnings,
                    K::Products,
                    row.line,
                    FoodicsIssue::Uncategorized { item: name.into() },
                );
                let c = *uncategorized.get_or_insert_with(|| {
                    match category_by_name.get(&name_key(UNCATEGORIZED)) {
                        Some(&c) => c,
                        None => {
                            categories.push(BulkCategory {
                                key: format!("c{}", categories.len() + 1),
                                name: UNCATEGORIZED.into(),
                                name_ar: Some(UNCATEGORIZED_AR.into()),
                            });
                            categories.len() - 1
                        }
                    }
                });
                categories[c].key.clone()
            }
        };
        let is_active = yes_no(row, "is_active", K::Products, &mut warnings);
        items.push(BulkItem {
            name: name.into(),
            name_ar: row.opt("name_localized"),
            description: row.opt("description"),
            description_ar: row.opt("description_localized"),
            category,
            price,
            is_active,
            sizes: Vec::new(),
            groups: Vec::new(),
            optionals: Vec::new(),
        });
        item_by_name.insert(name_key(name), items.len() - 1);
        if !sku.is_empty() {
            item_by_sku.insert(sku.into(), items.len() - 1);
        }
    }

    // Groups: keyed by reference, else name; the groups file first, then the
    // links in file order.
    let gkey = |reference: &str, name: &str| {
        if reference.is_empty() {
            name.to_string()
        } else {
            reference.to_string()
        }
    };
    let mut groups: Vec<Group> = Vec::new();
    let mut group_at: HashMap<String, usize> = HashMap::new();
    let mut add_group =
        |groups: &mut Vec<Group>, key: String, name: &str, name_ar: Option<String>| {
            let i = *group_at.entry(key.clone()).or_insert_with(|| {
                groups.push(Group {
                    key,
                    name: name.into(),
                    name_ar: None,
                    options: Vec::new(),
                    uses: Vec::new(),
                });
                groups.len() - 1
            });
            if groups[i].name_ar.is_none() {
                groups[i].name_ar = name_ar;
            }
            i
        };
    for row in &groups_rows {
        let name = row.get("name");
        if name.is_empty() {
            warn(
                &mut warnings,
                K::ModifierGroups,
                row.line,
                FoodicsIssue::BlankName,
            );
            continue;
        }
        add_group(
            &mut groups,
            gkey(row.get("reference"), name),
            name,
            row.opt("name_localized"),
        );
    }
    let mut links: Vec<Link> = Vec::new();
    let mut linked: HashSet<(usize, String)> = HashSet::new();
    for row in &links_rows {
        let name = row.get("modifier_name");
        if name.is_empty() {
            warn(
                &mut warnings,
                K::ProductModifiers,
                row.line,
                FoodicsIssue::BlankName,
            );
            continue;
        }
        let sku = row.get("product_sku");
        let Some(&item) = item_by_sku.get(sku) else {
            warn(
                &mut warnings,
                K::ProductModifiers,
                row.line,
                FoodicsIssue::UnknownSku { sku: sku.into() },
            );
            continue;
        };
        let (Some(min), Some(max)) = (
            whole(row.get("minimum_options")),
            whole(row.get("maximum_options")),
        ) else {
            warn(
                &mut warnings,
                K::ProductModifiers,
                row.line,
                FoodicsIssue::NotANumber {
                    column: "minimum_options/maximum_options".into(),
                    value: format!(
                        "{}/{}",
                        row.get("minimum_options"),
                        row.get("maximum_options")
                    ),
                },
            );
            continue;
        };
        let max = Some(max).filter(|&m| m > 0);
        let key = gkey(row.get("modifier_reference"), name);
        let g = add_group(
            &mut groups,
            key.clone(),
            name,
            row.opt("modifier_name_localized"),
        );
        if !linked.insert((item, key.clone())) {
            warn(
                &mut warnings,
                K::ProductModifiers,
                row.line,
                FoodicsIssue::DuplicateLink {
                    item: items[item].name.clone(),
                    group: name.into(),
                },
            );
            continue;
        }
        groups[g].uses.push((min, max));
        links.push(Link {
            item,
            group: key,
            min,
            max,
            line: row.line,
        });
    }

    // Options: one per name within a group.
    for row in &options_rows {
        let name = row.get("name");
        if name.is_empty() {
            warn(
                &mut warnings,
                K::ModifierOptions,
                row.line,
                FoodicsIssue::BlankName,
            );
            continue;
        }
        let key = gkey(row.get("modifier_reference"), row.get("modifier_name"));
        let Some(&g) = group_at.get(&key) else {
            warn(
                &mut warnings,
                K::ModifierOptions,
                row.line,
                FoodicsIssue::UnknownGroup { group: key },
            );
            continue;
        };
        if groups[g]
            .options
            .iter()
            .any(|o| name_key(&o.name) == name_key(name))
        {
            warn(
                &mut warnings,
                K::ModifierOptions,
                row.line,
                FoodicsIssue::Duplicate { name: name.into() },
            );
            continue;
        }
        let Some(price) = piastres(row.get("price")) else {
            warn(
                &mut warnings,
                K::ModifierOptions,
                row.line,
                FoodicsIssue::NotANumber {
                    column: "price".into(),
                    value: row.get("price").into(),
                },
            );
            continue;
        };
        let is_active = yes_no(row, "is_active", K::ModifierOptions, &mut warnings);
        groups[g].options.push(BulkOption {
            name: name.into(),
            name_ar: row.opt("name_localized"),
            price,
            is_active,
        });
    }

    // Which groups are sizes.
    let mut sizes: HashSet<String> = HashSet::new();
    for g in &groups {
        let named = SIZE_GROUP_NAMES.contains(&name_key(&g.name).as_str());
        let looks = !g.uses.is_empty()
            && g.uses.iter().all(|&u| u == (1, Some(1)))
            && (2..=6).contains(&g.options.len())
            && g.options.iter().all(|o| size_like(&o.name));
        if g.key == "size" {
            sizes.insert(g.key.clone());
        } else if named && looks {
            sizes.insert(g.key.clone());
            warnings.push(FoodicsWarning {
                file: None,
                line: None,
                issue: FoodicsIssue::SizesByName {
                    group: g.name.clone(),
                },
            });
        } else if named {
            warnings.push(FoodicsWarning {
                file: None,
                line: None,
                issue: FoodicsIssue::LooksLikeSizes {
                    group: g.name.clone(),
                },
            });
        }
    }

    // The groups that stay groups, one per name, with options.
    let mut out_groups: Vec<BulkGroup> = Vec::new();
    let mut group_names: HashSet<String> = HashSet::new();
    let mut kept: HashMap<String, usize> = HashMap::new();
    for g in &groups {
        if g.options.is_empty() {
            warnings.push(FoodicsWarning {
                file: None,
                line: None,
                issue: FoodicsIssue::EmptyGroup {
                    group: g.name.clone(),
                },
            });
            continue;
        }
        if sizes.contains(&g.key) {
            continue;
        }
        if !group_names.insert(name_key(&g.name)) {
            warnings.push(FoodicsWarning {
                file: None,
                line: None,
                issue: FoodicsIssue::DuplicateGroup {
                    name: g.name.clone(),
                },
            });
            continue;
        }
        let (min, max) = usual_limits(&g.uses);
        let active =
            u32::try_from(g.options.iter().filter(|o| o.is_active).count()).unwrap_or(u32::MAX);
        // Each product's link that asks for too many warns on its own.
        let min = min.min(active);
        let family = match g.key.as_str() {
            "milk_type" => BulkFamily::MilkType,
            "coffee_type" => BulkFamily::CoffeeType,
            "extra" => BulkFamily::Extra,
            _ => {
                let n = name_key(&g.name);
                if n.contains("milk") || n.contains("حليب") {
                    BulkFamily::MilkType
                } else if n.contains("bean") || n.contains("حبوب") {
                    BulkFamily::CoffeeType
                } else {
                    BulkFamily::Extra
                }
            }
        };
        kept.insert(g.key.clone(), out_groups.len());
        out_groups.push(BulkGroup {
            key: format!("g{}", out_groups.len() + 1),
            name: g.name.clone(),
            name_ar: g.name_ar.clone(),
            min,
            max,
            family,
            options: g.options.clone(),
        });
    }

    // Links: sizes onto the item, the rest as its groups.
    for l in &links {
        let item = &mut items[l.item];
        if sizes.contains(&l.group) {
            let g = &groups[group_at[&l.group]];
            if !item.sizes.is_empty() {
                warn(
                    &mut warnings,
                    K::ProductModifiers,
                    l.line,
                    FoodicsIssue::SecondSizes {
                        item: item.name.clone(),
                        group: g.name.clone(),
                    },
                );
                continue;
            }
            item.sizes = g
                .options
                .iter()
                .map(|o| BulkSize {
                    label: o.name.clone(),
                    price: item.price + o.price,
                    is_active: o.is_active,
                })
                .collect();
            continue;
        }
        let Some(&g) = kept.get(&l.group) else {
            continue;
        };
        let group = &out_groups[g];
        let active =
            u32::try_from(group.options.iter().filter(|o| o.is_active).count()).unwrap_or(u32::MAX);
        let min = lowered(l, active, &group.name, &item.name, &mut warnings);
        item.groups.push(BulkLink {
            group: group.key.clone(),
            min,
            max: l.max,
        });
    }

    // Per-product extras (Madar's own file).
    for row in &optionals_rows {
        let name = row.get("name");
        if name.is_empty() {
            warn(
                &mut warnings,
                K::ProductOptionals,
                row.line,
                FoodicsIssue::BlankName,
            );
            continue;
        }
        let sku = row.get("product_sku");
        let Some(&i) = item_by_sku.get(sku) else {
            warn(
                &mut warnings,
                K::ProductOptionals,
                row.line,
                FoodicsIssue::UnknownSku { sku: sku.into() },
            );
            continue;
        };
        let Some(price) = piastres(row.get("price")) else {
            warn(
                &mut warnings,
                K::ProductOptionals,
                row.line,
                FoodicsIssue::NotANumber {
                    column: "price".into(),
                    value: row.get("price").into(),
                },
            );
            continue;
        };
        if items[i]
            .optionals
            .iter()
            .any(|o| name_key(&o.name) == name_key(name))
        {
            warn(
                &mut warnings,
                K::ProductOptionals,
                row.line,
                FoodicsIssue::Duplicate { name: name.into() },
            );
            continue;
        }
        items[i].optionals.push(BulkOptional {
            name: name.into(),
            name_ar: row.opt("name_localized"),
            price,
        });
    }

    for kind in [
        K::Categories,
        K::ProductModifiers,
        K::ModifierGroups,
        K::ModifierOptions,
    ] {
        if !kinds.contains(&kind) {
            warnings.push(FoodicsWarning {
                file: None,
                line: None,
                issue: FoodicsIssue::MissingFile { kind },
            });
        }
    }
    Ok(FoodicsParsed {
        menu: BulkMenu {
            version: VERSION,
            categories,
            groups: out_groups,
            items,
        },
        files: kinds,
        warnings,
    })
}

fn warn(warnings: &mut Vec<FoodicsWarning>, file: FoodicsFile, line: u64, issue: FoodicsIssue) {
    warnings.push(FoodicsWarning {
        file: Some(file),
        line: Some(line),
        issue,
    });
}

/// The (min, max) most of a group's products use; ties go to the lower min,
/// then the lower max, with no maximum last. A group no product uses is
/// optional with no maximum.
fn usual_limits(uses: &[(u32, Option<u32>)]) -> (u32, Option<u32>) {
    let mut counts: Vec<((u32, Option<u32>), usize)> = Vec::new();
    for &u in uses {
        match counts.iter_mut().find(|(v, _)| *v == u) {
            Some((_, n)) => *n += 1,
            None => counts.push((u, 1)),
        }
    }
    counts
        .into_iter()
        .min_by_key(|&((min, max), n)| (std::cmp::Reverse(n), min, max.unwrap_or(u32::MAX)))
        .map_or((0, None), |(v, _)| v)
}

/// A link's minimum, lowered to the group's active options when it asks for
/// more than there are.
fn lowered(
    link: &Link,
    active: u32,
    group: &str,
    item: &str,
    warnings: &mut Vec<FoodicsWarning>,
) -> u32 {
    if link.min <= active {
        return link.min;
    }
    let issue = FoodicsIssue::MinLowered {
        group: group.into(),
        item: item.into(),
        from: link.min,
        to: active,
    };
    warn(warnings, FoodicsFile::ProductModifiers, link.line, issue);
    active
}

#[cfg(test)]
mod tests;
