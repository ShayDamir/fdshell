use super::Token;
use super::case_clause;
use super::semi::{trim_semi, try_join};
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ScriptText;
use sys::ShortCStr;

pub struct CaseBlock {
    pub word: ShortCStr,
    pub clauses: Vec<case_clause::CaseClause>,
}

pub(crate) fn tokens_to_case(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<CaseBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"case")),
        ParseError::MalformedCaseBlock
    );

    let in_idx = (1..tokens.len())
        .find(|&i| {
            tokens
                .get(i)
                .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"in"))
        })
        .ok_or(ParseError::CaseMissingIn)?;

    let esac_idx = tokens.len() - 1;
    ensure!(
        tokens
            .last()
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"esac")),
        ParseError::CaseMissingEsac
    );

    let word = try_join(trim_semi(
        tokens.get(1..in_idx).ok_or(ParseError::CaseMissingIn)?,
    ));

    let clauses = case_clause::parse_clauses(tokens, in_idx + 1, esac_idx, text)?;

    Ok(CaseBlock { word, clauses })
}

/// The token indices of the case word and every clause's pattern region in
/// a `case` line (bash applies no brace expansion to them); empty for other
/// lines. Mirrors the traversal of [tokens_to_case] without the checks.
pub(crate) fn literal_indices(tokens: &[Token]) -> Vec<usize> {
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
