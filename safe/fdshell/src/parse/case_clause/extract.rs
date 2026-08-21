use super::Token;
use crate::error::parse::ParseError;
use crate::parse::semi::{trim_semi, verbatim};
use error_stack::Report;
use sys::ScriptText;

/// Extract one clause body starting at `body_start`, up to the `;;`
/// clause separator or `esac_idx`. Returns the body text and the
/// position to resume clause scanning from.
pub(crate) fn body(
    tokens: &[Token],
    text: &ScriptText,
    body_start: usize,
    esac_idx: usize,
) -> Result<(ScriptText, usize), Report<ParseError>> {
    let body_slice = tokens.get(body_start..esac_idx).unwrap_or(&[]);
    let end = body_slice
        .windows(2)
        .position(|w| matches!(w, [a, b] if a.0.eq_bytes(b";") && b.0.eq_bytes(b";")))
        .map(|i| body_start + i);
    match end {
        Some(end) => {
            let b = verbatim(text, trim_semi(tokens.get(body_start..end).unwrap_or(&[])))?;
            Ok((b, end + 2))
        }
        None => {
            let b = verbatim(
                text,
                trim_semi(tokens.get(body_start..esac_idx).unwrap_or(&[])),
            )?;
            Ok((b, esac_idx))
        }
    }
}
