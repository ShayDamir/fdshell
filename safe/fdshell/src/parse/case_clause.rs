use super::semi::{trim_semi, try_join, verbatim};
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ScriptText;
use sys::ShortCStr;

pub struct CaseClause {
    pub patterns: Vec<ShortCStr>,
    pub body: ScriptText,
}
pub fn parse_clauses(
    tokens: &[(ShortCStr, usize, bool)],
    start: usize,
    esac_idx: usize,
    text: &ScriptText,
) -> Result<Vec<CaseClause>, Report<ParseError>> {
    let mut clauses = Vec::new();
    let mut pos = start;
    while pos < esac_idx {
        while pos <= esac_idx && tokens.get(pos).is_some_and(|(t, _, _)| t.eq_bytes(b";")) {
            pos += 1;
        }
        if pos == esac_idx {
            break;
        }
        let patterns = collect_patterns(tokens, &mut pos, esac_idx)?;
        let body_start = pos;
        let body_slice = tokens.get(body_start..esac_idx).unwrap_or(&[]);
        let end = body_slice
            .windows(2)
            .position(|w| matches!(w, [a, b] if a.0.eq_bytes(b";") && b.0.eq_bytes(b";")))
            .map(|i| body_start + i);
        let (body, next_pos) = match end {
            Some(end) => {
                let b = verbatim(text, trim_semi(tokens.get(body_start..end).unwrap_or(&[])))?;
                (b, end + 2)
            }
            None => {
                let b = verbatim(
                    text,
                    trim_semi(tokens.get(body_start..esac_idx).unwrap_or(&[])),
                )?;
                (b, esac_idx)
            }
        };
        clauses.push(CaseClause { patterns, body });
        pos = next_pos;
    }
    Ok(clauses)
}

/// Collect the `|`-separated patterns of one clause, up to and including `)`.
/// Advances `pos` past the closing paren.
fn collect_patterns(
    tokens: &[(ShortCStr, usize, bool)],
    pos: &mut usize,
    esac_idx: usize,
) -> Result<Vec<ShortCStr>, Report<ParseError>> {
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
            patterns.push(try_join(trim_semi(&current_pattern)));
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
        patterns.push(try_join(trim_semi(&current_pattern)));
    }
    Ok(patterns)
}
