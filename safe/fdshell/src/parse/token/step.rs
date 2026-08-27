use alloc::vec::Vec;

use super::super::{
    backtick::read_backtick, comment::skip_comment, emit::emit_token, token_pipe::handle_pipe,
    token_subst::read_dollar_paren,
};
use super::State;
use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};

/// Handle one unquoted byte of the line, updating the token state.
pub(super) fn unquoted(
    b: u8,
    st: &mut State,
    line: &[u8],
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<(), Report<ParseError>> {
    match b {
        b' ' | b'\t' | b';' | b'\n' | b')' => {
            let needs_sep = b == b';' || b == b'\n' || b == b')';
            let sep = if b == b')' { c")" } else { c";" };
            emit_token(
                &mut st.tokens,
                &mut st.cur,
                st.start,
                st.pos - 1,
                st.fq,
                &mut st.mask,
            );
            st.fq = false;
            st.word_started = false;
            st.start = st.pos;
            if needs_sep {
                st.tokens
                    .push((sep.into(), st.pos - 1, st.pos, false, Vec::new()));
            }
        }
        b'|' => {
            if handle_pipe(
                &mut st.tokens,
                &mut st.cur,
                st.start,
                st.fq,
                st.pos,
                &mut st.mask,
            )? {
                st.fq = false;
                st.word_started = false;
            }
        }
        b'"' => {
            if st.cur.is_empty() {
                st.fq = true;
            }
            st.in_quotes = true;
            st.word_started = true;
            st.quote_start = Some(st.pos - 1);
        }
        b'$' if bytes.peek() == Some(&b'(') => {
            st.fq = false;
            st.word_started = true;
            read_dollar_paren(line, &mut st.cur, &mut st.mask, bytes, &mut st.pos)?;
        }
        b'`' => {
            st.fq = false;
            st.word_started = true;
            read_backtick(line, &mut st.cur, &mut st.mask, bytes, &mut st.pos)?;
        }
        // `#` starts a comment only at the beginning of a word; inside a
        // word it is a literal byte (colors, URLs, `${#var}`, …).
        b'#' if !st.word_started => {
            // No token has accumulated yet, so there is nothing to emit.
            let consumed = skip_comment(bytes);
            st.pos += consumed - 1;
            st.fq = false;
            st.word_started = false;
            st.start = st.pos;
        }
        _ => {
            st.fq = false;
            st.word_started = true;
            st.cur
                .push_byte(b)
                .change_context(ParseError::InvalidChar { ch: 0 })?;
            st.mask.push(false);
        }
    }
    Ok(())
}
