//! The `\` arms of the substitution loop: POSIX 2.2.1 (outside quotes) and
//! 2.2.2 (inside quotes). Both keep `idx` synced to the bytes they consume
//! from `peek` so the loop's mask indexing stays aligned.

use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::substitute::mask::{Counting, push_byte};

/// Unquoted POSIX #4.1: `\X` removes the backslash and the escaped byte
/// becomes protected (mask `true` — it never splits or globs). `\<newline>`
/// is a line continuation: both bytes are dropped. A trailing `\` at end of
/// word keeps the backslash.
pub(super) fn unquoted(
    peek: &mut Counting<'_>,
    idx: &mut usize,
    out: &mut ShortCStr,
    mask: &mut Vec<bool>,
) -> Result<(), Report<ResolveError>> {
    match peek.next() {
        Some(b'\n') => *idx += 1,
        Some(c) => {
            *idx += 1;
            push_byte(out, mask, c, true)?;
        }
        None => push_byte(out, mask, b'\\', false)?,
    }
    Ok(())
}

/// Quoted POSIX #4.2: `\$`, `\\` and `` \` `` drop the backslash; any other
/// `\<char>` keeps it. The backtick pair is the encoding of a literal
/// backtick (single-quote spans and `"\`"`), so the backtick arm never sees
/// a byte that is not a region delimiter.
pub(super) fn quoted(
    peek: &mut Counting<'_>,
    idx: &mut usize,
    out: &mut ShortCStr,
    mask: &mut Vec<bool>,
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    match peek.peek() {
        Some(&c @ (b'$' | b'\\' | b'`')) => {
            peek.next();
            *idx += 1;
            push_byte(out, mask, c, quoted)?;
        }
        _ => push_byte(out, mask, b'\\', quoted)?,
    }
    Ok(())
}
