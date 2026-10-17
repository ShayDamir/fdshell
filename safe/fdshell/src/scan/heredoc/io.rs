//! The raw-byte shape rules a `<<` operator shares between the two levels:
//! the byte scan (`scan::heredoc::operator_delims`) and the token parser
//! (`parse::heredoc::is_operator`). The io-number form `[N]<<word` is written
//! *inside* one word (`cat 3<<T`), so only a raw-byte rule sees it: a token-text
//! version of the same rule (`3"<<"T` ≡ `3<<T` after quote removal) recreates
//! the byte/token count mismatch (LESSONS), and a shifted body region is the
//! symptom (tasks #138/#238).

use crate::parse::parse_fd;
use crate::scan::ScanState;
use sys::ShortCStr;

/// The io-number digit run in front of the `<<` at `op_start`, as
/// `(run_start, digits)`: the decimal run that begins the operator's word
/// (`3<<T`, `003<<-T`, `0<<T`). `None` when `op_start` is not a here-doc `<<`
/// (a `<<<` is the here-string), when the `<<` begins its word (the unprefixed
/// form), and when any other byte precedes the digits (`x<<T`, `x3<<T`,
/// `"3"<<T`, `\3<<T`, `>3<<T`). Purely syntactic: no fd-range test, so both
/// levels count `999…<<T` identically and only the parse site reports the
/// range failure (`heredoc_fd`'s `None`).
pub(super) fn io_prefix<'a>(
    line: &'a [u8],
    op_start: usize,
    state: &ScanState,
) -> Option<(usize, &'a [u8])> {
    if !is_chevron(line, op_start) {
        return None;
    }
    let mut start = op_start;
    while start > 0 && line.get(start - 1).is_some_and(u8::is_ascii_digit) {
        start -= 1;
    }
    if start == op_start || !word_start(line, start, state) {
        return None;
    }
    Some((start, line.get(start..op_start)?))
}

/// The io-number form inside a token's raw span: `(chevron_pos, prefix_len)`
/// for a span shaped `[io-number] << [not `<`]`, where the digits — when there
/// are any — begin the span. The token parser reads this span, never the
/// token text, so `3"<<"T`, `"3"<<T` and `x<<T` stay word data at both levels.
pub(crate) fn span_operator(line: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
    let at = line.get(start..end)?.iter().position(|&b| b == b'<')?;
    let op = start + at;
    if !is_chevron(line, op) {
        return None;
    }
    match io_prefix(line, op, &ScanState::new()) {
        Some((run, digits)) if run == start => Some((op, digits.len())),
        None if at == 0 => Some((op, 0)),
        _ => None,
    }
}

/// The here-doc target descriptor: the very rule `N>file` uses
/// (`parse::redirect::parse_fd`), so one decimal rule decides both families.
/// `None` when the run does not fit `i32`: the byte scan has no error channel
/// and counts the operator either way; the parse site reports the failure.
pub(crate) fn heredoc_fd(digits: &[u8]) -> Option<i32> {
    ShortCStr::from_vec(digits.to_vec())
        .ok()
        .and_then(|prefix| parse_fd(&prefix, b'<'))
}

/// A here-doc operator's first two bytes at `pos`: `<<`, never the `<<<` of a
/// here-string.
fn is_chevron(line: &[u8], pos: usize) -> bool {
    line.get(pos..pos + 2) == Some(&b"<<"[..]) && line.get(pos + 2) != Some(&b'<')
}

/// `i` starts a token word: the run start or a tokenizer word-break byte. A
/// `)` breaks the word only at top level — a `$( )`-closing `)` keeps the
/// token whole, exactly as the tokenizer does.
pub(super) fn word_start(line: &[u8], i: usize, state: &ScanState) -> bool {
    if i == 0 {
        return true;
    }
    match line.get(i - 1).copied() {
        Some(b')') => !state.word_active,
        // The `_` arm excludes `|`: a `|` byte never starts a word here, so a
        // `<<` right after `|` (absorbed into a `>|` operator or a pipeline
        // pipe) is never an operator; `precedes_pipe` decides the pipe.
        _ => matches!(
            line.get(i - 1).copied(),
            Some(b' ') | Some(b'\t') | Some(b';') | Some(b'\n')
        ),
    }
}

#[cfg(test)]
mod tests;
