use alloc::vec::Vec;
use error_stack::{Report, bail};

use crate::error::resolve::ResolveError;
use crate::paren_scan::scan_dollar_paren_body;

pub fn read_paren_expr(
    peek: &mut super::mask::Counting<'_>,
) -> Result<Vec<u8>, Report<ResolveError>> {
    let Some(body) = scan_dollar_paren_body(peek) else {
        bail!(ResolveError::UnclosedParen);
    };
    Ok(body)
}
