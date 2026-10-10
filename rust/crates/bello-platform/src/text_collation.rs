//! Line collation for Text Tools' Sort A→Z / Z→A. Swift `LineTool` sorts with
//! `localizedCaseInsensitiveCompare`; on macOS this calls the same Foundation
//! method, so ordering follows the user's locale exactly. Elsewhere it declines
//! and the caller falls back to the portable approximation in bellobox-core.

/// Sorts `rows` in place (stable, like Swift's sort) and returns true, or
/// returns false without touching `rows` when no native collation exists.
#[cfg(target_os = "macos")]
pub fn localized_caseless_sort(rows: &mut Vec<String>, descending: bool) -> bool {
    use objc2_foundation::NSString;
    use std::cmp::Ordering;
    // Convert each line once; the comparison runs O(n log n) times.
    let strings: Vec<_> = rows.iter().map(|row| NSString::from_str(row)).collect();
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| {
        let ordering: Ordering = strings[a]
            .localizedCaseInsensitiveCompare(&strings[b])
            .into();
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    let mut taken: Vec<Option<String>> = rows.drain(..).map(Some).collect();
    rows.extend(order.into_iter().filter_map(|index| taken[index].take()));
    true
}

#[cfg(not(target_os = "macos"))]
pub fn localized_caseless_sort(_rows: &mut Vec<String>, _descending: bool) -> bool {
    false
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::localized_caseless_sort;

    fn sorted(input: &[&str], descending: bool) -> Vec<String> {
        let mut rows: Vec<String> = input.iter().map(|s| (*s).to_owned()).collect();
        assert!(localized_caseless_sort(&mut rows, descending));
        rows
    }

    #[test]
    fn foundation_order_ignores_case_and_is_stable() {
        // Byte order would put "Zebra" and "B" first; Foundation ignores case.
        let ascending = sorted(&["b", "B", "Zebra", "apple", "a", "A"], false);
        assert_eq!(ascending, ["a", "A", "apple", "b", "B", "Zebra"]);
        let descending = sorted(&["b", "B", "Zebra", "apple", "a", "A"], true);
        assert_eq!(descending, ["Zebra", "b", "B", "apple", "a", "A"]);
    }

    #[test]
    fn empty_and_multibyte_rows_survive_unchanged() {
        let rows = sorted(&["", "日本語", "emoji 😀", "é", "e\u{301}"], false);
        let mut expected: Vec<String> = ["", "日本語", "emoji 😀", "é", "e\u{301}"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let mut got = rows.clone();
        expected.sort();
        got.sort();
        assert_eq!(got, expected, "sorting must only permute rows");
        assert_eq!(rows[0], "", "an empty line sorts first");
    }
}
