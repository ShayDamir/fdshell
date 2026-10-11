/// Check that `word` ends at a token boundary at position `len`.
fn boundary(word: &[u8], len: usize, extra: &[u8]) -> bool {
    word.get(len)
        .is_none_or(|&b| b.is_ascii_whitespace() || b == b';' || extra.contains(&b))
}

mod function;
mod wait;

pub(crate) use function::{function_def_name, name_len};
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
/// A `time [-p]` word is a *prefix*, not a keyword of its own: it opens no
/// block, so it never enters the depth table. A block keyword behind it
/// (`time while … done`) is scored on the rest of the word, which keeps the
/// whole statement one segment for the `time` parse hook.
pub(super) fn keyword_delta(word: &[u8], line: &[u8], word_end: usize) -> Option<i32> {
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
/// The prefix is transparent for the block-depth rule: the keyword behind it
/// (`time while … done`, `time -p { …; }`) is scored on the rest of the word,
/// which keeps such a statement one segment. The whitespace between prefixes and
/// between the last prefix and the keyword is part of the prefix, so the
/// recursive score sees the keyword itself.
///
/// A bare `time` / `time -p` (no trailing whitespace) counts as a prefix of its
/// own length: the group scanner trims the run before the `{`, so the
/// whitespace lies outside the word it scores there.
pub(crate) fn time_prefix_len(word: &[u8]) -> Option<usize> {
    let (mut total, mut rest) = (0usize, word);
    while let Some(n) = one_prefix_len(rest) {
        total += n;
        rest = rest.get(n..).unwrap_or(b"");
        let ws = rest.iter().take_while(|b| b.is_ascii_whitespace()).count();
        total += ws;
        rest = rest.get(ws..).unwrap_or(b"");
    }
    (total > 0).then_some(total)
}

/// Length of a single `time [-p]` prefix at the start of `word`, `None` if the
/// word does not start with one. Whitespace inside the prefix and between the
/// prefix and the keyword behind it is part of the prefix; a glued `-px` is not
/// the flag.
fn one_prefix_len(word: &[u8]) -> Option<usize> {
    let tail = word.strip_prefix(b"time")?;
    let ws = tail.iter().take_while(|b| b.is_ascii_whitespace()).count();
    let after = tail.get(ws..).unwrap_or(b"");
    if after.is_empty() {
        return Some(4 + ws);
    }
    match after.strip_prefix(b"-p") {
        None => (ws > 0).then_some(4 + ws),
        Some(rest) => {
            let ws2 = rest.iter().take_while(|b| b.is_ascii_whitespace()).count();
            (rest.get(ws2..).unwrap_or(b"").is_empty() || ws2 > 0).then_some(6 + ws + ws2)
        }
    }
}

#[cfg(test)]
mod tests;
