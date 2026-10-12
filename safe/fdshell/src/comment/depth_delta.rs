//! Block-depth deltas of the keywords in a word run.

use crate::bytes::SQUOTE;
use crate::scan::Quotes;
use alloc::vec::Vec;

/// Sum the block-depth deltas of the keywords in a space-separated word run.
///
/// A keyword only counts when its word is unquoted: a `for` / `if` / … inside
/// double quotes, a single-quoted span, backticks, or a `$( )` substitution is
/// data, not a nested block opener. The quote / backtick / `$( )` handling
/// follows `ScanState::advance`'s byte classification (a `'…'` span has no
/// escape pair, a `"` inside a span is data) — with one deliberate difference:
/// `advance` closes a span at its closing `'`, this scan never does. `bare` is
/// false while the span is open, so the closing `'` never matches its own arm
/// and the span stays open for the rest of the run.
///
/// That is safe: the run's tail is data (bash requires a closer keyword to sit
/// in command-word position, after a separator), the state is run-local (the
/// `Quotes` value is not returned to the caller) and it only ever suppresses
/// keyword scoring for the words after the span.
/// A run always starts at substitution depth 0 (separators only exist at
/// depth 0), so only the caller's quote state is taken as a parameter.
///
/// `line` and `base` (the run's offset in `line`) let a `wait` at the run's
/// end look ahead across the newline for its pattern keyword, matching the
/// segment scanner's disambiguation.
pub(crate) fn depth_delta(
    line: &[u8],
    raw: &[u8],
    base: usize,
    quotes: Quotes,
    in_backtick: bool,
) -> i32 {
    let mut sum = 0;
    let mut frag = Vec::new();
    let mut quotes = quotes;
    let mut in_backtick = in_backtick;
    let mut depth: u32 = 0;
    // A run starts quoted only when the caller is mid-quote/mid-backtick;
    // the flag is sticky until a split (splits only happen unquoted, so a
    // quoted word is one fragment and can never be a bare keyword).
    let mut frag_quoted = quotes.double || quotes.single || in_backtick;
    let mut i = 0usize;
    while let Some(&b) = raw.get(i) {
        let bare = !quotes.double && !quotes.single && !in_backtick;
        let mut step = 1usize;
        if b == SQUOTE && bare {
            // A `'…'` span is one literal word: its bytes are data and its
            // spaces are no split, so a keyword written in it never scores.
            quotes.single = !quotes.single;
            frag_quoted = true;
        } else if b == crate::bytes::QUOTE {
            quotes.double = !quotes.double;
        } else if bare && b == b'$' && raw.get(i + 1) == Some(&b'(') {
            depth = depth.saturating_add(1);
            step = 2;
        } else if bare && b == b'(' && depth > 0 {
            depth = depth.saturating_add(1);
        } else if bare && b == b')' {
            depth = depth.saturating_sub(1);
        } else if b == b'`' && bare != in_backtick {
            in_backtick = bare;
        } else if b == b' ' && bare && depth == 0 {
            // A split only exists outside quotes and substitutions, so the
            // next word always starts unquoted.
            if !frag_quoted {
                sum += crate::keywords::keyword_delta(&frag, line, base + i).unwrap_or(0);
            }
            frag.clear();
            frag_quoted = false;
            i += step;
            continue;
        }
        frag.push(b);
        i += step;
    }
    if !frag_quoted {
        sum += crate::keywords::keyword_delta(&frag, line, base + raw.len()).unwrap_or(0);
    }
    sum
}
