//! The brace-expansion-protected token indices of a `case` line: the `case`
//! word and every clause's pattern region. Lives beside `tokens_to_case`
//! because it mirrors that traversal.

use super::Token;
use crate::parse::semi::{List, list_closing_keyword_index};
use alloc::vec::Vec;

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
    let Some(esac_idx) = list_closing_keyword_index(tokens, line, List::Case) else {
        return Vec::new();
    };
    let Some(tokens) = tokens.get(..=esac_idx) else {
        return Vec::new();
    };
    let (Some(first), Some(last)) = (tokens.first(), tokens.last()) else {
        return Vec::new();
    };
    if !first.0.eq_bytes(b"case") || !last.0.eq_bytes(b"esac") {
        return Vec::new();
    }
    let Some(in_idx) = (1..tokens.len()).find(|&i| word_at(tokens, i, b"in")) else {
        return Vec::new();
    };
    let esac_idx = tokens.len() - 1;
    let mut out: Vec<usize> = (1..in_idx).collect();
    let mut pos = in_idx + 1;
    while pos < esac_idx {
        if word_at(tokens, pos, b";") {
            pos += 1;
            continue;
        }
        let Some(close) = (pos..esac_idx).find(|&i| word_at(tokens, i, b")")) else {
            break;
        };
        out.extend(pos..close);
        pos = close + 1;
        let pair = (pos..esac_idx.saturating_sub(1))
            .find(|&i| word_at(tokens, i, b";") && word_at(tokens, i + 1, b";"));
        pos = pair.map_or(esac_idx, |i| i + 2);
    }
    out
}

fn word_at(tokens: &[Token], i: usize, w: &[u8]) -> bool {
    tokens.get(i).is_some_and(|(t, _, _, _, _)| t.eq_bytes(w))
}

#[cfg(test)]
mod tests;
