//! The word-byte walk of the delimiter word: quote spans, escape pairs and
//! `$( … )` regions consumed whole (the module `lines` splits scan from walk).

use crate::bytes::{QUOTE, SQUOTE};
use crate::paren_scan::scan_at;

/// The index just past the delimiter word byte at `j`: a quoted span (`"…"`
/// or `'…'`) is consumed whole, because the tokenizer reads it as one word and
/// its spaces are word content (`<<'a b'` and `<<"a b"` delimit `a b`) —
/// byte-level and token-level rules must agree (LESSONS). So is an unquoted
/// escape pair `\X`: `parse/token/escape.rs::read_escape` keeps **both** bytes
/// in the word text, so a walk that steps over the `\` alone ends the word at
/// the separator byte **behind** it (`token_word_end`'s set) and the region
/// layer delimits on a truncated word — the visible signature is the false
/// `missing terminating delimiter line` while bash reads the whole folded word
/// (`<<a\ b` delimits `a b`, `<<\;` delimits `;`). A `\` at the window end
/// stays a lone byte: the pair never steps over the physical-line end (the
/// `\`+newline continuation is task #489's), exactly `read_escape`'s
/// trailing-`\` rule, and `fold_quotes` keeps that backslash. So is a `$( … )`
/// region opened on the word ([`substitution_span`]).
pub(crate) fn word_span(line: &[u8], j: usize, to: usize) -> usize {
    if line.get(j) == Some(&b'\\') {
        return j.saturating_add(2).min(to);
    }
    if let Some(end) = substitution_span(line, j, to) {
        return end;
    }
    let quote = line.get(j).copied().filter(|b| *b == QUOTE || *b == SQUOTE);
    let Some(q) = quote else {
        return j + 1;
    };
    let mut k = j + 1;
    // The walk ends through the range's own `is_empty` (no operator to
    // mutate — rule 18/24, task #513); the quote test and the clamp below
    // are byte-for-byte the old loop's.
    loop {
        if (k..to).is_empty() {
            break;
        }
        if line.get(k) == Some(&q) {
            break;
        }
        k += 1;
    }
    k.saturating_add(1).min(to)
}

/// The index just past a `$( … )` region opened at `j` — the word byte at `j`
/// is an unquoted `$` followed by `(`. The tokenizer reads such a region as one
/// unit inside the word (`token/step.rs`'s `$(` arm), so the byte-level word
/// scan must not end the word at the `)` that closes it: the delimiter word of
/// `cat <<$(x)` is the whole `$(x)`, and the terminator line that matches it is
/// `$(x)`, not `$(x` (LESSONS: "A quoted region is a unit for *every* byte
/// scanner", and `scan/advance.rs::advance` consumes `$(` for every scanner that
/// shares it). `to` bounds the region: a closer past the window end is not this
/// word's closer, and an unbalanced region returns `None` so the walk stays byte
/// at a time — the fallback every region scanner uses for unbalanced input. No
/// script reaches that arm: a here-doc word whose `$( ` is never closed is a parse
/// error in both shells (bash `unexpected EOF while looking for matching ')'`,
/// fdshell `unexpected end of input`), so the arm is a guard, unpinned on purpose.
pub(super) fn substitution_span(line: &[u8], j: usize, to: usize) -> Option<usize> {
    if line.get(j) != Some(&b'$') || line.get(j + 1) != Some(&b'(') {
        return None;
    }
    let window = line.get(..to)?;
    let (_, end) = scan_at(window, j + 2, 1)?;
    Some(end)
}
