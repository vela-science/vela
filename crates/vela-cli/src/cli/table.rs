//! A width-aware text table — ~60 lines, no dep (the `progress.rs` precedent).
//!
//! Cells are plain strings; column widths are computed from content so
//! columns align. On a terminal, if the table would overflow the width,
//! the widest column is truncated with `…`; piped output is never
//! truncated, so a script reading the columns gets byte-stable full-width
//! rows. Color, where a column needs it, is applied by the caller to the
//! already-padded cell — pad first, then color, because ANSI inside a
//! `{:<width$}` format breaks alignment (the rule discovered by hand in
//! the sign summary).

pub(crate) struct Table {
    rows: Vec<Vec<String>>,
}

const GAP: &str = "  "; // two spaces between columns, house style
const INDENT: &str = "  "; // leading indent, house style
const MIN_COL: usize = 8; // never truncate a column below this

impl Table {
    pub(crate) fn new() -> Self {
        Table { rows: Vec::new() }
    }

    pub(crate) fn row<I: IntoIterator<Item = String>>(&mut self, cells: I) {
        self.rows.push(cells.into_iter().collect());
    }

    /// Render using the real terminal width when stdout is a TTY, else
    /// unbounded (piped output stays full-width).
    pub(crate) fn render(&self) -> String {
        use std::io::IsTerminal;
        let max = if std::io::stdout().is_terminal() {
            Some(
                std::env::var("COLUMNS")
                    .ok()
                    .and_then(|c| c.parse::<usize>().ok())
                    .unwrap_or(100),
            )
        } else {
            None
        };
        self.render_within(max)
    }

    /// The testable core: `max_width = None` never truncates.
    pub(crate) fn render_within(&self, max_width: Option<usize>) -> String {
        if self.rows.is_empty() {
            return String::new();
        }
        let ncols = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0usize; ncols];
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }

        // If a max width is set and the natural table overflows, shrink the
        // widest column (the last one is never padded, so it can't overflow
        // — trim the widest of the padded columns).
        if let Some(max) = max_width {
            let gaps = INDENT.chars().count() + GAP.chars().count() * ncols.saturating_sub(1);
            let natural: usize = widths.iter().sum::<usize>() + gaps;
            if natural > max && ncols > 0 {
                let over = natural - max;
                if let Some((widest_i, _)) = widths[..ncols.saturating_sub(1).max(1)]
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, w)| **w)
                {
                    widths[widest_i] = widths[widest_i].saturating_sub(over).max(MIN_COL);
                }
            }
        }

        let mut out = String::new();
        for row in &self.rows {
            out.push_str(INDENT);
            for (i, cell) in row.iter().enumerate() {
                let w = widths[i];
                let shown = if cell.chars().count() > w {
                    let keep = w.saturating_sub(1);
                    format!("{}…", cell.chars().take(keep).collect::<String>())
                } else {
                    cell.clone()
                };
                // Last column is not padded (nothing follows it to align to).
                if i + 1 == row.len() {
                    out.push_str(&shown);
                } else {
                    out.push_str(&format!("{shown:<w$}"));
                    out.push_str(GAP);
                }
            }
            out.push('\n');
        }
        out.pop(); // trailing newline
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn columns_align_to_the_widest_cell() {
        let mut t = Table::new();
        t.row(cells(&["a", "one"]));
        t.row(cells(&["bbbb", "two"]));
        let out = t.render_within(None);
        let lines: Vec<&str> = out.lines().collect();
        // Column 0 padded to width 4 ("bbbb"); "a" → "a   ".
        assert_eq!(lines[0], "  a     one");
        assert_eq!(lines[1], "  bbbb  two");
    }

    #[test]
    fn piped_is_never_truncated() {
        let mut t = Table::new();
        t.row(cells(&["a-very-long-key-that-would-overflow", "v"]));
        // max_width None models piped output: full width, no ellipsis.
        assert!(!t.render_within(None).contains('…'));
    }

    #[test]
    fn narrow_terminal_truncates_the_widest_column() {
        let mut t = Table::new();
        t.row(cells(&["a-very-long-first-column-value", "short"]));
        let out = t.render_within(Some(20));
        assert!(out.contains('…'), "should truncate to fit 20 cols: {out:?}");
        // The trailing column survives intact.
        assert!(out.contains("short"));
    }
}
