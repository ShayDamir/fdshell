//! The truncation guard of `param_op/word.rs`: the first-`}` reader (task #174)
//! can cut a word in the middle of a `$( … )` or a backtick span, and expanding
//! that fragment would turn today's accepted literal output into an
//! `UnclosedParen` / `UnclosedBacktick` bail, so the guard keeps it literal.
//! LESSONS: "An operator word is a word"

// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

use crate::paren_scan::scan_at;

/// `true` when the word text holds an unclosed `$( … )` or `` ` … ` `` span: the
/// reader cut the word at the first `}`, so what looks like a substitution is a
/// fragment and today's literal output must stand. Conservative by design — a
/// `$(`-lookalike counts as unclosed — and it skips `\X` pairs, because the
/// tokenizer writes a single-quoted `$` as `\$`.
pub(super) fn truncated(bytes: &[u8]) -> bool {
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        i = match b {
            b'\\' => i + 2,
            b'$' if bytes.get(i + 1) == Some(&b'(') => match scan_at(bytes, i + 2, 1) {
                Some((_, end)) => end,
                None => return true,
            },
            b'`' => match closed_tick(bytes, i + 1) {
                Some(end) => end,
                None => return true,
            },
            _ => i + 1,
        };
    }
    false
}

/// The index just past the backtick that closes the one at `start`, or `None`
/// when the word text has none (the span was cut). `\X` pairs are skipped, as
/// every other byte scanner in this layer skips them.
fn closed_tick(bytes: &[u8], mut i: usize) -> Option<usize> {
    while let Some(&b) = bytes.get(i) {
        if b == b'\\' {
            i += 2;
            continue;
        }
        if b == b'`' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}
