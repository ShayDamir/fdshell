use error_stack::Report;

use crate::error::resolve::ResolveError;

pub fn read_paren_expr(
    peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<Vec<u8>, Report<ResolveError>> {
    let mut inner = Vec::new();
    let mut depth = 1u32;
    while depth > 0 {
        match peek.peek().copied() {
            Some(b'(') => {
                inner.push(b'(');
                depth += 1;
                peek.next();
            }
            Some(b')') => {
                depth -= 1;
                if depth == 0 {
                    peek.next();
                    break;
                }
                inner.push(b')');
                peek.next();
            }
            Some(c) => {
                inner.push(c);
                peek.next();
            }
            None => return Err(Report::new(ResolveError::UnclosedParen)),
        }
    }
    Ok(inner)
}
