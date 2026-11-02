//! The `|`-separated pattern walk of one `case` clause: the pattern list, up to
//! and including the `)` that ends it (POSIX 2.9.4.3's `( pattern-list )`).

use super::pattern;
use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::pipe_token_span;
use crate::parse::semi::trim_semi;
use crate::parse::word_quoted::span_word;
use alloc::vec::Vec;
use error_stack::{Report, bail, ensure};

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
pub(super) fn collect_patterns(
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
