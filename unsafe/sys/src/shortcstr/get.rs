use alloc::sync::Arc;
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
            ShortCStr::Inline { .. } => None,
            ShortCStr::Static(s, offset, _) => Some(ShortCStr::Static(s, offset + start, new_len)),
            ShortCStr::Arc { arc, offset, .. } => Some(ShortCStr::Arc {
                arc: Arc::clone(arc),
                offset: offset + start,
                length: new_len,
            }),
        }
    }

    pub fn split_once(&self, sep: &[u8]) -> Option<(Self, Self)> {
        let bytes = self.as_bytes().ok()?;
        if sep.is_empty() || sep.len() > bytes.len() {
            return None;
        }
        let pos = bytes.windows(sep.len()).position(|w| w == sep)?;
        let left = self.get(..pos)?;
        let right = self.get(pos + sep.len()..)?;
        Some((left, right))
    }

    pub fn split_once_byte(&self, byte: u8) -> Option<(Self, Self)> {
        self.split_once(&[byte])
    }

    pub fn strip_prefix(&self, prefix: &[u8]) -> Option<Self> {
        if self.as_bytes().ok()?.starts_with(prefix) {
            self.get(prefix.len()..)
        } else {
            None
        }
    }
}
