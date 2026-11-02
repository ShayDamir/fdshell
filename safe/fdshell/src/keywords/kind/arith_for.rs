//! The **arithmetic-`for` head** at the byte level (task #394): the `for`
//! keyword word and the `(( … ))` region written behind it. The recognition
//! table's row and the segment scan's run tests ask the same question from two
//! sides — what stands behind the word, and whether a run is nothing but the
//! head — so they share this one byte rule.

use crate::paren_scan;
use crate::segment::group::opens_at;

/// The region's opening pair, measured from the bytes so nothing here is a
/// literal for `cargo mutants` to move. Its depth is the whole pair, which is
/// what [`paren_scan::scan_at`] wants from a caller that consumed it.
const OPEN: &[u8] = b"((";

/// The `for` word the head is measured from (the same length `opener_len`
/// measures; the run test only ever sees the word at its own start).
const FOR: &[u8] = b"for";

/// The text behind the `for` word is a closed `(( … ))` region: `Some` = the byte
/// just past it, `None` when there is no whitespace gap, no region, or a region
/// that never closes. The region is opaque to every byte rule, so the walk is
/// [`paren_scan::scan_at`]'s tokenizer unit with the whole opening pair counted as
/// consumed: it ends at the closing pair's **first** `)`, so the pair's second `)`
/// must sit on the next byte — the same shape the arithmetic `for`'s own region
/// measure uses to join its clauses.
fn region_end(text: &[u8]) -> Option<usize> {
    let gap = ws_len(text);
    let body = text.get(gap..)?;
    (gap > 0 && body.starts_with(OPEN)).then(|| {
        let (_, past) = paren_scan::scan_at(body, OPEN.len(), OPEN.len() as u32)?;
        (body.get(past) == Some(&b')')).then_some(gap + past + 1)
    })?
}

/// The length of the whitespace run at the start of `text`.
fn ws_len(text: &[u8]) -> usize {
    text.iter().take_while(|b| b.is_ascii_whitespace()).count()
}

/// The recognition table's row: the text behind the `for` word is a closed region
/// with a group opener behind it — through the one `;` gap the parse layer's tail
/// rule allows — so the word contributes **no pairing frame**: the brace pair nets
/// zero in every scorer, and nesting, the cond split and the closer searches agree
/// without a `done` frame nothing pops.
pub(crate) fn opens_a_group(line: &[u8], word_end: usize) -> bool {
    let text = line.get(word_end..).unwrap_or(b"");
    region_end(text).is_some_and(|past| opener_behind(text, past))
}

/// The run gate's form of the same question, seen from the whole run instead of the
/// word's end (task #394): the run is the `for` word plus its region and nothing
/// else, and the group opener written behind the run is this construct's body — so
/// the block branch takes the run and its body as one statement. A run that is a
/// plain `do … done` for answers `false`; the `done` frame owns it.
pub(crate) fn opens_a_block(run: &[u8], line: &[u8], start: usize) -> bool {
    is_run_head(run) && opener_behind(line, start + run.len())
}

/// The group opener standing behind the region at `at`: the single `;` the parse
/// layer's tail rule allows as the head's separator, the whitespace around it, and
/// the opener. `)) ;; {` and `)) ; ; {` have two separators, so they answer `false`
/// and keep their parse errors, as bash's rc-2 pair does.
fn opener_behind(text: &[u8], at: usize) -> bool {
    let at = at + ws_len(text.get(at..).unwrap_or(b""));
    let at = at + usize::from(text.get(at) == Some(&b';'));
    opens_at(text, at + ws_len(text.get(at..).unwrap_or(b"")))
}

/// The run test the group branch and the span start share: the whole of `text` is
/// the head — the `for` keyword word and its region, nothing else — so the `{` the
/// caller is looking at is the body of *this* construct. It is the `time [-p]`
/// prefix's twin: the head sits inside the span so the parse layer sees one
/// statement, and the block branch that opens it is `segment::block`'s arm, never
/// the `opener_closer` fallback (task #394).
pub(crate) fn is_run_head(text: &[u8]) -> bool {
    let Some(rest) = text.strip_prefix(FOR) else {
        return false;
    };
    region_end(rest).is_some_and(|past| past == rest.len())
}

#[cfg(test)]
mod tests;
