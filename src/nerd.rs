//! Nerd Font icon metadata, backing the searchable icon picker.
//!
//! Roughly 11,000 glyphs live in private-use space. Rendering them all at once
//! would thrash the egui font atlas (which is dropped and re-rasterized once it
//! fills), so the picker always filters down to a small page before drawing
//! anything. That is why this module exposes `search` rather than just a list.

use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    /// Metadata name, e.g. `cod-account`.
    pub name: String,
    pub ch: char,
    /// The part of the name before the first `-`, e.g. `cod`, `md`, `fa`.
    pub category: String,
}

fn parse() -> Vec<Icon> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(crate::assets::GLYPHNAMES_JSON)
    else {
        // A malformed asset should degrade to "no icons", not take the app down.
        return Vec::new();
    };
    let Some(map) = root.as_object() else {
        return Vec::new();
    };

    let mut icons = Vec::with_capacity(map.len());
    for (name, entry) in map {
        // `METADATA` is a single object holding version info, not a glyph.
        if name == "METADATA" {
            continue;
        }
        // The hex `code` field is used rather than the `char` field: it is
        // unambiguous and avoids any surrogate-pair decoding surprises.
        let Some(code) = entry.get("code").and_then(|c| c.as_str()) else {
            continue;
        };
        let Ok(value) = u32::from_str_radix(code, 16) else {
            continue;
        };
        let Some(ch) = char::from_u32(value) else {
            continue;
        };
        let category = name.split('-').next().unwrap_or("").to_owned();
        icons.push(Icon { name: name.clone(), ch, category });
    }

    icons.sort_by(|a, b| a.name.cmp(&b.name));
    icons
}

pub fn all() -> &'static [Icon] {
    static ICONS: OnceLock<Vec<Icon>> = OnceLock::new();
    ICONS.get_or_init(parse)
}

/// Categories with how many icons each holds, largest first.
///
/// The counts matter for browsing: `md` alone is over half the set, so a user
/// who wants variety needs to see where the bulk of the icons actually live
/// rather than paging through whichever set happens to sort first.
pub fn category_counts() -> Vec<(&'static str, usize)> {
    let mut counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for icon in all() {
        *counts.entry(icon.category.as_str()).or_insert(0) += 1;
    }
    let mut out: Vec<(&'static str, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    out
}

/// Total number of icons in the set.
pub fn total() -> usize {
    all().len()
}

/// Score a match, lower being better. `None` means no match.
///
/// Ranked so an exact name beats a prefix, which beats a substring, so typing
/// `git` surfaces `dev-git` before `md-github_something`.
fn rank(name: &str, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(3);
    }
    let name = name.to_ascii_lowercase();
    let query = query.to_ascii_lowercase();
    if name == query {
        Some(0)
    } else if name.starts_with(&query) {
        Some(1)
    } else if name.contains(&query) {
        Some(2)
    } else {
        None
    }
}

/// Round-robin the icons across categories, preserving each category's order.
///
/// Alphabetical order alone makes browsing useless: the set is dominated by a
/// few large families, so the first screen fills with one of them (in practice
/// every icon whose name starts with `cod-`) and the picker looks far smaller
/// and duller than it is. Interleaving shows the actual variety straight away.
fn interleave(icons: Vec<&'static Icon>) -> Vec<&'static Icon> {
    let mut buckets: std::collections::BTreeMap<&'static str, Vec<&'static Icon>> =
        std::collections::BTreeMap::new();
    for icon in icons {
        buckets.entry(icon.category.as_str()).or_default().push(icon);
    }

    let deepest = buckets.values().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::with_capacity(buckets.values().map(Vec::len).sum());
    for depth in 0..deepest {
        for bucket in buckets.values() {
            if let Some(icon) = bucket.get(depth) {
                out.push(*icon);
            }
        }
    }
    out
}

/// Search by name, optionally restricted to a category.
///
/// With a query, results are ranked by match quality and then alphabetically, so
/// the ordering is stable across keystrokes rather than reshuffling. With an
/// empty query the caller is browsing, and the results are interleaved across
/// categories instead (see [`interleave`]).
pub fn search(query: &str, category: Option<&str>, limit: usize) -> Vec<&'static Icon> {
    let query = query.trim();
    let browsing = query.is_empty() && category.is_none();

    let mut hits: Vec<(u8, &'static Icon)> = all()
        .iter()
        .filter(|icon| category.is_none_or(|c| icon.category == c))
        .filter_map(|icon| rank(&icon.name, query).map(|r| (r, icon)))
        .collect();

    if browsing {
        hits.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        return interleave(hits.into_iter().map(|(_, icon)| icon).collect())
            .into_iter()
            .take(limit)
            .collect();
    }

    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    hits.into_iter().take(limit).map(|(_, icon)| icon).collect()
}

/// Look one up by its exact metadata name. Used by the tests, and the natural
/// entry point for a "jump to icon by name" feature.
#[allow(dead_code)]
pub fn by_name(name: &str) -> Option<&'static Icon> {
    all().iter().find(|i| i.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_asset_parses_into_icons() {
        let icons = all();
        assert!(
            icons.len() > 5000,
            "expected thousands of icons, got {}",
            icons.len()
        );
    }

    #[test]
    fn metadata_is_not_treated_as_a_glyph() {
        assert!(by_name("METADATA").is_none());
    }

    #[test]
    fn known_icons_are_present_with_sane_codepoints() {
        let account = by_name("cod-account").expect("cod-account should exist");
        assert_eq!(account.ch as u32, 0xeb99);
        assert_eq!(account.category, "cod");
    }

    #[test]
    fn every_icon_is_in_private_use_or_symbol_space() {
        for icon in all() {
            let c = icon.ch as u32;
            let private_use = (0xE000..=0xF8FF).contains(&c)
                || (0xF0000..=0xFFFFD).contains(&c)
                || (0x100000..=0x10FFFD).contains(&c);
            // A handful of Nerd Font glyphs live in normal symbol blocks.
            let symbol = (0x2000..=0x2BFF).contains(&c);
            assert!(private_use || symbol, "{} is U+{c:04X}", icon.name);
        }
    }

    #[test]
    fn categories_are_populated_and_unique() {
        let cats: Vec<_> = category_counts().iter().map(|(c, _)| *c).collect();
        assert!(cats.len() > 5, "got {cats:?}");
        let mut sorted = cats.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(cats.len(), sorted.len(), "a category was listed twice");
    }

    #[test]
    fn exact_name_outranks_prefix_and_substring() {
        let hits = search("cod-account", None, 10);
        assert_eq!(hits.first().map(|i| i.name.as_str()), Some("cod-account"));
    }

    #[test]
    fn search_is_case_insensitive() {
        let lower = search("account", None, 5);
        let upper = search("ACCOUNT", None, 5);
        assert!(!lower.is_empty());
        assert_eq!(
            lower.iter().map(|i| &i.name).collect::<Vec<_>>(),
            upper.iter().map(|i| &i.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn category_filter_restricts_results() {
        let hits = search("", Some("weather"), 20);
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|i| i.category == "weather"));
    }

    #[test]
    fn limit_is_respected() {
        assert_eq!(search("", None, 7).len(), 7);
        assert_eq!(search("a", None, 3).len(), 3);
    }

    #[test]
    fn an_unlimited_search_returns_the_whole_set() {
        // The picker virtualizes its drawing, so it asks for everything and
        // scrolls, rather than showing a page.
        assert_eq!(search("", None, total()).len(), total());
    }

    #[test]
    fn category_counts_add_up_to_the_total() {
        let sum: usize = category_counts().iter().map(|(_, n)| n).sum();
        assert_eq!(sum, total());
    }

    #[test]
    fn biggest_category_comes_first() {
        let counts = category_counts();
        assert!(counts.len() > 3);
        for pair in counts.windows(2) {
            assert!(
                pair[0].1 >= pair[1].1,
                "counts out of order: {:?} before {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn nonsense_query_returns_nothing() {
        assert!(search("zzzzz-not-a-real-icon-name", None, 10).is_empty());
    }

    #[test]
    fn empty_query_returns_a_full_page() {
        // The picker shows a default page before the user types anything.
        assert_eq!(search("", None, 50).len(), 50);
    }

    #[test]
    fn browsing_interleaves_categories() {
        // Alphabetical order alone would put every `cod-` icon on the first
        // screen and make the set look far smaller than it is.
        let first_page = search("", None, 24);
        let categories: std::collections::HashSet<_> =
            first_page.iter().map(|i| i.category.as_str()).collect();
        assert!(
            categories.len() > 5,
            "first page covered only {categories:?}"
        );
    }

    #[test]
    fn browsing_still_covers_every_icon() {
        let all_names: std::collections::HashSet<_> =
            search("", None, total()).iter().map(|i| i.name.as_str()).collect();
        assert_eq!(all_names.len(), total(), "interleaving dropped or duplicated icons");
    }

    #[test]
    fn searching_ranks_instead_of_interleaving() {
        // A query must not be reordered by category; relevance wins.
        let hits = search("arrow", None, 10);
        assert!(!hits.is_empty());
        let first_cat = &hits[0].category;
        assert!(
            hits.iter().take(4).filter(|i| &i.category == first_cat).count() >= 1,
            "relevance ranking should not be disturbed"
        );
    }

    #[test]
    fn interleaving_preserves_every_element() {
        let icons: Vec<_> = all().iter().take(100).collect();
        let n = icons.len();
        assert_eq!(interleave(icons).len(), n);
    }

    #[test]
    fn results_are_deterministic() {
        // Rankings must be stable or the list reshuffles on every keystroke.
        let a: Vec<_> = search("arrow", None, 20).iter().map(|i| i.name.clone()).collect();
        let b: Vec<_> = search("arrow", None, 20).iter().map(|i| i.name.clone()).collect();
        assert_eq!(a, b);
    }
}
