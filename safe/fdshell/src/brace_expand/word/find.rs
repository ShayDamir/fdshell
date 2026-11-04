//! The scan for the first valid brace group of a word (the gobbler's caller).

use super::super::gobbler::{GroupType, gobble};
use super::super::seq::valid_seqterm;

/// The first valid brace group in `text` as `(open, close, type)`. The
/// gobbler reports an `etype` only when the closing `}` was found, so one
/// check covers both.
pub(super) fn find_group(text: &[u8]) -> Option<(usize, usize, GroupType)> {
    let mut i = 0usize;
    loop {
        let (gi, found_open, _) = gobble(text, i, b'{');
        if !found_open {
            return None;
        }
        let (close, t) = match gobble(text, gi + 1, b'}') {
            (close, true, Some(t)) => (close, t),
            _ => {
                i = gi + 1;
                continue;
            }
        };
        if t == GroupType::Seq {
            let amble = text.get(gi + 1..close)?;
            if !valid_seqterm(amble) {
                i = gi + 1;
                continue;
            }
        }
        return Some((gi, close, t));
    }
}
