/// The event-case `wait` pattern keywords: the words that open a block when
/// they follow `wait` on the same line.
pub(crate) const WAIT_PATTERN_KEYWORDS: &[&[u8]] =
    &[b"readable", b"writable", b"finished", b"after"];

pub(crate) fn is_wait_pattern_keyword(word: &[u8]) -> bool {
    WAIT_PATTERN_KEYWORDS.contains(&word)
}

/// The offset (relative to `off`) just past the first word of `part`.
pub(crate) fn first_word_end(part: &[u8], off: usize) -> usize {
    let len = part
        .iter()
        .position(|&b| b.is_ascii_whitespace() || b == b';')
        .unwrap_or(part.len());
    off + len
}

/// Whether a `wait` ending at `word_end` in `line` opens an event-case block.
///
/// The next word — skipping whitespace (incl. newlines) and whole-line
/// comments — opens a block if it is on a subsequent line, or if it is on the
/// same line and is a pattern keyword. A `;`, a quoted word, a same-line
/// pid/`$!`, or EOF is the POSIX `wait` builtin.
pub(crate) fn wait_opens_block(line: &[u8], word_end: usize) -> bool {
    let mut i = word_end;
    let mut crossed_newline = false;
    loop {
        let b = match line.get(i) {
            Some(&b) => b,
            None => return false,
        };
        if b == b'\n' {
            crossed_newline = true;
            i += 1;
            continue;
        }
        if b == b';' && !crossed_newline {
            return false;
        }
        if b == b'"' || b == b'\'' || b == b'`' {
            return false;
        }
        if b == b'#' {
            while let Some(&c) = line.get(i) {
                if c == b'\n' {
                    break;
                }
                i += 1;
            }
            continue;
        }
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while let Some(&b) = line.get(i) {
            if b.is_ascii_whitespace() || b == b';' {
                break;
            }
            i += 1;
        }
        let word = line.get(start..i).unwrap_or(b"");
        return crossed_newline || is_wait_pattern_keyword(word);
    }
}
