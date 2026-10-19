/// Check that `word` ends at a token boundary at position `len`.
fn boundary(word: &[u8], len: usize, extra: &[u8]) -> bool {
    word.get(len)
        .is_none_or(|&b| b.is_ascii_whitespace() || b == b';' || extra.contains(&b))
}

mod function;
mod pair;
mod position;
mod quoted;
mod wait;

pub(crate) use quoted::quoted_word;

pub(crate) use function::{definer_len, function_def_name, name_len};
pub(crate) use pair::pair_payload;
pub(crate) use position::{Glue, Place, after, scores};
pub(super) use wait::{first_word_end, is_wait_pattern_keyword, wait_opens_block};

use crate::segment::group::{closes, opens};

/// Return `Some(1)` for keywords that open a block, `Some(-1)` for closers.
///
/// Recognized: `case`, `esac`, `if`, `fi`, `for`, `wait`, `while`, `until`,
/// `done`, `{`, `}`.
/// Each check requires a word boundary after the keyword to avoid
/// matching prefixes like `ifconfig` or `donec`. `wait` opens a block only
/// when [`wait_opens_block`] holds (otherwise it is the POSIX builtin, `0`).
/// `{`/`}` are the group command's delimiters and use the predicates the
/// segment scanner uses for its group opener, so the depth rule and the
/// segment opener cannot drift.
/// A word that contains quote bytes scores `None`: a quoted keyword word is
/// data ([`quoted_word`]), as bash requires — the same rule the segment
/// scanner's `frag_quoted` test applies to a word run.
/// A `time [-p]` word is a *prefix*, not a keyword of its own: it opens no
/// block, so it never enters the depth table. A block keyword behind it
/// (`time while … done`) is scored on the rest of the word, which keeps the
/// whole statement one segment for the `time` parse hook.
pub(super) fn keyword_delta(word: &[u8], line: &[u8], word_end: usize) -> Option<i32> {
    if quoted_word(line, word_end) {
        return None;
    }
    if let Some(p) = time_prefix_len(word) {
        return keyword_delta(word.get(p..).unwrap_or(b""), line, word_end);
    }
    if word.starts_with(b"case") && boundary(word, 4, b"") {
        return Some(1);
    }
    if word.starts_with(b"esac") && boundary(word, 4, b"&|") {
        return Some(-1);
    }
    if word.starts_with(b"if") && boundary(word, 2, b"") {
        return Some(1);
    }
    if word.starts_with(b"fi") && boundary(word, 2, b"&|") {
        return Some(-1);
    }
    if word.starts_with(b"for") && boundary(word, 3, b"") {
        return Some(1);
    }
    if word.starts_with(b"wait") && boundary(word, 4, b"") {
        return Some(i32::from(wait_opens_block(line, word_end)));
    }
    if (word.starts_with(b"while") || word.starts_with(b"until")) && boundary(word, 5, b"") {
        return Some(1);
    }
    if word.starts_with(b"done") && boundary(word, 4, b"&|") {
        return Some(-1);
    }
    if opens(word, line, word_end) {
        return Some(1);
    }
    if closes(word) {
        return Some(-1);
    }
    None
}

/// Length of the whole `time [-p]` prefix run at the start of `word` (`time
/// time -p` counts both prefixes), `None` if the word does not start with one.
/// Each prefix's own length comes from [`one_prefix_len`] and the loop adds the
/// whitespace closing it, so the returned length ends exactly at the word behind
/// the run. The prefix is transparent for the block-depth rule: the keyword
/// behind it (`time while … done`, `time -p { …; }`) is scored on the rest of the
/// word, which keeps such a statement one segment.
///
/// A bare `time` / `time -p` has no closing whitespace, so it counts as a prefix
/// of its own length: the group scanner trims the run before the `{`, so the
/// whitespace lies outside the word it scores there.
pub(crate) fn time_prefix_len(word: &[u8]) -> Option<usize> {
    let (mut total, mut rest) = (0usize, word);
    while let Some(n) = one_prefix_len(rest) {
        let tail = rest.get(n..).unwrap_or(b"");
        let ws = tail.iter().take_while(|b| b.is_ascii_whitespace()).count();
        let step = n + ws;
        total += step;
        rest = rest.get(step..).unwrap_or(b"");
    }
    (total > 0).then_some(total)
}

/// The `time` prefix word and its `-p` flag, with their lengths derived from the
/// bytes: nothing to keep in step, and no integer literal for `cargo mutants` to
/// move — every arm of [`one_prefix_len`] shares these lengths, so a length
/// change is caught by the bare `time` pins.
const TIME: &[u8] = b"time";
const FLAG: &[u8] = b"-p";
const TIME_LEN: usize = b"time".len();
const FLAG_LEN: usize = b"-p".len();

/// Length of a single `time [-p]` prefix at the start of `word`, `None` if the
/// word does not start with one. The whitespace *between* `time` and its `-p` is
/// part of the prefix; the whitespace *closing* it is not (the loop in
/// [`time_prefix_len`] owns it, so no term is counted twice). A glued `-px` is
/// not the flag, and any other word behind the prefix is not the keyword
/// (`time ls` is the command `ls`, timed).
fn one_prefix_len(word: &[u8]) -> Option<usize> {
    let tail = word.strip_prefix(TIME)?;
    let ws = tail.iter().take_while(|b| b.is_ascii_whitespace()).count();
    let after = tail.get(ws..).unwrap_or(b"");
    if after.is_empty() {
        return Some(TIME_LEN);
    }
    match after.strip_prefix(FLAG) {
        None => (ws > 0).then_some(TIME_LEN),
        Some(rest) => {
            let ws2 = rest.iter().take_while(|b| b.is_ascii_whitespace()).count();
            (rest.get(ws2..).unwrap_or(b"").is_empty() || ws2 > 0)
                .then_some(TIME_LEN + ws + FLAG_LEN)
        }
    }
}

#[cfg(test)]
mod tests;
