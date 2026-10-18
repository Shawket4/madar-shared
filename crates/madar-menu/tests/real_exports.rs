//! Real Foodics exports, run only where they are: customers' menus never go
//! in this public repo. `MADAR_FOODICS_DIR=<dir> cargo test -p madar-menu
//! --features foodics --test real_exports -- --nocapture` reads every
//! sub-folder of `<dir>` that holds a products.csv.
#![cfg(feature = "foodics")]

use madar_menu::foodics::parse;
use madar_menu::validate;

#[test]
fn every_real_export_reads_into_a_valid_menu() {
    let Ok(dir) = std::env::var("MADAR_FOODICS_DIR") else {
        eprintln!("MADAR_FOODICS_DIR not set; skipped");
        return;
    };
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("MADAR_FOODICS_DIR is a folder") {
        let folder = entry.expect("readable").path();
        if !folder.join("products.csv").exists() {
            continue;
        }
        let texts: Vec<String> = [
            "categories",
            "products",
            "product_modifiers",
            "modifier_groups",
            "modifier_options",
            "product_optionals",
        ]
        .iter()
        .filter_map(|n| std::fs::read_to_string(folder.join(format!("{n}.csv"))).ok())
        .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let parsed = parse(&refs).unwrap_or_else(|e| panic!("{}: {e}", folder.display()));
        let summary =
            validate(&parsed.menu).unwrap_or_else(|e| panic!("{}: {e}", folder.display()));
        println!("{}: {summary:?}", folder.display());
        for w in &parsed.warnings {
            println!("  {w:?}");
        }
        seen += 1;
    }
    assert!(seen > 0, "no export folders under {dir}");
}
