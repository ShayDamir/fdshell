use alloc::ffi::CString;

use crate::shortcstr::ShortCStr;

impl ShortCStr {
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
                let full = s.to_bytes();
                full.get(*offset..offset + length)
                    .ok_or(crate::errno::EINVAL)
            }
            ShortCStr::Rc { rc, offset, length } => {
                let full = rc.to_bytes();
                full.get(*offset..offset + length)
                    .ok_or(crate::errno::EINVAL)
            }
        }
    }

    pub fn to_c_string(&self) -> Result<CString, i32> {
        CString::new(self.as_bytes()?.to_vec()).map_err(|_| crate::errno::EINVAL)
    }

    pub fn eq_bytes(&self, other: &[u8]) -> bool {
        self.as_bytes().map(|b| b == other).unwrap_or(false)
    }

    pub fn starts_with(&self, prefix: &[u8]) -> bool {
        self.as_bytes()
            .map(|b| b.starts_with(prefix))
            .unwrap_or(false)
    }

    pub fn contains(&self, byte: u8) -> bool {
        self.as_bytes().map(|b| b.contains(&byte)).unwrap_or(false)
    }
}
