use super::semi::{trim_semi, try_join};
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

pub struct CaseClause {
    pub patterns: Vec<ShortCStr>,
    pub body: ShortCStr,
}
pub fn parse_clauses(
    tokens: &[(ShortCStr, usize, bool)],
    start: usize,
    esac_idx: usize,
) -> Result<Vec<CaseClause>, Report<ParseError>> {
    let mut clauses = Vec::new();
    let mut pos = start;
    while pos <= esac_idx {
        if pos == esac_idx {
            break;
        }
        while pos <= esac_idx && tokens.get(pos).is_some_and(|(t, _, _)| t.eq_bytes(b";")) {
            pos += 1;
        }
        if pos == esac_idx {
            break;
        }
        let mut patterns = Vec::new();
        let mut current_pattern = Vec::new();
        while pos <= esac_idx {
            let Some((token, _, _)) = tokens.get(pos) else {
                break;
            };
            if token.eq_bytes(b")") {
                pos += 1;
                break;
            }
            if token.eq_bytes(b"|") {
                if current_pattern.is_empty() {
                    bail!(ParseError::CaseEmptyPattern);
                }
                patterns.push(try_join(trim_semi(&current_pattern)));
                current_pattern.clear();
                pos += 1;
            } else if let Some(token) = tokens.get(pos) {
                current_pattern.push(token.clone());
                pos += 1;
            }
        }
        if pos >= esac_idx {
            bail!(ParseError::CaseMissingCloseParen);
        }
        if current_pattern.is_empty() && patterns.is_empty() {
            bail!(ParseError::CaseEmptyPattern);
        }
        if !current_pattern.is_empty() {
            patterns.push(try_join(trim_semi(&current_pattern)));
        }
        let body_start = pos;
        let mut found = false;
        let mut body_end = esac_idx;
        let body_slice = tokens.get(body_start..esac_idx).unwrap_or(&[]);
        for (i, window) in body_slice.windows(2).enumerate() {
            let [a, b] = window else {
                continue;
            };
            if a.0.eq_bytes(b";") && b.0.eq_bytes(b";") {
                found = true;
                body_end = body_start + i;
                break;
            }
        }
        let (body, next_pos) = if found {
            let b = try_join(trim_semi(tokens.get(body_start..body_end).unwrap_or(&[])));
            (b, body_end + 2)
        } else {
            let b = try_join(trim_semi(tokens.get(body_start..esac_idx).unwrap_or(&[])));
            (b, esac_idx)
        };
        clauses.push(CaseClause { patterns, body });
        pos = next_pos;
    }
    Ok(clauses)
}
