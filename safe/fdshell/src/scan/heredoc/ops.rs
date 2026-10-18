//! The byte-level `<<` operator scan for a run, shared by the skip helpers
//! and the line-body extraction.

use crate::scan::ScanState;

use super::io::{io_fd, io_prefix, word_start};
use super::lines::delimiter_span;
use super::op::Operator;
use alloc::vec::Vec;

/// The `Operator` of every `<<` in the run, in order. `None` when a bare
/// `<<` (or `<<-`) has no delimiter word. A `#` comment (outside quotes, at
/// a word start) is skipped whole, so a `<<` in a comment is not an operator
/// (the tokenizer never yields it, and the two counts must agree).
///
/// A `<<` counts as an operator when a token precedes its word and that token
/// is not a pipeline `|` — the byte mirror of `parse::heredoc::is_operator`
/// (`i != 0 && prev != b"|"`). A `<<` at the run start is the command word
/// and never counts. The io-number form (`3<<T`, `0<<-T`) is read through the
/// shared `io::io_prefix`, so its digits begin the operator's word and the
/// operator carries their fd; `io::span_operator` is the token-level half of
/// that same rule (LESSONS: the byte-level and token-level counts must agree).
pub(crate) fn operator_delims(line: &[u8], from: usize, to: usize) -> Option<Vec<Operator>> {
    let mut state = ScanState::new();
    let mut found: Vec<Operator> = Vec::new();
    let mut seen_token = false;
    let mut i = from;
    while i < to {
        let bare =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
        if bare && !state.word_active && line.get(i) == Some(&b'#') {
            match line
                .get(i..)
                .and_then(|s| s.iter().position(|&b| b == b'\n'))
            {
                Some(p) => i = i + p + 1,
                None => return Some(found),
            }
            continue;
        }
        // The io-number form's word starts at its digits, so both the word
        // gate and the pipeline gate are evaluated at the operator's own word
        // start `run`: the operator's prefix is never its own preceding token
        // (`is_operator`'s `i != 0`), and a `|` written right before the
        // digits is a stage boundary (`prev != |`).
        let io = io_prefix(line, i, &state);
        let run = io.map_or(i, |(run, _)| run);
        let preceded = if run == i {
            seen_token && word_start(line, i, &state)
        } else {
            token_before(line, from, run)
        };
        if bare
            && preceded
            && line.get(i) == Some(&b'<')
            && line.get(i + 1) == Some(&b'<')
            && line.get(i + 2) != Some(&b'<')
            && !precedes_pipe(line, run)
        {
            let dash = line.get(i + 2) == Some(&b'-');
            let (next, raw) = delimiter_span(line, i + 2 + usize::from(dash), to)?;
            let fd = io.and_then(|(_, digits)| io_fd(digits)).unwrap_or(0);
            found.push(Operator::new(raw, dash, fd));
            while i < next {
                i = state.advance(line, i);
            }
            continue;
        }
        // Read the byte before `advance`: the `\<`, `$(`, `((` and quoted-region
        // arms jump 2+ bytes, so the post-jump byte is not the token's first byte.
        let byte = line.get(i).copied();
        i = state.advance(line, i);
        if byte.is_some_and(is_token_byte) {
            seen_token = true;
        }
    }
    Some(found)
}

/// The byte at a consumed position starts (or continues) a token: any
/// non-whitespace byte, except a pipeline `|`, which is a stage boundary with
/// no token of its own — `parse::heredoc::is_operator` rejects a `<<` whose
/// previous token is `|`, and a `|`-leading run has no preceding token at all
/// (`| <<EOF`). A `|` absorbed into a `>|` clobber is operator content of its
/// word, decided by `precedes_pipe`, never here. `;`/`&&` ARE tokens for the
/// tokenizer, so they count here: `cat <<A ; <<B` is two operators at both levels.
fn is_token_byte(b: u8) -> bool {
    !b.is_ascii_whitespace() && b != b'|'
}

/// A token byte sits in `line[from..run]`: the byte mirror of `is_operator`'s
/// `i != 0` for an operator whose word begins at `run` (its io-number digits
/// belong to that word, so they never count as the preceding token — a run
/// that opens with only whitespace has none, exactly as the whitespace-leading
/// `<<` rows of `is_token_byte` decide).
fn token_before(line: &[u8], from: usize, run: usize) -> bool {
    line.get(from..run)
        .unwrap_or_default()
        .iter()
        .any(|&b| is_token_byte(b))
}

/// A `|` with only whitespace between it and `i`: a pipeline position, where
/// `<<` is a command word, not an operator. A `|` absorbed into a `>|` clobber
/// operator is not a pipeline position: `parse::redirect::clobber_pipe` is the
/// ONE rule, the same `>`-terminated word test the tokenizer applies to the
/// word's raw bytes + mask (`parse/token_pipe.rs`), so the byte and token
/// counts cannot diverge (LESSONS: byte-level and token-level rules must
/// agree).
fn precedes_pipe(line: &[u8], i: usize) -> bool {
    let mut k = i;
    while k > 0 {
        k -= 1;
        match line.get(k) {
            Some(&b' ') | Some(b'\t') => continue,
            Some(b'|') => return !crate::parse::clobber_pipe(line, k),
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests;
