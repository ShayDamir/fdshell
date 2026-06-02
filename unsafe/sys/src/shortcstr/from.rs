use alloc::ffi::CString;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::ffi::CStr;

use crate::shortcstr::{INLINE_CAP, INLINE_MAX, InlineSize, ShortCStr};

pub(crate) fn from_inline(bytes: &[u8]) -> Result<ShortCStr, i32> {
    if bytes.len() > INLINE_MAX as usize {
        return Err(crate::errno::EINVAL);
    }
    let mut buf = [0u8; INLINE_CAP];
    for (dest, &src) in buf.iter_mut().zip(bytes.iter()) {
        *dest = src;
    }
    *buf.get_mut(bytes.len()).ok_or(crate::errno::EINVAL)? = 0;
    // SAFETY: bytes.len() ≤ INLINE_MAX, checked above.
    let len = unsafe { InlineSize::from_u8(bytes.len() as u8) };
    Ok(ShortCStr::Inline { len, buf })
}

impl ShortCStr {
    pub fn from_vec(bytes: Vec<u8>) -> Result<Self, i32> {
        if bytes.contains(&0) {
            return Err(crate::errno::EINVAL);
        }
        let result = from_inline(&bytes);
        Ok(result.unwrap_or_else(|_| {
            let cs = unsafe { CString::from_vec_unchecked(bytes) };
            let len = cs.count_bytes();
            ShortCStr::Rc {
                rc: Rc::from(cs.into_boxed_c_str()),
                offset: 0,
                length: len,
            }
        }))
    }

    pub const fn from_static(s: &'static CStr) -> Self {
        let len = s.count_bytes();
        ShortCStr::Static(s, 0, len)
    }
}
