// mutants: -C --lib -- -E 'test(/^parse::case_block::/)'

//! The brace-expansion-protected token indices of a `case` line: the `case`
//! word and every clause's pattern region. Lives beside `tokens_to_case`
//! because it mirrors that traversal — the file therefore owns both walks, the
//! one that answers `tokens_to_case` and the one that answers `literal_indices`.

use super::Token;
use crate::error::parse::ParseError;
use crate::keywords::Closer;
use crate::parse::block_tail;
use crate::parse::semi::{List, list_closing_keyword_index};
use alloc::vec::Vec;
use error_stack::{Report, ensure};

/// The token indices of the case word and every clause's pattern region in
/// a `case` line (bash applies no brace expansion to them); empty for other
/// lines. Mirrors the traversal of `tokens_to_case` without the checks.
///
/// The token slice is truncated at the closing keyword first: in a
/// single-line `case` the clause heredoc's bodies sit **after** `esac`, so the
/// tokens past it are body data and a consumer that keys on the last token is a
/// body-data consumer — without the truncation the pattern is brace-expanded
/// on the way to the parser. The walk is [`list_closing_keyword_index`] with
/// [`List::Case`], so the word list's closer-shaped **patterns**
/// (`case x in done) …`) stay word data here too, exactly as `tokens_to_case`
/// reads them.
pub(crate) fn literal_indices(tokens: &[Token], line: &[u8]) -> Vec<usize> {
    let Some(esac_idx) = list_closing_keyword_index(tokens, line, Closer::Esac, List::Case) else {
        return Vec::new();
    };
    let Some(tokens) = tokens.get(..=esac_idx) else {
        return Vec::new();
    };
    let Some(first) = tokens.first() else {
        return Vec::new();
    };
    // The truncation gate above is the `esac` half of the rule: it only ever
    // accepts a line whose word list closes on an `esac`-shaped token, so the
    // last token's shape says nothing the guard has to repeat (its `||` had a
    // family no test could see). The guard states the one rule left.
    if !first.word.eq_bytes(b"case") {
        return Vec::new();
    }
    let Some(in_idx) = (1..tokens.len()).find(|&i| word_at(tokens, i, b"in")) else {
        return Vec::new();
    };
    // The clause walk starts one word behind `in`, and the pattern words are
    // everything before it: one base for both, so a mutant of the step drops a
    // pattern word from the output **and** moves the walk.
    let base = in_idx + 1;
    let mut out: Vec<usize> = (1..base - 1).collect();
    let mut pos = base;
    // No loop bound: the searches below carry the ranges (`pos..esac_idx`), and
    // once `pos` reaches the `esac` index neither finds anything, so the walk
    // ends by the `break` — a bound of its own is a duplicate that only re-probes
    // the closer, which no needle can match.
    loop {
        if word_at(tokens, pos, b";") {
            pos += 1;
            continue;
        }
        let Some(close) = (pos..esac_idx).find(|&i| word_at(tokens, i, b")")) else {
            break;
        };
        out.extend(pos..close);
        // No step offset and no window trim: `close` is the `)` token and the
        // pinned pattern tokens cannot match the searched needle, so no `;;`
        // pair starts at `close`, at `close + 1`, or at the token before the
        // `esac` closer (whose pair-test's second slot is the closer itself) —
        // the walk answers identically and no operator is left to mutate.
        pos = close;
        let pair =
            (pos..esac_idx).find(|&i| word_at(tokens, i, b";") && word_at(tokens, i + 1, b";"));
        pos = pair.map_or(esac_idx, |i| i + 2);
    }
    out
}

/// The needle word at `i`, gated on the raw span ([`crate::parse::Token::span_word`], task #336): a
/// quoted `";"` / `")"` is pattern data, and the `in` needle is the keyword the
/// mirror site `case_block.rs` reads through [`keyword_word`] — one span fact
/// for a keyword needle, so the #455/#273 mirror property holds.
fn word_at(tokens: &[Token], i: usize, w: &[u8]) -> bool {
    tokens.get(i).is_some_and(|tok| tok.span_word(w))
}

/// What the `case` word slot owns as its own separator: the `;` (and the newline that
/// tokenizes as one) [`crate::parse::semi::trim_semi`] trims — task **#470**'s decision,
/// and the ONE predicate's per-site data. Every other row of the ONE set, and a word
/// opening with `(`, is the control operator bash rejects at this position.
pub(crate) fn owns_semi(w: &[u8]) -> bool {
    w == b";"
}

/// The `case` block's tokens up to and including its closing `esac`: the word
/// walk of [`list_closing_keyword_index`] with [`List::Case`], whose closer must
/// be an `esac`, plus the block's own leniency. A last clause whose body is not
/// `;;`-terminated has its `esac` in argument position, where no closer scores —
/// bash rejects the shape, fdshell accepts it
/// (`parse::tests::case_last_clause_no_semi_semi`), so an unclosed walk keeps
/// the whole list, and its last token must still be `esac`.
pub(crate) fn up_to_esac<'a>(
    tokens: &'a [Token],
    line: &[u8],
) -> Result<&'a [Token], Report<ParseError>> {
    let rest = match list_closing_keyword_index(tokens, line, Closer::Esac, List::Case) {
        Some(idx) => tokens.get(..=idx).ok_or(ParseError::CaseMissingEsac)?,
        None => tokens,
    };
    ensure!(
        rest.last()
            .is_some_and(|tok| block_tail::is_closer(tok, b"esac")),
        ParseError::CaseMissingEsac
    );
    Ok(rest)
}

#[cfg(test)]
mod tests;
