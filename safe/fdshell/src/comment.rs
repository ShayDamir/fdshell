mod depth_delta;

use crate::keywords::{Closer, Pairing, Place};
use crate::scan::{Boundary, Quotes, boundary, heredoc, skip_comment};

/// Scan forward from `i` looking for the matching closing keyword (the scan's
/// pairing empties). Updates `quotes` and `start` as side effects.
/// Returns `(end_position, block_was_closed)`.
///
/// `open` is the closer the scanned text's **own** block requires (task #309):
/// the scan starts inside the block, so its opener word is never read, and the
/// pairing it seeds pops only a closer of that kind — a `while` span survives a
/// `fi` written in its body, an `if` span survives a `done`, and a mismatched
/// closer is word data (see [`Pairing`]).
///
/// `seed` is the [`Place`] of the scanned text's first word: a command
/// position, or the block's own word list — a `for` block is scanned from just
/// past its `for`, so its list has no opener word for the scorer to read and
/// the caller seeds it with [`Place::Words`] (task #306,
/// [`depth_delta`'s seed][depth_delta::depth_delta]). Only the run the scan
/// begins in reads the seed: a word list never crosses a separator.
pub(crate) fn scan_block(
    line: &[u8],
    mut i: usize,
    quotes: &mut Quotes,
    start: &mut usize,
    open: Closer,
    mut seed: Place,
) -> (usize, bool) {
    let mut state = quotes.scan_state();
    let mut run_quotes = state.quotes();
    let mut run_backtick = state.in_backtick;
    let mut pairing = Pairing::new();
    pairing.open(open);
    while i <= line.len() && pairing.is_open() {
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = state.advance(line, i);
            continue;
        }
        // A separator or comment boundary ends the current word.
        state.word_active = false;
        let raw_untrimmed = line.get(*start..i).unwrap_or(b"");
        let lead = raw_untrimmed
            .iter()
            .take_while(|&&b| b.is_ascii_whitespace())
            .count();
        let raw = raw_untrimmed.trim_ascii();
        let base = *start + lead;
        // The run's keywords pair with the open blocks (its sum is what used to
        // be the loop's `depth`; the pairing carries it now).
        depth_delta::depth_delta(
            line,
            raw,
            base,
            run_quotes,
            run_backtick,
            seed,
            &mut pairing,
        );
        // The seed belongs to the run the scan began in only: every later run
        // starts a command word.
        seed = Place::Command;
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
            *start = i;
        } else if let Some(resume) = heredoc::skip(line, *start, i) {
            // A heredoc body is opaque: its lines carry no block keywords.
            i = resume;
            *start = i + 1;
        } else {
            *start = i + 1;
            i += 1;
        }
        run_quotes = state.quotes();
        run_backtick = state.in_backtick;
    }
    *quotes = state.quotes();
    (i, !pairing.is_open())
}

#[cfg(test)]
mod tests;
