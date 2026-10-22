use super::Token;
use super::stray;
use crate::error::parse::ParseError;
use crate::parse::part_text::part_text;
use crate::parse::semi::{amp_semi, trim_body};
use error_stack::{Report, ResultExt, bail};
use sys::ScriptText;

/// Extract one clause body starting at `body_start`, up to the `;;`
/// clause separator or `esac_idx`. Returns the body text and the
/// position to resume clause scanning from.
///
/// The text is the slice's verbatim span with the heredoc body regions its own
/// `<<` operators claim attached (`part_text`): in a single-line `case` those
/// regions sit past `esac`, in a multi-line clause they are inside the span,
/// where `part_text`'s extension mode returns the slice's verbatim span
/// unchanged.
///
/// The arm region (up to the `;;` window that ends it) is also where a missing
/// clause separator shows up: [`stray`] decides the shape, and this site owns
/// the verdict (task #300).
pub(crate) fn body(
    tokens: &[Token],
    text: &ScriptText,
    body_start: usize,
    esac_idx: usize,
) -> Result<(ScriptText, usize), Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let body_slice = tokens.get(body_start..esac_idx).unwrap_or(&[]);
    let end = body_slice
        .windows(2)
        .position(|w| matches!(w, [a, b] if a.0.eq_bytes(b";") && b.0.eq_bytes(b";")))
        .map(|i| body_start + i);
    // The bound is the window's index **in the region**, so the next clause's
    // own pattern parens (`a) echo hi;; b) ;; esac`) are never in scope.
    if stray::opens_next_pattern(body_slice, end.map(|e| e - body_start), line) {
        bail!(ParseError::CaseMissingSemiSemi);
    }
    match end {
        Some(end) => {
            // A `;` glued behind a plain `&` is the statement terminator, not
            // the `;;` clause separator (task #355): keep the pair's head in
            // the body so the arm's own scan reports the `& ;` split (bash
            // rc 2); a glued `;;` pair is the separator itself, and the
            // resume stays past the pair — the leftover tail of a non-glued
            // pair is absorbed by `parse_clauses`' leading-`;` skip.
            let head_glued = tokens
                .get(end)
                .zip(tokens.get(end + 1))
                .is_some_and(|(h, t)| h.2 == t.1);
            let keep = usize::from(!head_glued && amp_semi(body_slice, line, end - body_start));
            let b = part_text(
                text,
                trim_body(tokens.get(body_start..end + keep).unwrap_or(&[]), line),
            )?;
            Ok((b, end + 2))
        }
        None => {
            let b = part_text(
                text,
                trim_body(tokens.get(body_start..esac_idx).unwrap_or(&[]), line),
            )?;
            Ok((b, esac_idx))
        }
    }
}
