//! The sentence an owner types to replace a whole menu (`POST /menu/bulk`,
//! mode `replace`). Both dashboards show it in the reader's language; the
//! server takes it in either, compared like names ([`name_key`]).

use crate::bulk::name_key;

/// The sentence to type: Arabic for `lang` `"ar"`, else English.
pub fn replace_sentence(org_name: &str, lang: &str) -> String {
    let org = org_name.split_whitespace().collect::<Vec<_>>().join(" ");
    if lang == "ar" {
        format!("استبدل قائمة {org}")
    } else {
        format!("Replace the menu of {org}")
    }
}

/// Whether `typed` is the sentence, in either language.
pub fn confirms_replace(org_name: &str, typed: &str) -> bool {
    let typed = name_key(typed);
    ["en", "ar"]
        .iter()
        .any(|lang| name_key(&replace_sentence(org_name, lang)) == typed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_language_confirms_compared_like_names() {
        assert_eq!(
            replace_sentence(" Drops ", "en"),
            "Replace the menu of Drops"
        );
        assert_eq!(replace_sentence("Drops", "ar"), "استبدل قائمة Drops");
        assert!(confirms_replace("Drops", "  replace the MENU of   drops "));
        assert!(confirms_replace("Drops", "استبدل قائمة Drops"));
        assert!(!confirms_replace("Drops", "Replace the menu of Drop"));
        assert!(!confirms_replace("Drops", "Drops"));
        assert!(!confirms_replace("Drops", ""));
    }
}
