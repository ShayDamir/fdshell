//! Block-depth deltas of the keywords in a word run.

use crate::bytes::SQUOTE;
use crate::keywords::{Glue, Place, after, keyword_delta, pair_payload, scores};
use crate::scan::Quotes;
use alloc::vec::Vec;

/// Score one word of the run and move the command-position flag with it
/// (task #236): bash recognizes a block keyword in command position *only*,
/// so a keyword-shaped *argument* (`echo if`, `echo done`) is word data, and
/// only the words behind a run start, a cond operator, `then`/`do`/`else`/
/// `elif`, a condition-taking opener, a group opener or a `case` clause's `)`
/// can open or close a nested block. A brace word stays ungated, the accepted
/// divergence `tests/group.rs::literal_brace_argument_in_a_block_body` pins.
fn word_step(frag: &[u8], line: &[u8], word_end: usize, place: &mut Place) -> i32 {
    let delta = scores(frag, line, word_end, *place)
        .then(|| keyword_delta(frag, line, word_end))
        .flatten()
        .unwrap_or(0);
    // The glued-pair payload (task #236's S2): a fragment carrying a cond
    // pair (`echo a&&if` is one whitespace unit) scores the keyword behind
    // the pair at the fragment's place — `echo a&&if` stays data (argument),
    // `a&&if` at a clause start nests the `if` so its `fi` closes *the if*.
    // The payload is scored at the *fragment's* end in `line` (`word_end`):
    // the payload is a slice of the fragment, so the fragment's own end is
    // the only offset into `line` that is true for it.
    let payload_delta = pair_payload(frag).map_or(0, |payload| {
        if scores(payload, line, word_end, *place) {
            keyword_delta(payload, line, word_end).unwrap_or(0)
        } else {
            0
        }
    });
    // The fragment is the run's *whitespace* unit: a glued pair is one
    // fragment, never a bare tail byte, so the fragment is never in a pair
    // ([`Glue::None`]).
    *place = after(frag, line, word_end, *place, Glue::None);
    delta + payload_delta
}

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
///
/// `seed` is the place the run's **first** word sits in. Every run starts at a
/// command position, with one exception (task #306): the first run of a `for`
/// block's span is the block's own word list, whose opener lies *before* the
/// scan starts, so [`crate::segment::block`] seeds it with
/// [`Place::Words`] — a closer-shaped list word (`for i in done`) must not drop
/// the depth. A `for` list never crosses a separator, so the seed is read once,
/// by the run the scan begins in.
pub(crate) fn depth_delta(
    line: &[u8],
    raw: &[u8],
    base: usize,
    quotes: Quotes,
    in_backtick: bool,
    seed: Place,
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
    // The run starts where the caller seeds it: a command position, or a `for`
    // block's own word list ([`depth_delta`'s `seed`]). `scan_block` breaks runs
    // at `;`/newline/comment/heredoc, and each break starts a new command word.
    let mut place = seed;
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
                sum += word_step(&frag, line, base + i, &mut place);
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
        sum += word_step(&frag, line, base + raw.len(), &mut place);
    }
    sum
}
