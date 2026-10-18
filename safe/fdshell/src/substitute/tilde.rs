//! Leading-`~` expansion — `$HOME` when a tilde starts a word.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::mask::{push_byte, push_expanded};
use crate::error::resolve::ResolveError;

/// Whether the word opens a tilde expansion: a leading `~` whose tilde-prefix
/// — the word's bytes from `~` through the first `/` **inclusive** (the whole
/// word when it has no `/`) — carries **no quoted byte**, the `~`'s own bit
/// included. POSIX #2.6.1 expands a tilde only when no quoted character
/// appears in the tilde-prefix, and it expands *before* quote removal, so
/// task #269's quoted `~` (`"~"`, `"~/x"`, and the all-quoted here-doc body
/// `~`) is literal and so is every quoted byte *inside* the prefix — task
/// #281, measured in bash 5.3.9 with `HOME=/h/o/m/e`: `~"/*"` → `~/*`,
/// `~"/*"/sub` → `~/*/sub`, `~"/x"` and `~'/'x` → `~/x` (the terminating `/`'s
/// own bit counts), while a quote **after** the `/` leaves the prefix unquoted
/// and the tilde expands (`~/"x"` → `$HOME/x`). A mask shorter than the word
/// pads as unquoted, like `arg.rs`. The rule lives here, not in `arg.rs`,
/// which is at its line budget (LESSONS #269).
pub(super) fn leads(bytes: &[u8], mask: &[bool]) -> bool {
    bytes.first() == Some(&b'~') && !prefix_quoted(bytes, mask)
}

/// `true` when any byte of the tilde-prefix was quoted: the mask bits from
/// index 0 (the `~` itself, task #269) through the first `/` **inclusive**. A
/// bare `~` scans indices 0..=1, where index 1 pads to `false` — it must keep
/// expanding. Backslashes are deliberately not consulted: an unquoted
/// `\<char>` carries no mask bit and is protected by `escape::unquoted` at
/// push time, and the arm only fires when the byte after `~` is `/` or the end
/// of the word, so `~\/x` → `~/x` and `~\*` → `~*` agree with bash by
/// construction (POSIX counts `\X` as a quoted character; unobservable here
/// because fdshell expands no `~user`).
fn prefix_quoted(bytes: &[u8], mask: &[bool]) -> bool {
    let end = bytes.iter().position(|&b| b == b'/').unwrap_or(bytes.len());
    (0..=end).any(|i| mask.get(i).copied().unwrap_or(false))
}

/// Expand the leading `~` already at the front of `peek`. `~` alone or
/// `~/…` becomes `$HOME` (nothing when unset); any other `~x` keeps the
/// tilde. Consumed input and appended output keep the quote mask aligned.
/// The pushed `$HOME` bytes are deliberately **not** recorded as a
/// provenance span: bash never IFS-splits tilde output
/// (`IFS=,; HOME="a,b"; printf %s ~` → `[a,b]`), so they stay literal.
pub(super) fn expand(
    peek: &mut super::mask::Counting<'_>,
    idx: &mut usize,
    mask: &[bool],
    out: &mut ShortCStr,
    out_mask: &mut Vec<bool>,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    *idx += 1;
    let home_q = mask.first().copied().unwrap_or(false);
    match peek.peek() {
        None | Some(&b'/') => {
            if let Some(home) = sys::env::getenv(c"HOME") {
                let bytes = home.as_bytes().change_context(ResolveError::RefNotFound)?;
                push_expanded(out, out_mask, bytes, home_q)?;
            }
        }
        _ => push_byte(out, out_mask, b'~', home_q)?,
    }
    Ok(())
}
