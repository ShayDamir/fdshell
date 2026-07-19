use alloc::sync::Arc;

use crate::shortcstr::copy::copy_to_shortcstr;
use crate::shortcstr::push_fallback::extend_from_slice_fallback;
use crate::shortcstr::{INLINE_CAP, InlineSize, ShortCStr, ShortCStrError};

impl ShortCStr {
    pub fn push(&mut self, byte: u8) -> Result<(), ShortCStrError> {
        if byte == 0 {
            return Err(ShortCStrError::NulByte);
        }
        // SAFETY: non-NUL checked above.
        unsafe { self.extend_from_slice_unchecked(&[byte]) };
        Ok(())
    }

    pub fn push_str(&mut self, other: &ShortCStr) -> Result<(), ShortCStrError> {
        self.push_slice(other.as_bytes()?)
    }

    pub fn push_slice(&mut self, bytes: &[u8]) -> Result<(), ShortCStrError> {
        if bytes.contains(&0) {
            return Err(ShortCStrError::NulByte);
        }
        // SAFETY: all bytes validated as non-NUL above.
        unsafe { self.extend_from_slice_unchecked(bytes) };
        Ok(())
    }

    /// Append bytes without checking for NUL.
    ///
    /// # Safety
    ///
    /// The caller must ensure no byte is NUL, or intend to seal
    /// the NUL terminator via [`ExportedCStr`] or [`ShortCStr::export`].
    pub unsafe fn extend_from_slice_unchecked(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        let n = self.len();
        let additional = bytes.len();
        let new_len = n + additional;

        // 1. Inline with room — direct memcpy, no copy
        if let ShortCStr::Inline { len, buf } = self
            && new_len <= INLINE_CAP
        {
            // SAFETY: new_len = n + bytes.len() ≤ INLINE_CAP ≤ buf.len()
            unsafe {
                buf.get_unchecked_mut(n..new_len).copy_from_slice(bytes);
            }
            // SAFETY: new_len ≤ INLINE_CAP ≤ INLINE_MAX
            *len = unsafe { InlineSize::from_u8(new_len as u8) };
            return;
        }

        // 2. Static tail slice pushing NUL — already present
        if bytes.len() == 1
            // SAFETY: bytes is non-empty, checked above.
            && unsafe { *bytes.get_unchecked(0) } == 0
            && let ShortCStr::Static(s, offset, length) = self
            && *offset + *length == s.count_bytes()
        {
            return;
        }

        // 3. Short data — copy into Inline
        if new_len <= INLINE_CAP {
            let (src, offset, length) = match self {
                ShortCStr::Arc {
                    arc,
                    offset,
                    length,
                } => {
                    let s: &[u8] = arc;
                    (s, *offset, *length)
                }
                ShortCStr::Static(s, offset, length) => (s.to_bytes(), *offset, *length),
                _ => unreachable!(),
            };
            *self = copy_to_shortcstr(src, offset, length, bytes);
            return;
        }

        // 4. Arc tail view — in-place growth
        if let ShortCStr::Arc {
            arc,
            offset,
            length,
        } = self
            && *offset + *length == arc.len()
        {
            Arc::make_mut(arc).extend_from_slice(bytes);
            *length += additional;
            return;
        }

        // 5. Everything else — allocate Arc
        extend_from_slice_fallback(self, bytes);
    }
}
