//! The raw-byte shape rules the `<<` and `<<<` operators share between the two
//! levels: the byte scan (`scan::heredoc::operator_delims`) and the token
//! parser (`parse::heredoc::is_operator`, `parse::here_string`). The io-number
//! form `[N]<<word` is written *inside* one word (`cat 3<<T`), so only a
//! raw-byte rule sees it: a token-text version of the same rule (`3"<<"T` ≡
//! `3<<T` after quote removal) recreates the byte/token count mismatch
//! (LESSONS), and a shifted body region is the symptom (tasks #138/#238).
//! The byte scan reads only `<<` (`is_chevron` excludes a `<<<`); the
//! here-string's own span rule (`string_span`) exists for the token parser, so
//! the two operators never share a chevron test.

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
/// range failure (`io_fd`'s `None`).
pub(super) fn io_prefix<'a>(
    line: &'a [u8],
    op_start: usize,
    state: &ScanState,
) -> Option<(usize, &'a [u8])> {
    if !is_chevron(line, op_start) {
        return None;
    }
    let start = digit_run(line, op_start)?;
    if !word_start(line, start, state) {
        return None;
    }
    Some((start, line.get(start..op_start)?))
}

/// The decimal digit run that ends at `op`: the index its first digit sits at,
/// `None` when `op` is not preceded by a digit. Both count levels walk the
/// digits here, so a run glued to a longer word (`x3<<T`, `x3<<<T`) is rejected
/// identically at the byte scan and at the token parser.
fn digit_run(line: &[u8], op: usize) -> Option<usize> {
    let mut start = op;
    while start > 0 && line.get(start - 1).is_some_and(u8::is_ascii_digit) {
        start -= 1;
    }
    (start < op).then_some(start)
}

/// The here-string's raw span: `[io-number] <<<`, the digits beginning the
/// span — `span_operator`'s rule for the `<<<` operator, and the token parser's
/// only reading of a here-string (its `prefix` is the shift the word and its
/// mask are sliced by). Purely syntactic like `span_operator`: `999…<<<T` is a
/// span, and only the parse site reports the range failure (`io_fd`'s `None`).
pub(crate) fn string_span(line: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
    let at = line.get(start..end)?.iter().position(|&b| b == b'<')?;
    let op = start + at;
    if line.get(op..op + 3) != Some(&b"<<<"[..]) {
        return None;
    }
    match digit_run(line, op) {
        Some(run) if run == start => Some((op, op - run)),
        None if at == 0 => Some((op, 0)),
        _ => None,
    }
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

/// The io-number target descriptor for `N>file`, `N<file`, `N<<word` and
/// `N<<<word`: the very rule `N>file` uses (`parse::redirect::parse_fd`), so
/// one decimal rule decides all four operators and the fd-number semantics
/// cannot drift between them. `None` when the run does not fit `i32`: the byte
/// scan has no error channel and counts the operator either way; the parse site
/// reports the failure.
pub(crate) fn io_fd(digits: &[u8]) -> Option<i32> {
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
