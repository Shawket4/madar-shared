use super::*;
use crate::bulk::validate;

const CATEGORIES: &str =
    "id,name,name_localized,reference,image\n,Coffee,قهوة,cat-1,\n,Cold,بارد,cat-2,\n";
const PRODUCTS_HEAD: &str = "id,name,sku,category_reference,tax_group_reference,is_sold_by_weight,is_active,is_stock_product,price,barcode,description,preparation_time,calories,walking_minutes_to_burn_calories,is_high_salt,image,name_localized,description_localized,ereceipt_item_type,ereceipt_item_code,ereceipt_unit_type\n";
const LINKS_HEAD: &str = "product_name,product_name_localized,product_sku,modifier_name,modifier_name_localized,modifier_reference,minimum_options,maximum_options,free_options,default_options,unique_options\n";
const GROUPS_HEAD: &str = "id,reference,name,name_localized\n";
const OPTIONS_HEAD: &str = "id,modifier_reference,modifier_name,modifier_name_localized,tax_group_reference,name,sku,price,calories,name_localized,is_active\n";

fn product(name: &str, sku: &str, cat: &str, price: &str, ar: &str) -> String {
    format!(",\"{name}\",{sku},{cat},,No,Yes,No,{price},,,,,,No,,{ar},,,,\n")
}

fn link(sku: &str, group: &str, reference: &str, min: &str, max: &str) -> String {
    format!(",,{sku},{group},,{reference},{min},{max},0,,No\n")
}

fn option(reference: &str, group: &str, name: &str, price: &str, ar: &str) -> String {
    format!(",{reference},{group},,,{name},,{price},,{ar},Yes\n")
}

/// A café keyed by name, like a raw export (no references on groups).
fn by_name() -> Vec<String> {
    let products = [
        PRODUCTS_HEAD.to_string(),
        product("Espresso", "p-1", "cat-1", "110", "إسبريسو"),
        product("Latte", "p-2", "cat-1", "150", "لاتيه"),
        product("Iced Latte", "p-3", "cat-2", "165.5", ""),
    ]
    .concat();
    let links = [
        LINKS_HEAD.to_string(),
        link("p-1", "Extra Flavors", "", "0", "2"),
        link("p-2", "Milk", "", "1", "1"),
        link("p-2", "Extra Flavors", "", "0", "2"),
        link("p-3", "Milk", "", "1", "1"),
    ]
    .concat();
    let groups = format!("{GROUPS_HEAD},,Milk,الحليب\n,,Extra Flavors,نكهات إضافية\n");
    let options = [
        OPTIONS_HEAD.to_string(),
        option("", "Milk", "Full Cream", "0", "كامل الدسم"),
        option("", "Milk", "Oat", "45", "شوفان"),
        option("", "Extra Flavors", "Vanilla", "20", ""),
        option("", "Extra Flavors", "Caramel", "20", ""),
    ]
    .concat();
    vec![CATEGORIES.into(), products, links, groups, options]
}

fn parse_all(files: &[String]) -> FoodicsParsed {
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    parse(&refs).unwrap()
}

fn issues(p: &FoodicsParsed) -> Vec<&FoodicsIssue> {
    p.warnings.iter().map(|w| &w.issue).collect()
}

#[test]
fn a_menu_keyed_by_name_reads_whole_and_valid() {
    let p = parse_all(&by_name());
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    let m = &p.menu;
    assert_eq!(validate(m).unwrap().items, 3);
    assert_eq!(
        m.categories
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["Coffee", "Cold"]
    );
    assert_eq!(m.categories[0].name_ar.as_deref(), Some("قهوة"));

    let milk = m.groups.iter().find(|g| g.name == "Milk").unwrap();
    assert_eq!(
        (milk.min, milk.max, milk.family),
        (1, Some(1), BulkFamily::MilkType)
    );
    assert_eq!(milk.name_ar.as_deref(), Some("الحليب"));
    assert_eq!(milk.options[1].price, 4500);
    let extra = m.groups.iter().find(|g| g.name == "Extra Flavors").unwrap();
    assert_eq!(
        (extra.min, extra.max, extra.family),
        (0, Some(2), BulkFamily::Extra)
    );

    let latte = m.items.iter().find(|i| i.name == "Latte").unwrap();
    assert_eq!(latte.price, 15000);
    assert_eq!(latte.category, m.categories[0].key);
    assert_eq!(latte.groups.len(), 2);
    assert!(latte.sizes.is_empty());
    // Half a piastre rounds up, like the SQL's round().
    assert_eq!(m.items[2].price, 16550);
    assert_eq!(m.items[2].name_ar, None);
}

/// Groups keyed by reference, like a cleaned export: `size` and `milk_type`.
fn keyed() -> Vec<String> {
    let products = [
        PRODUCTS_HEAD.to_string(),
        product("Spanish latte", "psk-10", "cat-1", "100", ""),
    ]
    .concat();
    let links = [
        LINKS_HEAD.to_string(),
        link("psk-10", "Size", "size", "1", "1"),
        link("psk-10", "Milk", "milk_type", "1", "1"),
    ]
    .concat();
    let groups = format!("{GROUPS_HEAD},size,Size,الحجم\n,milk_type,Milk,الحليب\n");
    let options = [
        OPTIONS_HEAD.to_string(),
        option("size", "Size", "Cup", "0", "كوب"),
        option("size", "Size", "Can", "25", "كان"),
        option("milk_type", "Milk", "Full Cream", "0", ""),
    ]
    .concat();
    vec![CATEGORIES.into(), products, links, groups, options]
}

#[test]
fn the_size_reference_makes_sizes_at_the_full_price() {
    let p = parse_all(&keyed());
    let item = &p.menu.items[0];
    assert_eq!(
        item.sizes,
        [
            BulkSize {
                label: "Cup".into(),
                price: 10000,
                is_active: true
            },
            BulkSize {
                label: "Can".into(),
                price: 12500,
                is_active: true
            },
        ]
    );
    assert_eq!(
        p.menu.groups.len(),
        1,
        "the size group is not a modifier group"
    );
    assert_eq!(item.groups.len(), 1);
    validate(&p.menu).unwrap();
}

/// One product linked to a group named "Size" with no reference.
fn size_files(options: &[(&str, &str)], min: &str, max: &str) -> Vec<String> {
    let products = [
        PRODUCTS_HEAD.to_string(),
        product("Latte", "p-1", "cat-1", "100", ""),
    ]
    .concat();
    let links = [LINKS_HEAD.to_string(), link("p-1", "Size", "", min, max)].concat();
    let opts: String = options
        .iter()
        .map(|(n, p)| option("", "Size", n, p, ""))
        .collect();
    vec![
        CATEGORIES.into(),
        products,
        links,
        format!("{OPTIONS_HEAD}{opts}"),
    ]
}

fn size_named(options: &[(&str, &str)], min: &str, max: &str) -> FoodicsParsed {
    parse_all(&size_files(options, min, max))
}

#[test]
fn a_group_named_size_that_looks_like_sizes_is_sizes() {
    let p = size_named(
        &[("Small", "0"), ("Medium", "10"), ("Large", "20")],
        "1",
        "1",
    );
    assert!(issues(&p).contains(&&FoodicsIssue::SizesByName {
        group: "Size".into()
    }));
    assert_eq!(
        p.menu.items[0]
            .sizes
            .iter()
            .map(|s| s.price)
            .collect::<Vec<_>>(),
        [10000, 11000, 12000]
    );
    assert!(p.menu.groups.is_empty());
    // Arabic and ounce labels count as sizes too.
    let p = size_named(&[("صغير", "0"), ("12 oz", "10")], "1", "1");
    assert_eq!(p.menu.items[0].sizes.len(), 2);
}

#[test]
fn a_group_named_size_that_does_not_look_like_sizes_stays_a_group() {
    // Options that are not sizes.
    let p = size_named(&[("Hot", "0"), ("Iced", "10")], "1", "1");
    assert!(issues(&p).contains(&&FoodicsIssue::LooksLikeSizes {
        group: "Size".into()
    }));
    assert!(p.menu.items[0].sizes.is_empty());
    assert_eq!(p.menu.groups.len(), 1);
    // Pick-many is not sizes.
    let p = size_named(&[("Small", "0"), ("Large", "10")], "0", "2");
    assert!(issues(&p).contains(&&FoodicsIssue::LooksLikeSizes {
        group: "Size".into()
    }));
    validate(&p.menu).unwrap();
}

#[test]
fn files_are_known_by_header_in_any_order_with_a_bom_and_crlf() {
    let mut files = by_name();
    files.reverse();
    files[0] = format!("\u{feff}{}", files[0].replace('\n', "\r\n"));
    let p = parse_all(&files);
    assert_eq!(p.menu.items.len(), 3);
    assert_eq!(p.files.len(), 5);
    assert_eq!(detect(CATEGORIES), Some(FoodicsFile::Categories));
    assert_eq!(detect(GROUPS_HEAD), Some(FoodicsFile::ModifierGroups));
}

#[test]
fn files_that_cannot_be_read_are_refused() {
    assert_eq!(
        parse(&["name,price\nx,1\n"]),
        Err(FoodicsError::UnknownFile { index: 0 })
    );
    assert_eq!(parse(&[CATEGORIES]), Err(FoodicsError::MissingProducts));
    assert_eq!(
        parse(&[CATEGORIES, CATEGORIES]),
        Err(FoodicsError::DuplicateFile {
            kind: FoodicsFile::Categories
        })
    );
    let huge = format!("{PRODUCTS_HEAD}{}", "x".repeat(MAX_FILE_BYTES));
    assert!(matches!(parse(&[&huge]), Err(FoodicsError::TooBig { .. })));
    let rows = format!("{PRODUCTS_HEAD}{}", ",x\n".repeat(MAX_FILE_ROWS + 1));
    assert!(matches!(parse(&[&rows]), Err(FoodicsError::TooBig { .. })));
}

/// One of each fault the reader skips or changes, next to good rows.
fn bad_rows() -> Vec<String> {
    let products = [
        PRODUCTS_HEAD.to_string(),
        product("Latte", "p-1", "cat-1", "100", ""),
        product("latte ", "p-9", "cat-1", "100", ""),
        product("Mocha", "p-2", "nope", "100", ""),
        product("Cake", "p-3", "cat-1", "abc", ""),
        product("", "p-4", "cat-1", "1", ""),
    ]
    .concat();
    let links = [
        LINKS_HEAD.to_string(),
        link("p-1", "Milk", "", "3", "3"),
        link("p-404", "Milk", "", "1", "1"),
        link("p-1", "Empty", "", "0", "1"),
    ]
    .concat();
    let options = [
        OPTIONS_HEAD.to_string(),
        option("", "Milk", "Oat", "0", ""),
        option("", "Ghost", "X", "0", ""),
    ]
    .concat();
    vec![CATEGORIES.into(), products, links, options]
}

#[test]
fn bad_rows_are_skipped_with_a_warning_and_the_rest_still_reads() {
    let p = parse_all(&bad_rows());
    let got = issues(&p);
    assert!(got.contains(&&FoodicsIssue::Duplicate {
        name: "latte".into()
    }));
    assert!(got.contains(&&FoodicsIssue::Uncategorized {
        item: "Mocha".into()
    }));
    assert!(got.contains(&&FoodicsIssue::NotANumber {
        column: "price".into(),
        value: "abc".into()
    }));
    assert!(got.contains(&&FoodicsIssue::BlankName));
    assert!(got.contains(&&FoodicsIssue::UnknownSku {
        sku: "p-404".into()
    }));
    assert!(got.contains(&&FoodicsIssue::UnknownGroup {
        group: "Ghost".into()
    }));
    assert!(got.contains(&&FoodicsIssue::EmptyGroup {
        group: "Empty".into()
    }));
    // Once, on the product's link (the group's own limits follow its links).
    let lowered: Vec<&FoodicsWarning> = p
        .warnings
        .iter()
        .filter(|w| matches!(w.issue, FoodicsIssue::MinLowered { .. }))
        .collect();
    assert_eq!(
        lowered,
        [&FoodicsWarning {
            file: Some(FoodicsFile::ProductModifiers),
            line: Some(2),
            issue: FoodicsIssue::MinLowered {
                group: "Milk".into(),
                item: "Latte".into(),
                from: 3,
                to: 1
            }
        }]
    );
    assert!(got.contains(&&FoodicsIssue::MissingFile {
        kind: FoodicsFile::ModifierGroups
    }));

    let m = &p.menu;
    assert_eq!(
        m.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        ["Latte", "Mocha"]
    );
    let uncategorized = m
        .categories
        .iter()
        .find(|c| c.name == "Uncategorized")
        .unwrap();
    assert_eq!(m.items[1].category, uncategorized.key);
    // The duplicate's SKU finds the first Latte.
    assert_eq!(m.items[0].groups.len(), 1);
    validate(m).unwrap();
}

#[test]
fn prices_read_like_the_sql() {
    assert_eq!(piastres(""), Some(0));
    assert_eq!(piastres("110"), Some(11000));
    assert_eq!(piastres("12.5"), Some(1250));
    assert_eq!(piastres("12.345"), Some(1235));
    assert_eq!(piastres("12.344"), Some(1234));
    assert_eq!(piastres(".5"), Some(50));
    for bad in ["abc", "-5", "1,200", "1.2.3", ".", "1e3", "9999999999"] {
        assert_eq!(piastres(bad), None, "{bad}");
    }
}

#[test]
fn a_groups_limits_are_the_ones_most_products_use() {
    assert_eq!(
        usual_limits(&[(1, Some(1)), (0, Some(2)), (1, Some(1))]),
        (1, Some(1))
    );
    // A tie goes to the lower min, then the lower max, no maximum last.
    assert_eq!(usual_limits(&[(1, Some(1)), (0, Some(2))]), (0, Some(2)));
    assert_eq!(usual_limits(&[(0, None), (0, Some(5))]), (0, Some(5)));
    assert_eq!(usual_limits(&[]), (0, None));
}

#[test]
fn a_zero_maximum_is_no_maximum() {
    let products = [
        PRODUCTS_HEAD.to_string(),
        product("Latte", "p-1", "cat-1", "100", ""),
    ]
    .concat();
    let links = [LINKS_HEAD.to_string(), link("p-1", "Extras", "", "0", "0")].concat();
    let options = [
        OPTIONS_HEAD.to_string(),
        option("", "Extras", "Shot", "20", ""),
    ]
    .concat();
    let p = parse_all(&[CATEGORIES.into(), products, links, options]);
    assert_eq!(p.menu.groups[0].max, None);
    validate(&p.menu).unwrap();
}

// ── menu_import_vectors.json: the web and Flutter builds' pin ──────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct VectorFile {
    about: String,
    foodics: Vec<FoodicsCase>,
    validate: Vec<ValidateCase>,
    sentences: Vec<SentenceCase>,
    confirm: Vec<ConfirmCase>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SentenceCase {
    org_name: String,
    lang: String,
    expected: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ConfirmCase {
    name: String,
    org_name: String,
    typed: String,
    expected: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FoodicsCase {
    name: String,
    about: String,
    files: Vec<String>,
    /// A `FoodicsParsed`, or the `FoodicsError` (as the browser build returns it).
    expected: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ValidateCase {
    name: String,
    about: String,
    menu: crate::BulkMenu,
    /// A `BulkSummary`, or the `BulkRefusal`.
    expected: serde_json::Value,
}

fn either<T: serde::Serialize, E: serde::Serialize>(r: Result<T, E>) -> serde_json::Value {
    match r {
        Ok(v) => serde_json::to_value(v),
        Err(e) => serde_json::to_value(e),
    }
    .unwrap()
}

fn foodics_answer(files: &[String]) -> serde_json::Value {
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    either(parse(&refs))
}

fn vector_file() -> VectorFile {
    let foodics = |name: &str, about: &str, files: Vec<String>| FoodicsCase {
        name: name.into(),
        about: about.into(),
        expected: foodics_answer(&files),
        files,
    };
    let check = |name: &str, about: &str, menu: crate::BulkMenu| ValidateCase {
        name: name.into(),
        about: about.into(),
        expected: either(validate(&menu)),
        menu,
    };
    let mut crlf = by_name();
    crlf.reverse();
    crlf[0] = format!("\u{feff}{}", crlf[0].replace('\n', "\r\n"));
    let zero_max = vec![
        CATEGORIES.to_string(),
        [
            PRODUCTS_HEAD.to_string(),
            product("Latte", "p-1", "cat-1", "100", ""),
        ]
        .concat(),
        [LINKS_HEAD.to_string(), link("p-1", "Extras", "", "0", "0")].concat(),
        [
            OPTIONS_HEAD.to_string(),
            option("", "Extras", "Shot", "20", ""),
        ]
        .concat(),
    ];

    let base = parse_all(&by_name()).menu;
    let with = |f: &dyn Fn(&mut crate::BulkMenu)| {
        let mut m = base.clone();
        f(&mut m);
        m
    };

    VectorFile {
        about: "madar-menu: foodics::parse over Foodics console exports (each case's files, \
                in the order passed), validate over bulk menus, and the replace sentence \
                 (replace_sentence, confirms_replace). Inputs are hand-written \
                (crates/madar-menu/src/foodics/tests.rs); expected answers are this crate's, \
                written by MADAR_REGENERATE_MENU_VECTORS=1 and reviewed by hand. A refusal \
                or error is the expected value itself, as the browser build returns it."
            .into(),
        foodics: vec![
            foodics("by_name", "groups keyed by name (a raw export): Milk is milk_type 1..1, Extra Flavors extra 0..2; 165.5 EGP is 16550 piastres; no Arabic stays null", by_name()),
            foodics("size_reference", "reference `size` makes item sizes at the product's price plus the option's; the size group is not a modifier group", keyed()),
            foodics("sizes_by_name", "a group named Size, pick exactly one everywhere, options named like sizes: sizes, with a sizes_by_name warning", size_files(&[("Small", "0"), ("Medium", "10"), ("Large", "20")], "1", "1")),
            foodics("sizes_by_name_arabic_and_ounces", "Arabic and ounce labels count as sizes", size_files(&[("صغير", "0"), ("12 oz", "10")], "1", "1")),
            foodics("looks_like_sizes_hot_iced", "named Size but its options are not sizes: kept as a group, looks_like_sizes warning", size_files(&[("Hot", "0"), ("Iced", "10")], "1", "1")),
            foodics("looks_like_sizes_pick_many", "named Size but pick-many: kept as a group", size_files(&[("Small", "0"), ("Large", "10")], "0", "2")),
            foodics("bad_rows", "one of each skipped or changed row next to good ones; the duplicate's SKU finds the first Latte", bad_rows()),
            foodics("bom_crlf_any_order", "files reversed, the first with a BOM and CRLF line ends: the same menu as by_name", crlf),
            foodics("zero_max_is_no_max", "maximum_options 0 means no maximum", zero_max),
            foodics("unknown_file", "a file whose header is no Foodics menu export", vec!["name,price\nx,1\n".into()]),
            foodics("missing_products", "no products file", vec![CATEGORIES.into()]),
            foodics("duplicate_file", "two categories files", vec![CATEGORIES.into(), CATEGORIES.into()]),
        ],
        sentences: [("Drops", "en"), ("Drops", "ar"), (" Pachamama  Cafe ", "en"), ("دروبس", "ar")]
            .iter()
            .map(|(org, lang)| SentenceCase {
                org_name: (*org).into(),
                lang: (*lang).into(),
                expected: crate::replace_sentence(org, lang),
            })
            .collect(),
        confirm: [
            ("english", "Drops", "Replace the menu of Drops"),
            ("arabic", "Drops", "استبدل قائمة Drops"),
            ("case_and_spaces", "Drops", "  replace the MENU of   drops "),
            ("another_org", "Drops", "Replace the menu of Drop"),
            ("org_name_alone", "Drops", "Drops"),
            ("empty", "Drops", ""),
        ]
        .iter()
        .map(|(name, org, typed)| ConfirmCase {
            name: (*name).into(),
            org_name: (*org).into(),
            typed: (*typed).into(),
            expected: crate::confirms_replace(org, typed),
        })
        .collect(),
        validate: vec![
            check("whole_menu", "by_name's menu passes, counted", base.clone()),
            check("unsupported_version", "a version this crate does not read", with(&|m| m.version = 2)),
            check("empty", "no items", with(&|m| m.items.clear())),
            check("duplicate_item_by_case", "names compare trimmed, spaces folded, case ignored", with(&|m| {
                let mut twin = m.items[0].clone();
                twin.name = "  ESPRESSO ".into();
                m.items.push(twin);
            })),
            check("unknown_category", "an item names a category key not in the menu", with(&|m| m.items[0].category = "c404".into())),
            check("unknown_group", "an item names a group key not in the menu", with(&|m| m.items[1].groups[0].group = "g404".into())),
            check("unmeetable_min", "a link requiring more picks than the group has active options", with(&|m| m.items[1].groups[0].min = 5)),
            check("max_under_min", "a group maximum under its minimum", with(&|m| {
                m.groups[0].min = 2;
                m.groups[0].max = Some(1);
            })),
            check("control_character", "a bell in an item name", with(&|m| m.items[0].name = "Espresso\u{7}".into())),
            check("name_too_long_in_characters", "121 Arabic letters: the limit counts characters, not bytes", with(&|m| m.items[0].name = "ق".repeat(121))),
            check("name_at_the_limit", "120 Arabic letters pass", with(&|m| m.items[0].name = "ق".repeat(120))),
            check("reserved_size_label", "one_size is the server's hidden size", with(&|m| {
                m.items[0].sizes = vec![BulkSize { label: "one_size".into(), price: 100, is_active: true }];
            })),
            check("negative_price", "an option at -1 EGP", with(&|m| m.groups[0].options[0].price = -100)),
        ],
    }
}

#[test]
fn menu_import_vectors() {
    let file = vector_file();
    if std::env::var("MADAR_REGENERATE_MENU_VECTORS").is_ok() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/vectors/menu_import_vectors.json"
        );
        std::fs::write(path, serde_json::to_string_pretty(&file).unwrap() + "\n").unwrap();
        return;
    }
    let pinned: VectorFile = serde_json::from_str(crate::vectors::MENU_IMPORT).unwrap();
    for c in &pinned.foodics {
        assert_eq!(foodics_answer(&c.files), c.expected, "{}", c.name);
    }
    for c in &pinned.validate {
        assert_eq!(either(validate(&c.menu)), c.expected, "{}", c.name);
    }
    for c in &pinned.sentences {
        assert_eq!(crate::replace_sentence(&c.org_name, &c.lang), c.expected);
    }
    for c in &pinned.confirm {
        assert_eq!(
            crate::confirms_replace(&c.org_name, &c.typed),
            c.expected,
            "{}",
            c.name
        );
    }
    // The pinned inputs are this file's fixtures: an edit to either shows.
    assert_eq!(
        serde_json::to_value(&pinned).unwrap(),
        serde_json::to_value(&file).unwrap(),
        "stale: MADAR_REGENERATE_MENU_VECTORS=1 cargo test -p madar-menu --features foodics menu_import_vectors"
    );
}

#[test]
fn a_size_group_with_no_options_warns_and_leaves_the_price() {
    let products = [
        PRODUCTS_HEAD.to_string(),
        product("Latte", "p-1", "cat-1", "100", ""),
    ]
    .concat();
    let links = [
        LINKS_HEAD.to_string(),
        link("p-1", "Size", "size", "1", "1"),
    ]
    .concat();
    let p = parse_all(&[CATEGORIES.into(), products, links]);
    assert!(issues(&p).contains(&&FoodicsIssue::EmptyGroup {
        group: "Size".into()
    }));
    assert!(p.menu.items[0].sizes.is_empty());
    assert_eq!(p.menu.items[0].price, 10000);
    validate(&p.menu).unwrap();
}
