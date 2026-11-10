// mutants: -C --lib -C --test -C case -- -E 'test(/^parse::case_clause::/) | binary(case)'

pub(crate) mod extract;
pub(crate) mod pattern;
mod patterns;
pub(crate) mod stray;
pub(crate) mod terminator;

use super::Token;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;

pub struct CaseClause {
    /// The clause's `|`-separated patterns, each with its own quote mask.
    pub patterns: Vec<pattern::Pattern>,
    pub body: ScriptText,
    /// What the clause's terminator says happens after this clause ran
    /// ([`terminator`]: `;;` stops, `;&` runs the next clause, `;;&` tests the
    /// next clause's patterns against the same word).
    pub terminator: terminator::Terminator,
}
pub fn parse_clauses(
    tokens: &[Token],
    start: usize,
    esac_idx: usize,
    text: &ScriptText,
) -> Result<Vec<CaseClause>, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let mut clauses = Vec::new();
    let mut pos = start;
    while pos < esac_idx {
        if tokens.get(pos).is_some_and(|tok| tok.span_word(b";")) {
            pos += 1;
            continue;
        }
        let patterns = patterns::collect_patterns(tokens, line, &mut pos, esac_idx)?;
        let (body, terminator, next_pos) = extract::body(tokens, text, pos, esac_idx)?;
        clauses.push(CaseClause {
            patterns,
            body,
            terminator,
        });
        pos = next_pos;
    }
    Ok(clauses)
}

#[cfg(test)]
mod tests;
