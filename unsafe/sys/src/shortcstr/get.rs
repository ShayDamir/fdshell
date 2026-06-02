use alloc::rc::Rc;
use core::slice::SliceIndex;

use crate::shortcstr::{INLINE_MAX, ShortCStr, from_inline};

impl ShortCStr {
    pub fn get<I>(&self, index: I) -> Option<Self>
    where
        I: SliceIndex<[u8], Output = [u8]>,
    {
        let orig = self.as_bytes().ok()?;
        let new = orig.get(index)?;
        let new_len = new.len();

        if new_len <= INLINE_MAX as usize {
            return from_inline(new).ok();
        }

        // SAFETY: `new` is a subslice of `orig` — `get()` returned `Some`,
        // so both point into the same allocation and `new` ≥ `orig`.
        let start = unsafe { new.as_ptr().offset_from(orig.as_ptr()) as usize };
        match self {
            ShortCStr::Inline { .. } => unreachable!(),
            ShortCStr::Static(s, offset, _) => Some(ShortCStr::Static(s, offset + start, new_len)),
            ShortCStr::Rc { rc, offset, .. } => Some(ShortCStr::Rc {
                rc: Rc::clone(rc),
                offset: offset + start,
                length: new_len,
            }),
        }
    }

    pub fn split_once_byte(&self, byte: u8) -> Option<(Self, Self)> {
        let pos = self.as_bytes().ok()?.iter().position(|&b| b == byte)?;
        Some((self.get(..pos)?, self.get(pos + 1..)?))
    }

    pub fn strip_prefix(&self, prefix: &[u8]) -> Option<Self> {
        if self.as_bytes().ok()?.starts_with(prefix) {
            self.get(prefix.len()..)
        } else {
            None
        }
    }
}
