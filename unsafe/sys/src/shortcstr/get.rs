use alloc::rc::Rc;
use core::slice::SliceIndex;

use crate::shortcstr::{INLINE_MAX, ShortCStr, from_inline, from_long};

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

    pub fn split_once_byte(&self, byte: u8) -> Option<(Self, Self)> {
        let pos = self.as_bytes().iter().position(|&b| b == byte)?;
        Some((self.get(..pos)?, self.get(pos + 1..)?))
    }

    pub fn strip_prefix(&self, prefix: &[u8]) -> Option<Self> {
        if self.as_bytes().starts_with(prefix) {
            self.get(prefix.len()..)
        } else {
            None
        }
    }
}
