//! Block-depth deltas of the keywords in a word run.

// mutants: -C --lib -C --test -C case -- -E 'test(/^comment::/) | binary(case)'

mod word_step;

use crate::bytes::SQUOTE;
use crate::keywords::{Pairing, Place};
use crate::scan::Quotes;
use alloc::vec::Vec;

pub(crate) use word_step::word_step;

/// Sum the block-depth deltas of the keywords in a whitespace-separated word
/// run — the two bytes bash breaks a word on that can sit **inside** a run: a
/// space and a tab (task #520: `case x in` + tab + `esac`).
///
/// The set is the tokenizer's whitespace minus the run-boundary bytes, and it
/// is neither of the two shared word-break predicates: `bytes::word_break`'s
/// metacharacters (`;` `|` `&` `<` `>` `(` `)`) are run *content* here — the
/// glued cond pair of task #236, the glued closer operand of task #413, the
/// clause `)` of tasks #396/#421 — while `;` and a newline are exactly where
/// `scan_block` broke the run; and `scan::advance::is_word_break`'s wider
/// whitespace is bash's, which refuses a `\v`/`\f`/`\r` break here, so those
/// bytes must stay word bytes.
///
/// A keyword only counts when its word is unquoted: a `for` / `if` / … inside
/// double quotes, a single-quoted span, backticks, or a `$( )` substitution is
/// data, not a nested block opener. The quote / backtick / `$( )` handling
/// follows `ScanState::advance`'s byte classification (a `'…'` span has no
/// escape pair, a `"` inside a span is data) — the toggles included: both quote
/// arms close their span at the closing byte (task #519), so a completed quoted
/// word splits like any other word, is scored `0` by the word-start quote gate,
/// and leaves the place an ordinary word leaves.
///
/// It must: "the run's tail is data" only holds **outside** a word list. Inside
/// one, task #273's rule keeps command position alive, so a quoted word must
/// stay word data and put a closer written behind it in argument position. The
/// state is run-local (the `Quotes` value is not returned to the caller).
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
///
/// `pairing` is the caller's open blocks, carried across the runs of one scan
/// (task #309): the returned **sum** is the depth arithmetic the caller used to
/// keep as an integer, while the pairing is what decides *which* closer closes
/// *which* block. A caller that only reads the sum (a depth counter clamped at
/// `0`) keeps its arithmetic; a block-span scan reads the pairing, because the
/// sum of a mismatched closer is `0` and the block stays open.
///
/// The pairing carries task #436's record (see [`Pairing::observe`] and
/// [`word_step`]) — the walk that
/// scans the **top level** reads it, every other walk leaves it alone.
pub(crate) fn depth_delta(
    line: &[u8],
    raw: &[u8],
    base: usize,
    quotes: Quotes,
    in_backtick: bool,
    seed: Place,
    pairing: &mut Pairing,
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
        if b == SQUOTE {
            // A `'…'` span is one word: its spaces are no split, its closing
            // `'` closes it, and the word gate is what keeps a keyword written
            // inside it out of the score — exactly the `"` arm below, and
            // `ScanState::advance` (task #519).
            quotes.single = !quotes.single;
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
        } else if (b == b' ' || b == b'\t') && bare && depth == 0 {
            // A split only exists outside quotes and substitutions, so the
            // next word always starts unquoted.
            if !frag_quoted {
                sum += word_step(&frag, line, base + i, &mut place, pairing);
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
        sum += word_step(&frag, line, base + raw.len(), &mut place, pairing);
    }
    sum
}
