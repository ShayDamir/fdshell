// mutants: -C --lib -C --test -C loops -- -E 'test(/^keywords::kind::arith_for::/) | test(/^parse::for_block::/) | binary(loops)'

//! The **arithmetic-`for` head** at the byte level (task #394): the `for`
//! keyword word and the `(( … ))` region written behind it. The recognition
//! table's row and the segment scan's run tests ask the same question from two
//! sides — what stands behind the word, and whether a run is nothing but the
//! head — so they share this one byte rule.

use crate::paren_scan;
use crate::scan::{Boundary, ScanState, boundary, comment_end};
use crate::segment::group::opens_at;

/// The region's opening pair, measured from the bytes so nothing here is a
/// literal for `cargo mutants` to move. Its depth is the whole pair, which is
/// what [`paren_scan::scan_at`] wants from a caller that consumed it.
const OPEN: &[u8] = b"((";

/// The `for` word the head is measured from (the same length `opener_len`
/// measures; the run test only ever sees the word at its own start).
const FOR: &[u8] = b"for";

/// The text behind the `for` word is a closed `(( … ))` region: `Some` = the byte
/// just past it, `None` when there is no region (whitespace-separated or glued —
/// task #396 — both spellings land here) or one that never closes. The region is
/// opaque to every byte rule, so the walk is
/// [`paren_scan::scan_at`]'s tokenizer unit with the whole opening pair counted as
/// consumed: it ends at the closing pair's **first** `)`, so the pair's second `)`
/// must sit on the next byte — the same shape the arithmetic `for`'s own region
/// measure uses to join its clauses.
fn region_end(text: &[u8]) -> Option<usize> {
    let gap = ws_len(text);
    let body = text.get(gap..)?;
    body.starts_with(OPEN).then(|| {
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
/// layer's tail rule allows as the head's separator, any interleaving of whitespace
/// runs and comment rows, and the opener. `)) ;; {` and `)) ; ; {` have two
/// separators, so they answer `false` and keep their parse errors, as bash's rc-2
/// pair does. The `;` slot sits **before** the comment rows, and that order is
/// load-bearing: bash rc 2 rows like `))` + `# c` + `; { …; }` (the comment
/// **behind** the `;`) answer `false` exactly because the slot is spent before the
/// first comment row (task #518). [`gap_end`] consumes every whitespace run it walks,
/// so its answer is exactly where the opener has to stand — no skip of its own here
/// (one would be an operator over a length that is always zero).
fn opener_behind(text: &[u8], at: usize) -> bool {
    opens_at(text, gap_end(text, at))
}

/// The gap the byte rule allows between the head and its opener: whitespace, the
/// single `;` slot, and any interleaving of whitespace runs and comment rows — the
/// end of what the walk consumes, so [`is_run_head`] can ask for it to have eaten
/// the whole run, and [`opener_behind`] asks it to end on an opener (task #518).
fn gap_end(text: &[u8], at: usize) -> usize {
    let at = at + ws_len(text.get(at..).unwrap_or(b""));
    let at = at + usize::from(text.get(at) == Some(&b';'));
    comment_rows(text, at + ws_len(text.get(at..).unwrap_or(b"")))
}

/// Any run of whitespace runs and `#` comment rows at `at` — the gap rows a comment
/// may insert between the `;` slot and the opener (task #518, bash runs them). The
/// comment test is the shared [`boundary`] gate (a bare word-start `#`, outside
/// quotes, backticks and regions), so a quoted `"#"` row, an escaped `\#` and a `#`
/// glued to a word stay ordinary rows and keep their parse errors; a fresh state is
/// right because the region has closed before the gap. The skip is the shared
/// [`comment_end`], landing **on** the newline that ends the comment (its sibling
/// `skip_comment` lands past it), so this walk's own whitespace step consumes the
/// newline and tests the next row.
fn comment_rows(text: &[u8], mut at: usize) -> usize {
    let state = ScanState::new();
    while boundary(text, at, &state) == Boundary::Comment {
        at = comment_end(text, at);
        at += ws_len(text.get(at..).unwrap_or(b""));
    }
    at
}

/// The run test the group branch and the span start share: the whole of `text` is
/// the head — the `for` keyword word and its region, nothing but the gap the byte
/// rule allows (task #518) — so the `{` the caller is looking at is the body of
/// *this* construct. It is the `time [-p]`
/// prefix's twin: the head sits inside the span so the parse layer sees one
/// statement, and the block branch that opens it is `segment::block`'s arm, never
/// the `opener_closer` fallback (task #394).
pub(crate) fn is_run_head(text: &[u8]) -> bool {
    let Some(rest) = text.strip_prefix(FOR) else {
        return false;
    };
    region_end(rest).is_some_and(|past| gap_end(rest, past) == rest.len())
}

#[cfg(test)]
mod tests;
