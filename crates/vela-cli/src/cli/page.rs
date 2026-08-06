//! The one page-and-cursor rule for the list verbs.
//!
//! `review list` grew its cursor loop inline. `claims` needs the same one over
//! a different row type, and two copies of a cursor rule are two chances for a
//! page boundary to drift apart. The rule lives here once and both verbs bind
//! to it.
//!
//! The cursor is a row's own stable identity, never an offset into the list: a
//! caller resuming from `next_cursor` skips exactly the rows it already saw.
//! A cursor naming no row is refused rather than silently restarted from the
//! top, because a silent restart returns page one to a caller that believes it
//! is reading page nine.

/// The largest page any list verb returns, whatever `--limit` asks for.
pub(crate) const MAX_LIMIT: usize = 100;

pub(crate) struct Page<T> {
    /// Rows matching the caller's filter, counted before the page was cut.
    pub(crate) total: usize,
    pub(crate) items: Vec<T>,
    /// The last returned row's identity, and only when rows follow it. A
    /// cursor on the final page would invite one more round trip to learn
    /// there is nothing left.
    pub(crate) next_cursor: Option<String>,
}

/// Cut one page out of an already-ordered, already-filtered row list.
///
/// `key` names the row's stable identity — the value a caller passes back as
/// `--cursor`. It must be unique within `items` and must not change between
/// calls; `verb` and `subject` only shape the refusal an unknown cursor gets
/// (`"review cursor does not name an exact current Proposal"`).
pub(crate) fn paginate<T>(
    verb: &str,
    subject: &str,
    items: Vec<T>,
    limit: usize,
    cursor: Option<&str>,
    key: impl Fn(&T) -> Option<&str>,
) -> Page<T> {
    let total = items.len();
    let limit = limit.clamp(1, MAX_LIMIT);
    let start = match cursor {
        None => 0,
        Some(cursor) => items
            .iter()
            .position(|item| key(item) == Some(cursor))
            .map(|index| index + 1)
            .unwrap_or_else(|| {
                crate::cli::fail_return(&format!(
                    "{verb} cursor does not name an exact current {subject}"
                ))
            }),
    };
    /* One row past the page is read so `next_cursor` states a fact rather
    than a guess: it is present exactly when a further row exists. */
    let mut items = items
        .into_iter()
        .skip(start)
        .take(limit + 1)
        .collect::<Vec<_>>();
    let has_more = items.len() > limit;
    items.truncate(limit);
    let next_cursor = has_more
        .then(|| items.last().and_then(&key).map(str::to_string))
        .flatten();
    Page {
        total,
        items,
        next_cursor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("row{index:02}")).collect()
    }

    fn page(items: Vec<String>, limit: usize, cursor: Option<&str>) -> Page<String> {
        paginate("test", "Row", items, limit, cursor, |row| {
            Some(row.as_str())
        })
    }

    #[test]
    fn total_counts_the_filtered_set_not_the_page() {
        let page = page(rows(7), 3, None);
        assert_eq!(page.total, 7);
        assert_eq!(page.items.len(), 3);
    }

    #[test]
    fn the_cursor_resumes_after_the_row_it_names() {
        let first = page(rows(5), 2, None);
        assert_eq!(first.items, ["row00", "row01"]);
        assert_eq!(first.next_cursor.as_deref(), Some("row01"));
        let second = page(rows(5), 2, first.next_cursor.as_deref());
        assert_eq!(second.items, ["row02", "row03"]);
        let third = page(rows(5), 2, second.next_cursor.as_deref());
        assert_eq!(third.items, ["row04"]);
        assert_eq!(third.next_cursor, None);
    }

    /// A page that exactly consumes the remainder is the last page. Emitting a
    /// cursor here would cost a caller one round trip to learn nothing follows.
    #[test]
    fn an_exactly_filled_final_page_carries_no_cursor() {
        let page = page(rows(4), 2, Some("row01"));
        assert_eq!(page.items, ["row02", "row03"]);
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn limit_is_clamped_at_both_ends() {
        assert_eq!(page(rows(200), 0, None).items.len(), 1);
        assert_eq!(page(rows(200), 5000, None).items.len(), MAX_LIMIT);
    }

    #[test]
    fn an_empty_set_pages_to_nothing() {
        let page = page(Vec::new(), 10, None);
        assert_eq!(page.total, 0);
        assert!(page.items.is_empty());
        assert_eq!(page.next_cursor, None);
    }
}
