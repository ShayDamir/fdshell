use alloc::ffi::CString;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::ffi::CStr;

use crate::shortcstr::{INLINE_CAP, INLINE_MAX, InlineSize, ShortCStr};

/// # Safety
/// `bytes.len()` ≤ `INLINE_MAX` and `bytes` has no interior NUL.
pub(crate) unsafe fn from_inline(bytes: &[u8]) -> ShortCStr {
    let mut buf = [0u8; INLINE_CAP];
    buf[..bytes.len()].copy_from_slice(bytes);
    buf[bytes.len()] = 0;
    // SAFETY: caller guaranteed bytes.len() ≤ INLINE_MAX.
    let len = unsafe { InlineSize::from_u8(bytes.len() as u8) };
    ShortCStr::Inline { len, buf }
}

impl ShortCStr {
    pub fn from_vec(bytes: Vec<u8>) -> Result<Self, i32> {
        if bytes.contains(&0) {
            return Err(crate::errno::EINVAL);
        }
        if bytes.len() <= INLINE_MAX as usize {
            // SAFETY: bytes.len() ≤ INLINE_MAX and no interior NUL, verified above.
            Ok(unsafe { from_inline(&bytes) })
        } else {
            let cs = unsafe { CString::from_vec_unchecked(bytes) };
            let len = cs.count_bytes();
            Ok(ShortCStr::Rc {
                rc: Rc::from(cs.into_boxed_c_str()),
                offset: 0,
                length: len,
            })
        }
    }

    pub const fn from_static(s: &'static CStr) -> Self {
        let len = s.count_bytes();
        ShortCStr::Static(s, 0, len)
    }
}
