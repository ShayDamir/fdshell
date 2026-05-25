use alloc::rc::Rc;
use core::slice::SliceIndex;

use crate::shortcstr::{from_inline, from_long, ShortCStr, INLINE_MAX};

impl ShortCStr {
    pub fn get<I>(&self, index: I) -> Option<Self>
    where
        I: SliceIndex<[u8], Output = [u8]>,
    {
        let orig = self.as_bytes();
        let new = orig.get(index)?;
        let new_len = new.len();

        if new_len <= INLINE_MAX as usize {
            // SAFETY: new_len and no interior NUL guaranteed by origin.
            return Some(unsafe { from_inline(new) });
        }

        let start = new.as_ptr() as usize - orig.as_ptr() as usize;
        if start + new_len == self.len() {
            // Tail slice — preserve variant, adjust offset.
            match self {
                ShortCStr::Inline { .. } => unreachable!(),
                ShortCStr::Static(s, offset, _) => {
                    Some(ShortCStr::Static(s, offset + start, new_len))
                }
                ShortCStr::Rc { rc, offset, .. } => Some(ShortCStr::Rc {
                    rc: Rc::clone(rc),
                    offset: offset + start,
                    length: new_len,
                }),
            }
        } else {
            // SAFETY: new_len and no interior NUL guaranteed by origin.
            Some(unsafe { from_long(new) })
        }
    }
}
