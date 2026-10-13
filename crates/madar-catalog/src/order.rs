//! The menu's order: the order a category list is shown in.
//!
//! **The POS's rule** (madar-core `menu::categories`, since the dashboard's
//! drag-and-drop ordering): categories by `display_order` (lower first; a
//! category without one is 0, as everything that predates ordering is),
//! ties broken by the name the reader sees — resolved for their locale
//! ([`resolve_name`]) — compared by Unicode code point (Rust's `str` order,
//! not a locale collation), and an equal name by `id`. The public menu
//! (MadarRust `load_public_menu`) and the customer pages (madar-web
//! `category_order`) follow it, so a shop's menu reads the same on the till,
//! the QR menu and the online store.
//!
//! Menu items carry no order of their own (`display_order` was dropped from
//! `menu_items` in 2026-06): every reader shows them in the order the server
//! lists them.
//!
//! Pinned by the hand-computed `vectors/menu_order_vectors.json`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What one category is ordered by: its position and the name the reader
/// sees (already resolved for their locale).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key<'a> {
    pub display_order: i32,
    pub name: &'a str,
    pub id: &'a str,
}

/// The rule: `display_order`, then the name by code point, then `id`.
pub fn cmp(a: &Key<'_>, b: &Key<'_>) -> Ordering {
    a.display_order
        .cmp(&b.display_order)
        .then_with(|| a.name.cmp(b.name))
        .then_with(|| a.id.cmp(b.id))
}

/// Sort `rows` by the rule, each row's [`Key`] read by `key` (names already
/// resolved, as a reader that holds localized views has them).
pub fn sort<T>(rows: &mut [T], key: impl Fn(&T) -> Key<'_>) {
    rows.sort_by(|a, b| cmp(&key(a), &key(b)));
}

/// The name a reader in `locale` sees: the translation for the exact locale
/// (`ar-EG`), else its language (`ar`), else English, else the base name. An
/// empty or non-text translation is no translation. (madar-core `menu::resolve`,
/// moved here unchanged.)
pub fn resolve_name(translations: &Value, base: &str, locale: &str) -> String {
    resolve_with(
        |k| translations.get(k).and_then(Value::as_str),
        base,
        locale,
    )
}

/// [`resolve_name`] over any lookup of a translation by locale key.
fn resolve_with<'a>(get: impl Fn(&str) -> Option<&'a str>, base: &str, locale: &str) -> String {
    let lang = locale.split(['-', '_']).next().unwrap_or(locale);
    for key in [locale, lang, "en"] {
        if let Some(s) = get(key) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    base.to_string()
}

/// A name's translations by locale key, as the API sends them
/// (`name_translations`). A missing or `null` object is none; a value that is
/// not text is dropped (it is no translation). Its own type rather than a
/// JSON value, so the browser build does not carry a JSON parser for it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Translations(pub BTreeMap<String, String>);

impl Translations {
    /// The text translations of a JSON value (what a server row holds).
    pub fn from_value(v: &Value) -> Self {
        Self(
            v.as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect(),
        )
    }

    /// The name a reader in `locale` sees ([`resolve_name`]'s rule).
    pub fn resolve(&self, base: &str, locale: &str) -> String {
        resolve_with(|k| self.0.get(k).map(String::as_str), base, locale)
    }
}

impl<'de> Deserialize<'de> for Translations {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = Translations;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of translations, or null")
            }
            fn visit_unit<E>(self) -> Result<Translations, E> {
                Ok(Translations::default())
            }
            fn visit_none<E>(self) -> Result<Translations, E> {
                Ok(Translations::default())
            }
            fn visit_some<D: serde::Deserializer<'de>>(
                self,
                d: D,
            ) -> Result<Translations, D::Error> {
                d.deserialize_any(self)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut m: A,
            ) -> Result<Translations, A::Error> {
                let mut out = BTreeMap::new();
                while let Some(k) = m.next_key::<String>()? {
                    if let Text(Some(v)) = m.next_value::<Text>()? {
                        out.insert(k, v);
                    }
                }
                Ok(Translations(out))
            }
        }
        d.deserialize_option(Visit)
    }
}

/// One translation's value: text, or anything else (dropped).
struct Text(Option<String>);

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = Text;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any value")
            }
            fn visit_str<E>(self, s: &str) -> Result<Text, E> {
                Ok(Text(Some(s.to_string())))
            }
            fn visit_string<E>(self, s: String) -> Result<Text, E> {
                Ok(Text(Some(s)))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_i64<E>(self, _: i64) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_u64<E>(self, _: u64) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_f64<E>(self, _: f64) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_unit<E>(self) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_none<E>(self) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Text, D::Error> {
                d.deserialize_any(self)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut s: A) -> Result<Text, A::Error> {
                while s.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(Text(None))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Text, A::Error> {
                while m
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}
                Ok(Text(None))
            }
        }
        d.deserialize_any(Visit)
    }
}

/// A category as the API sends it (`/categories`, the public menu): the
/// fields the rule reads. Other fields are ignored.
/// (In TypeScript: `OrderCategory`.)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "tsify",
    derive(tsify::Tsify),
    tsify(rename = "OrderCategory")
)]
pub struct Category {
    pub id: String,
    pub name: String,
    /// `{ "ar": "…", … }`; missing or `null` is none.
    #[serde(default)]
    #[cfg_attr(
        feature = "tsify",
        tsify(type = "Record<string, unknown> | null", optional)
    )]
    pub name_translations: Translations,
    /// Missing or `null` is 0.
    #[serde(default)]
    #[cfg_attr(feature = "tsify", tsify(optional))]
    pub display_order: Option<i32>,
}

/// The positions of `categories` in the order a reader in `locale` sees
/// them: `category_order(cs, l)[0]` is the index of the first one shown.
pub fn category_order(categories: &[Category], locale: &str) -> Vec<usize> {
    let names: Vec<String> = categories
        .iter()
        .map(|c| c.name_translations.resolve(&c.name, locale))
        .collect();
    let key = |i: usize| Key {
        display_order: categories[i].display_order.unwrap_or(0),
        name: &names[i],
        id: &categories[i].id,
    };
    let mut order: Vec<usize> = (0..categories.len()).collect();
    order.sort_by(|&a, &b| cmp(&key(a), &key(b)));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        resolve: Vec<ResolveCase>,
        categories: Vec<OrderCase>,
    }

    #[derive(Deserialize)]
    struct ResolveCase {
        name: String,
        #[serde(default)]
        translations: Value,
        base: String,
        locale: String,
        expected: String,
    }

    #[derive(Deserialize)]
    struct OrderCase {
        name: String,
        locale: String,
        categories: Vec<Category>,
        /// The ids, in the order shown.
        expected: Vec<String>,
    }

    fn vectors() -> Vectors {
        serde_json::from_str(crate::vectors::MENU_ORDER).expect("menu_order_vectors.json parses")
    }

    #[test]
    fn every_resolve_case() {
        let v = vectors();
        assert!(v.resolve.len() >= 6);
        for c in &v.resolve {
            assert_eq!(
                resolve_name(&c.translations, &c.base, &c.locale),
                c.expected,
                "{}",
                c.name
            );
        }
    }

    #[test]
    fn every_order_case() {
        let v = vectors();
        assert!(v.categories.len() >= 10);
        for c in &v.categories {
            let got: Vec<&str> = category_order(&c.categories, &c.locale)
                .into_iter()
                .map(|i| c.categories[i].id.as_str())
                .collect();
            assert_eq!(got, c.expected, "{}", c.name);
        }
    }

    #[test]
    fn sort_over_resolved_views_agrees_with_category_order() {
        // A reader holding localized views (the POS core) sorts them with
        // `sort`; the answer is `category_order`'s.
        for c in &vectors().categories {
            let mut views: Vec<(i32, String, String)> = c
                .categories
                .iter()
                .map(|k| {
                    (
                        k.display_order.unwrap_or(0),
                        k.name_translations.resolve(&k.name, &c.locale),
                        k.id.clone(),
                    )
                })
                .collect();
            sort(&mut views, |v| Key {
                display_order: v.0,
                name: &v.1,
                id: &v.2,
            });
            let got: Vec<&str> = views.iter().map(|v| v.2.as_str()).collect();
            assert_eq!(got, c.expected, "{}", c.name);
        }
    }

    #[test]
    fn the_order_does_not_depend_on_the_input_order() {
        for c in &vectors().categories {
            let mut reversed = c.categories.clone();
            reversed.reverse();
            let got: Vec<&str> = category_order(&reversed, &c.locale)
                .into_iter()
                .map(|i| reversed[i].id.as_str())
                .collect();
            assert_eq!(got, c.expected, "{} (reversed)", c.name);
        }
    }

    #[test]
    fn every_resolve_case_through_the_wire_type() {
        // The categories' own translations (what the browser and the server
        // pass to `category_order`) resolve as the JSON value does.
        for c in &vectors().resolve {
            let t: Translations = serde_json::from_value(c.translations.clone()).unwrap();
            assert_eq!(t, Translations::from_value(&c.translations), "{}", c.name);
            assert_eq!(t.resolve(&c.base, &c.locale), c.expected, "{}", c.name);
        }
    }

    #[test]
    fn translations_drop_what_is_not_text() {
        let t: Translations = serde_json::from_str(
            r#"{"ar":"قهوة","n":1,"b":true,"x":null,"o":{"a":"b"},"l":["a"],"f":1.5}"#,
        )
        .unwrap();
        assert_eq!(t.0.len(), 1);
        let c: Category = serde_json::from_str(r#"{"id":"a","name":"A"}"#).unwrap();
        assert_eq!(
            (c.name_translations, c.display_order),
            (Translations::default(), None)
        );
    }

    #[test]
    fn an_empty_list_is_empty() {
        assert!(category_order(&[], "en").is_empty());
    }
}
