use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::shortcstr::{INLINE_CAP, InlineSize, ShortCStr};

impl ShortCStr {
    pub fn new() -> Self {
        // SAFETY: 0 is ≤ INLINE_MAX.
        ShortCStr::Inline {
            len: unsafe { InlineSize::from_u8(0) },
            buf: [0u8; INLINE_CAP],
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ShortCStr::Inline { len, .. } => len.as_u8() as usize,
            ShortCStr::Static(_, _, length) => *length,
            ShortCStr::Rc { length, .. } => *length,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_bytes(&self) -> Result<&[u8], i32> {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                buf.get(..n).ok_or(crate::errno::EINVAL)
            }
            ShortCStr::Static(s, offset, length) => {
                let end = offset + length;
                s.to_bytes().get(*offset..end).ok_or(crate::errno::EINVAL)
            }
            ShortCStr::Rc { rc, offset, length } => {
                let end = offset + length;
                rc.get(*offset..end).ok_or(crate::errno::EINVAL)
            }
        }
    }

    pub(crate) fn as_cstr_bytes(&self) -> Result<&[u8], i32> {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                buf.get(..n).ok_or(crate::errno::EINVAL)
            }
            ShortCStr::Rc { rc, offset, length } => {
                rc.get(*offset..offset + length).ok_or(crate::errno::EINVAL)
            }
            ShortCStr::Static(s, offset, ..) => s
                .to_bytes_with_nul()
                .get(*offset..)
                .ok_or(crate::errno::EINVAL),
        }
    }

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

    pub fn eq_bytes(&self, other: &[u8]) -> bool {
        self.as_bytes().map(|b| b == other).unwrap_or(false)
    }

    pub fn starts_with(&self, prefix: &[u8]) -> bool {
        self.as_bytes()
            .map(|b| b.starts_with(prefix))
            .unwrap_or(false)
    }

    pub fn ends_with(&self, suffix: &[u8]) -> bool {
        self.as_bytes()
            .map(|b| b.ends_with(suffix))
            .unwrap_or(false)
    }

    pub fn contains(&self, byte: u8) -> bool {
        self.as_bytes().map(|b| b.contains(&byte)).unwrap_or(false)
    }
}

impl Default for ShortCStr {
    fn default() -> Self {
        Self::new()
    }
}

/// Copy `src[offset..offset+length]` + `byte` into a new ShortCStr,
/// choosing Inline or Rc based on length.
fn copy_to_shortcstr(src: &[u8], offset: usize, length: usize, byte: u8) -> ShortCStr {
    if length < INLINE_CAP {
        let mut buf = [0u8; INLINE_CAP];
        for (d, b) in buf.iter_mut().zip(
            src.iter()
                .skip(offset)
                .take(length)
                .copied()
                .chain(core::iter::once(byte)),
        ) {
            *d = b;
        }
        // SAFETY: length < INLINE_CAP ≤ INLINE_MAX, so length + 1 ≤ INLINE_MAX
        ShortCStr::Inline {
            len: unsafe { InlineSize::from_u8((length + 1) as u8) },
            buf,
        }
    } else {
        let mut v = Vec::with_capacity(length + 1);
        for &b in src.iter().skip(offset).take(length) {
            v.push(b);
        }
        v.push(byte);
        ShortCStr::Rc {
            rc: Rc::new(v),
            offset: 0,
            length: length + 1,
        }
    }
}
