use crate::errno::EINVAL;
use crate::shortcstr::{INLINE_CAP, InlineSize, ShortCStr};

impl ShortCStr {
    pub fn new() -> Self {
        // SAFETY: 0 is ≤ INLINE_MAX.
        let len = unsafe { InlineSize::from_u8(0) };
        ShortCStr::Inline {
            len,
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
                buf.get(..n).ok_or(EINVAL)
            }
            ShortCStr::Static(s, offset, length) => {
                let end = offset + length;
                s.to_bytes().get(*offset..end).ok_or(EINVAL)
            }
            ShortCStr::Rc { rc, offset, length } => {
                let end = offset + length;
                rc.get(*offset..end).ok_or(EINVAL)
            }
        }
    }

    pub(crate) fn as_cstr_bytes(&self) -> Result<&[u8], i32> {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                buf.get(..n).ok_or(EINVAL)
            }
            ShortCStr::Rc { rc, offset, length } => rc.get(*offset..offset + length).ok_or(EINVAL),
            ShortCStr::Static(s, offset, ..) => s.to_bytes_with_nul().get(*offset..).ok_or(EINVAL),
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
