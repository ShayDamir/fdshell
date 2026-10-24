mod function;
mod kind;
mod pair;
mod pairing;
mod position;
mod quoted;
mod wait;

pub(crate) use quoted::quoted_word;

pub(crate) use function::{definer_len, function_def_name, leading_ws, name_len};
pub(crate) use kind::{Closer, Keyword, keyword_kind};
pub(crate) use pair::pair_payload;
pub(crate) use pairing::Pairing;
pub(crate) use position::{Glue, Place, after, body_keyword, clause_keyword, list_keyword, scores};
pub(super) use wait::{first_word_end, is_wait_pattern_keyword, wait_opens_block};

/// Return `Some(1)` for keywords that open a block, `Some(-1)` for closers —
/// the **sign** projection of the recognition table [`keyword_kind`], which
/// carries the *kind* half of the pairing (task #309): `case`, `esac`, `if`,
/// `fi`, `for`, `wait`, `while`, `until`, `done`, `{`, `}`. The boundary rule,
/// the closer's `&|<>` set, the `wait` disambiguation, the quote gate and the
/// `time [-p]` strip are [`keyword_kind`]'s; a scorer that needs the kind reads
/// that table directly, and [`Pairing`] is the state that pairs the kinds.
///
/// The one behavior the projection cannot express: the POSIX builtin `wait` is
/// **no keyword** here (`None`), where the old table scored it `Some(0)`. Every
/// consumer reads the two identically — `scores`' `_` arm, `.unwrap_or(0)`,
/// `is_some_and(|d| d < 0)`, `midrun::span_end`'s `== Some(1)` gate — and
/// `keywords/tests.rs::keyword_delta_wait_block_vs_builtin` pins it.
pub(super) fn keyword_delta(word: &[u8], line: &[u8], word_end: usize) -> Option<i32> {
    keyword_kind(word, line, word_end).map(|k| match k {
        Keyword::Opens(_) => 1,
        Keyword::Closes(_) => -1,
    })
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
