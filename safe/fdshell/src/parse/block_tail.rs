//! The keyword block's **tail**: the redirections written after its closing
//! keyword (`while :; do :; done > f`). The closer's own separator (a `;`/newline,
//! not the keyword) is the block span's end, so a trailing redirection rides in
//! the block's own text and every builder's `up_to_closing` truncation drops it:
//! the tail is parsed **at** the closer, by this one module, exactly as
//! [`crate::parse::group_block`] does it for `{ …; }`.

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::classify::parse_redirect;
use crate::redirect::RedirectDef;
use crate::scan::heredoc::span_operator;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;

/// The bytes a closing keyword glues to and stays a closer: the redirection
/// operator bytes (`done>f`, `esac>>out`). The byte-level closer boundary in
/// [`crate::keywords::keyword_delta`] accepts these plus the `&`/`|` glue
/// (`done&`, `done| cat`), which the tokenizer already splits into their own
/// tokens, so only the redirection pair is the tail's own.
const GLUE: &[u8] = b"<>";

/// Whether `tok` is the block's closing keyword `kw`: the bare word, or the word
/// glued to a redirection operator (the form [`redirects`] splits).
pub(crate) fn is_closer(tok: &Token, kw: &[u8]) -> bool {
    let (t, _, _, _, _, _) = tok;
    if t.eq_bytes(kw) {
        return true;
    }
    t.starts_with(kw)
        && t.as_bytes()
            .is_ok_and(|b| b.get(kw.len()).is_some_and(|&c| GLUE.contains(&c)))
}

/// The index of the block's closing keyword in `tokens`: the **last** token the
/// closer rule matches, since a keyword-shaped word inside a body (`echo done`)
/// is word data and the closer is the block's own last keyword.
pub(crate) fn closer_index(tokens: &[Token], kw: &[u8]) -> Option<usize> {
    (0..tokens.len()).rfind(|&i| tokens.get(i).is_some_and(|t| is_closer(t, kw)))
}

/// The redirections written after the closing keyword `kw`, in source order:
/// `Scope::open` needs the list stable per target fd, and last-wins per fd is
/// what bash gives (`done > f > g` leaves `f` empty, `g` holds the output).
///
/// The walk **stops** at the first token that is not a redirection, and at a
/// `<<`-shaped operator (the shared raw-span shape test, so the io-number form
/// `done 3<<T` stops as a here-doc too): a plain word behind the closer is task
/// #405's and a here-doc/here-string on the closer is #408's, so both keep
/// exactly the behavior their pins record.
pub(crate) fn redirects(
    kw: &[u8],
    at_closer: &[Token],
    line: &[u8],
) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let tail = split_close(kw, at_closer);
    let mut redirects = Vec::new();
    let mut i = 0;
    while i < tail.len() {
        let (t, start, end, _fq, _mask, _spans) = tail.get(i).ok_or(ParseError::Never)?;
        if t.starts_with(b"<<") || span_operator(line, *start, *end).is_some() {
            break;
        }
        let Some((def, extra)) = parse_redirect(&tail, i)? else {
            break;
        };
        redirects.push(def);
        i += extra + 1;
    }
    Ok(redirects)
}

/// The tail tokens, with a glued closer split off: `done>f` contributes the
/// `>f` token, aligned with its raw range, mask and quote spans — the rule
/// `group_block::split_close` applies to the group's `}`.
fn split_close(kw: &[u8], at_closer: &[Token]) -> Vec<Token> {
    let rest_tokens = at_closer.get(1..).unwrap_or(&[]).to_vec();
    let Some((t, start, end, fq, mask, spans)) = at_closer.first().cloned() else {
        return rest_tokens;
    };
    let Some(rest) = t
        .get(kw.len()..)
        .filter(|r| r.starts_with(b"<") || r.starts_with(b">"))
    else {
        return rest_tokens;
    };
    let off = kw.len();
    let mut tail = vec![(
        rest,
        start + off,
        end,
        fq,
        mask.get(off..).unwrap_or(&[]).to_vec(),
        shift_spans(&spans, off),
    )];
    tail.extend_from_slice(&rest_tokens);
    tail
}

/// The quote-span indices of the slice of a word that starts at offset `off`:
/// every span at or past `off` shifts by it (`envassign::value_spans`'
/// alignment, without its `=`-byte step).
fn shift_spans(spans: &[usize], off: usize) -> Vec<usize> {
    spans
        .iter()
        .filter(|&&i| i >= off)
        .map(|&i| i - off)
        .collect()
}

#[cfg(test)]
mod tests;
