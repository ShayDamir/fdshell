/// Skip from `i` past a `#` comment to the next `\n` (or end of input).
/// Returns the index to resume scanning from.
pub(crate) fn skip_comment(line: &[u8], mut i: usize) -> usize {
    while i < line.len() {
        if line.get(i) == Some(&b'\n') {
            i += 1;
            break;
        }
        i += 1;
    }
    i
}

/// The `\n` that ends the `#` comment starting at `i`, or `line.len()` when the
/// comment runs to end of input. The sibling of [`skip_comment`], which lands
/// **past** the newline: a caller whose own arms are keyed on that byte — the
/// cond-boundary scan's heredoc-region arm and run-reset arm, whose run must
/// keep the newline that ends it (task #296) — takes the newline itself and
/// lets those arms fire. Always advances by at least one, so a caller at a real
/// `#` byte cannot spin.
pub(crate) fn comment_end(line: &[u8], mut i: usize) -> usize {
    while i < line.len() && line.get(i) != Some(&b'\n') {
        i += 1;
    }
    i
}
