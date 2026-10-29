//! The shared walk behind "the last token of the text so far is a top-level
//! `&&`/`||`" — the state-aware rule POSIX #2.9's AND-IF-TERMINATOR is built on
//! (task #492). Two callers read it at their own granularity: the segment
//! scanner's newline/comment arms (a line that ends on the operator does not end
//! the cond list) and `repl/trailing.rs` (the REPL keeps reading).

use crate::scan::{Boundary, ScanState, boundary, heredoc, skip_comment};

/// Walk `text` up to (never including) `at` and report the span of the last
/// top-level `&&`/`||` that is also the text's last **token**, plus the walk's
/// end state (the REPL's open-quote/backtick/`$( )` check reads it). Passing
/// `at == text.len() + 1` includes the end-of-input boundary, which is what the
/// REPL asks about; the segment scanner passes the newline or `#` it stands on,
/// so a region lying on the lines below is outside the walk.
///
/// Quotes, escapes, comments, `$( )`, backticks and heredoc regions are read by
/// the existing scan rules, never by a byte compare (LESSONS: "A separator-shaped
/// word needs the span gate"), and the glued pair (`false&&`) counts as an
/// operator token (task #496). Comment and region text are not tokens, so an
/// operator written *inside* one is never the answer, and any word token behind
/// the operator takes the answer away.
pub(crate) fn walk(text: &[u8], at: usize) -> (ScanState, Option<(usize, usize)>) {
    let mut state = ScanState::new();
    let mut i = 0usize;
    let mut run_start = 0usize;
    let mut last_op: Option<(usize, usize)> = None;
    while i < at {
        let kind = boundary(text, i, &state);
        // The same bare test `boundary` applies to a separator, restated here
        // because the walk needs it at its own arm: a `&&` inside a quote, a span
        // or an open `$( )` is word data, never an operator token.
        let bare =
            !(state.in_quote || state.in_single || state.in_backtick) && state.paren_depth == 0;
        if kind == Boundary::Char {
            let op = bare.then(|| top_level_op(text, i)).flatten();
            if let Some(len) = op {
                last_op = Some((i, len));
                for _ in 0..len {
                    i = state.advance(text, i);
                }
                continue;
            }
            if !text.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
                // An ordinary word token (a quote, an escape pair, a closer) sits
                // behind the operator: it is the text's last token now.
                last_op = None;
            }
            i = state.advance(text, i);
            continue;
        }
        state.word_active = false;
        if kind == Boundary::Comment {
            i = skip_comment(text, i);
            run_start = i;
            continue;
        }
        match heredoc::skip_region(text, run_start, i) {
            Some((_, resume, _)) => {
                i = resume;
                run_start = i;
            }
            None => {
                i += 1;
                run_start = i;
            }
        }
    }
    (state, last_op)
}

/// `true` when the last token of `text[..at]` is a top-level `&&`/`||`
/// operator: the physical-line end there is the operator's terminator, not a
/// statement separator, so the cond list continues onto the next physical line
/// (POSIX #2.9, task #492). Comment text is ignored — bash's lexer discards it —
/// so `false && #c` + newline + `echo b` joins.
pub(crate) fn ends_on_operator(text: &[u8], at: usize) -> bool {
    walk(text, at).1.is_some()
}

/// The length of the top-level cond-list operator at `i`: `&&` (2) or `||`
/// (2). A lone `&` is not a continuation: it is the plain async operator
/// (task #326) and a complete input the boundary loop already ended, so the
/// REPL executes it at once. A lone `|` is not a continuation either (the
/// pipeline parser cannot span a `|` across a newline, so a trailing `|`
/// stays a hard parse error).
fn top_level_op(text: &[u8], i: usize) -> Option<usize> {
    match text.get(i).copied() {
        Some(b'&') if text.get(i + 1) == Some(&b'&') => Some(2),
        Some(b'|') if text.get(i + 1) == Some(&b'|') => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
