mod escape;
mod mask;
mod single;
mod state;
mod statement;
mod step;

pub(super) use state::State;
pub(crate) use statement::tokenize_statement;

use super::Token;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::Report;

pub fn tokenize(line: &[u8]) -> Result<Vec<Token>, Report<ParseError>> {
    let mut st = State::new();
    let mut bytes = line.iter().copied().peekable();
    while let Some(b) = bytes.next() {
        st.pos += 1;
        if st.in_single {
            if !st.single_char(b)? {
                st.in_single = false;
                st.single_start = None;
                st.close_span();
            }
        } else if st.in_quotes {
            if !st.quoted_char(b, &mut bytes, line)? {
                st.in_quotes = false;
                st.quote_start = None;
                st.close_span();
            }
        } else {
            st.unquoted(b, line, &mut bytes)?;
        }
    }
    st.finish(line)
}

#[cfg(test)]
mod tests;
