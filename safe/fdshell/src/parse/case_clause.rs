pub(crate) mod extract;
pub(crate) mod pattern;
pub(crate) mod stray;

use super::Token;
use super::semi::trim_semi;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, bail, ensure};
use sys::ScriptText;

pub struct CaseClause {
    /// The clause's `|`-separated patterns, each with its own quote mask.
    pub patterns: Vec<pattern::Pattern>,
    pub body: ScriptText,
}
pub fn parse_clauses(
    tokens: &[Token],
    start: usize,
    esac_idx: usize,
    text: &ScriptText,
) -> Result<Vec<CaseClause>, Report<ParseError>> {
    let mut clauses = Vec::new();
    let mut pos = start;
    while pos < esac_idx {
        if tokens
            .get(pos)
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
        {
            pos += 1;
            continue;
        }
        let patterns = collect_patterns(tokens, &mut pos, esac_idx)?;
        let (body, next_pos) = extract::body(tokens, text, pos, esac_idx)?;
        clauses.push(CaseClause { patterns, body });
        pos = next_pos;
    }
    Ok(clauses)
}

/// Collect the `|`-separated patterns of one clause, up to and including `)`.
/// Advances `pos` past the closing paren.
///
/// The **paren** ends the pattern list, not the position: a clause whose `)` is
/// the token right before `esac` is a clause with an empty body
/// (`case x in a) esac`, bash rc 0, and the multi-line spelling reaches the same
/// walk through the newline's `;` token), so only a pattern list that never
/// reaches its `)` is the missing-paren shape (`case x in a esac`,
/// `case x in echo yes;; esac`).
fn collect_patterns(
    tokens: &[Token],
    pos: &mut usize,
    esac_idx: usize,
) -> Result<Vec<pattern::Pattern>, Report<ParseError>> {
    let mut patterns = Vec::new();
    let mut current_pattern = Vec::new();
    let mut closed_paren = false;
    while *pos <= esac_idx {
        let Some(token) = tokens.get(*pos) else {
            break;
        };
        if token.0.eq_bytes(b")") {
            *pos += 1;
            closed_paren = true;
            break;
        }
        if token.0.eq_bytes(b"|") {
            if current_pattern.is_empty() {
                bail!(ParseError::CaseEmptyPattern);
            }
            patterns.push(pattern::join(trim_semi(&current_pattern)));
            current_pattern.clear();
            *pos += 1;
        } else {
            current_pattern.push(token.clone());
            *pos += 1;
        }
    }
    ensure!(closed_paren, ParseError::CaseMissingCloseParen);
    if current_pattern.is_empty() && patterns.is_empty() {
        bail!(ParseError::CaseEmptyPattern);
    }
    if !current_pattern.is_empty() {
        patterns.push(pattern::join(trim_semi(&current_pattern)));
    }
    Ok(patterns)
}

#[cfg(test)]
mod tests;
