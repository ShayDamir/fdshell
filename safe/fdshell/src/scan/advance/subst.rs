//! The quoted-region skip used by `ScanState::advance` (task #130).

use crate::paren_scan::scan_at;
use crate::region_scan::{scan_backtick_at, scan_brace_at};

/// Consume a `$( … )`, `${ … }` or `` ` … ` `` region whole, starting at `i`
/// (the region's first byte), while the scanner is inside double quotes: the
/// region is data of the enclosing quoted word, so its `;`, `|`, `&&`,
/// `<`/`>`, `<<`, `"` and `)` are never top-level separators. Returns the index
/// to continue scanning at, or `None` when the region does not close (the
/// caller falls back to byte-at-a-time scanning, which is the existing
/// behavior for unbalanced text) — and for a byte that opens no region.
pub(super) fn skip_quoted_region(line: &[u8], i: usize) -> Option<usize> {
    match line.get(i).copied()? {
        b'$' => match line.get(i + 1).copied()? {
            b'(' => scan_at(line, i + 2, 1).map(|(_, at)| at),
            b'{' => scan_brace_at(line, i),
            _ => None,
        },
        b'`' => scan_backtick_at(line, i),
        _ => None,
    }
}
