pub(crate) mod extract;
pub(crate) mod pattern;

use super::Token;
use super::semi::trim_semi;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, bail};
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
fn collect_patterns(
    tokens: &[Token],
    pos: &mut usize,
    esac_idx: usize,
) -> Result<Vec<pattern::Pattern>, Report<ParseError>> {
    let mut patterns = Vec::new();
    let mut current_pattern = Vec::new();
    while *pos <= esac_idx {
        let Some(token) = tokens.get(*pos) else {
            break;
        };
        if token.0.eq_bytes(b")") {
            *pos += 1;
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
    if *pos >= esac_idx {
        bail!(ParseError::CaseMissingCloseParen);
    }
    if current_pattern.is_empty() && patterns.is_empty() {
        bail!(ParseError::CaseEmptyPattern);
    }
    if !current_pattern.is_empty() {
        patterns.push(pattern::join(trim_semi(&current_pattern)));
    }
    Ok(patterns)
}
