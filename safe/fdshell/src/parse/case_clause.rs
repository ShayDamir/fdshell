pub(crate) mod extract;
pub(crate) mod pattern;
pub(crate) mod stray;

use super::Token;
use super::pipe_token_span;
use super::semi::trim_semi;
use super::word_quoted::span_word;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail, ensure};
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
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let mut clauses = Vec::new();
    let mut pos = start;
    while pos < esac_idx {
        if tokens.get(pos).is_some_and(|tok| span_word(tok, b";")) {
            pos += 1;
            continue;
        }
        let patterns = collect_patterns(tokens, line, &mut pos, esac_idx)?;
        let (body, next_pos) = extract::body(tokens, text, pos, esac_idx)?;
        clauses.push(CaseClause { patterns, body });
        pos = next_pos;
    }
    Ok(clauses)
}

/// Collect the `|`-separated patterns of one clause, up to and including `)`.
/// Advances `pos` past the closing paren.
///
/// The clause's first token goes through [`pattern::take_open_paren`] first, so
/// the pattern list starts behind bash's optional leading `(` (POSIX 2.9.4.3's
/// `( pattern-list )`, task #334).
///
/// The separator tests read **raw spans**, never the token text. The `|` is
/// [`pipe_token_span`]'s — task #261's ONE pipe-token predicate: a pipe token's
/// span is exactly one `|` byte, while quote removal makes a `"|"`, a `'|'` and
/// an escaped `\|` word tokens whose *text* is `|`. `)`, `;` and `;;` keep task
/// #336's [`span_word`] gate (its rule for a separator word in general), so a
/// quoted `")"` / `"|"` / `";"` is **pattern text**: `case x in a"|"b)` is the
/// one pattern `a|b`, and a leading `";"` is a pattern word, never the skip.
///
/// The **paren** ends the pattern list, not the position: a clause whose `)` is
/// the token right before `esac` is a clause with an empty body
/// (`case x in a) esac`, bash rc 0, and the multi-line spelling reaches the same
/// walk through the newline's `;` token), so only a pattern list that never
/// reaches its `)` is the missing-paren shape (`case x in a esac`,
/// `case x in echo yes;; esac`).
fn collect_patterns(
    tokens: &[Token],
    line: &[u8],
    pos: &mut usize,
    esac_idx: usize,
) -> Result<Vec<pattern::Pattern>, Report<ParseError>> {
    let mut patterns = Vec::new();
    let mut current_pattern = Vec::new();
    let mut closed_paren = false;
    // One token, one byte, the clause's **first** token only: `((a*))` keeps its
    // second `(`, and a `(` written behind a `|` (`z|(a)`) is pattern text.
    if let Some(opened) = tokens.get(*pos).and_then(pattern::take_open_paren) {
        current_pattern.extend(opened);
        *pos += 1;
    }
    while *pos <= esac_idx {
        let Some(tok @ (_t, start, end, _fq, _mask, _spans)) = tokens.get(*pos) else {
            break;
        };
        if span_word(tok, b")") {
            *pos += 1;
            closed_paren = true;
            break;
        }
        if pipe_token_span(line, *start, *end) {
            if current_pattern.is_empty() {
                bail!(ParseError::CaseEmptyPattern);
            }
            patterns.push(pattern::join(trim_semi(&current_pattern)));
            current_pattern.clear();
            *pos += 1;
        } else {
            current_pattern.push(tok.clone());
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
