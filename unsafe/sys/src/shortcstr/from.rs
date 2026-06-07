use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::shortcstr::{INLINE_CAP, INLINE_MAX, InlineSize, ShortCStr};

pub(crate) fn from_inline(bytes: &[u8]) -> Result<ShortCStr, i32> {
    if bytes.len() > INLINE_MAX as usize {
        return Err(crate::errno::EINVAL);
    }
    let mut buf = [0u8; INLINE_CAP];
    for (dest, &src) in buf.iter_mut().zip(bytes.iter()) {
        *dest = src;
    }
    // SAFETY: bytes.len() ≤ INLINE_MAX, checked above.
    let len = unsafe { InlineSize::from_u8(bytes.len() as u8) };
    Ok(ShortCStr::Inline { len, buf })
}

impl ShortCStr {
    pub fn from_vec(bytes: Vec<u8>) -> Result<Self, i32> {
        if bytes.contains(&0) {
            return Err(crate::errno::EINVAL);
        }
        let length = bytes.len();
        Ok(from_inline(&bytes).unwrap_or_else(|_| ShortCStr::Rc {
            rc: Rc::new(bytes),
            offset: 0,
            length,
        }))
    }
}
