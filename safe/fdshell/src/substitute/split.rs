use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;

/// Split a word on the shell's IFS characters.
///
/// Runs of IFS whitespace collapse to one delimiter and trim the ends;
/// each non-whitespace IFS char delimits a field, empty fields included
/// (a trailing one is kept). An empty IFS disables splitting. An IFS char
/// whose mask bit is `true` was inside double quotes and never delimits.
pub(crate) fn split_word(
    word: &ShortCStr,
    mask: &[bool],
    ifs: &ShortCStr,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let bytes = word.as_bytes().change_context(ResolveError::RefNotFound)?;
    let ifs_bytes = ifs.as_bytes().change_context(ResolveError::RefNotFound)?;
    let is_ifs_ws = |b: u8| (b == b' ' || b == b'\t' || b == b'\n') && ifs_bytes.contains(&b);
    let is_ifs = |b: u8| ifs_bytes.contains(&b);
    let quoted = |i: usize| mask.get(i).copied().unwrap_or(false);
    let mut fields = Vec::new();
    let mut cur = ShortCStr::new();
    let mut trailing = false;
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        if is_ifs_ws(b) && !quoted(i) {
            if !cur.is_empty() {
                fields.push(core::mem::take(&mut cur));
            }
            trailing = false;
            while let Some(&n) = bytes.get(i + 1) {
                if !is_ifs_ws(n) || quoted(i + 1) {
                    break;
                }
                i += 1;
            }
        } else if is_ifs(b) && !quoted(i) {
            fields.push(core::mem::take(&mut cur));
            trailing = true;
        } else {
            cur.push_byte(b).change_context(ResolveError::NulByte)?;
            trailing = false;
        }
        i += 1;
    }
    if !cur.is_empty() || trailing {
        fields.push(cur);
    }
    Ok(fields)
}

#[cfg(test)]
mod tests;
