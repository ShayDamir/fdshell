use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::shortcstr::copy::copy_to_shortcstr;
use crate::shortcstr::{INLINE_CAP, InlineSize, ShortCStr};

impl ShortCStr {
    pub fn push(&mut self, byte: u8) -> Result<(), i32> {
        if byte == 0 {
            return Err(crate::errno::EINVAL);
        }
        // SAFETY: non-NUL checked above.
        unsafe { self.push_unchecked(byte) };
        Ok(())
    }

    /// Push a byte without checking for NUL.
    ///
    /// # Safety
    ///
    /// The caller must ensure the byte is not NUL, or intend to
    /// seal the NUL terminator via [`RefCStr`].
    pub unsafe fn push_unchecked(&mut self, byte: u8) {
        let n = self.len();

        // 1. Inline with room — direct write, no copy
        if let ShortCStr::Inline { len, buf } = self
            && n < INLINE_CAP
        {
            // SAFETY: n < INLINE_CAP ≤ buf.len()
            *unsafe { buf.get_unchecked_mut(n) } = byte;
            // SAFETY: n + 1 ≤ INLINE_MAX
            *len = unsafe { InlineSize::from_u8((n + 1) as u8) };
            return;
        }

        // 2. Static tail slice pushing NUL — already present
        if byte == 0
            && let ShortCStr::Static(s, offset, length) = self
            && *offset + *length == s.count_bytes()
        {
            return;
        }

        // 3. Short data — copy into Inline
        if n < INLINE_CAP {
            let (src, offset, length) = match self {
                ShortCStr::Rc { rc, offset, length } => {
                    let s: &[u8] = rc;
                    (s, *offset, *length)
                }
                ShortCStr::Static(s, offset, length) => (s.to_bytes(), *offset, *length),
                _ => unreachable!(),
            };
            *self = copy_to_shortcstr(src, offset, length, byte);
            return;
        }

        // 4. Rc tail view — in-place growth
        if let ShortCStr::Rc { rc, offset, length } = self
            && *offset + *length == rc.len()
        {
            Rc::make_mut(rc).push(byte);
            *length += 1;
            return;
        }

        // 5. Everything else — allocate Rc
        match self {
            ShortCStr::Inline { buf, .. } => {
                let mut v = Vec::with_capacity(INLINE_CAP * 2);
                for &b in buf.iter() {
                    v.push(b);
                }
                v.push(byte);
                *self = ShortCStr::Rc {
                    rc: Rc::new(v),
                    offset: 0,
                    length: INLINE_CAP + 1,
                };
            }
            ShortCStr::Rc { rc, offset, length } => {
                let src: &[u8] = rc;
                let o = *offset;
                let l = *length;
                *self = copy_to_shortcstr(src, o, l, byte);
            }
            ShortCStr::Static(s, offset, length) => {
                let src = s.to_bytes();
                let o = *offset;
                let l = *length;
                *self = copy_to_shortcstr(src, o, l, byte);
            }
        }
    }
}
