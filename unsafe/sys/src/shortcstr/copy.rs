use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::shortcstr::{INLINE_CAP, InlineSize, ShortCStr};

/// Copy `src[offset..offset+length]` + `byte` into a new ShortCStr,
/// choosing Inline or Arc based on length.
pub(crate) fn copy_to_shortcstr(src: &[u8], offset: usize, length: usize, byte: u8) -> ShortCStr {
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
        let len = unsafe { InlineSize::from_u8((length + 1) as u8) };
        ShortCStr::Inline { len, buf }
    } else {
        let mut v = Vec::with_capacity(length + 1);
        for &b in src.iter().skip(offset).take(length) {
            v.push(b);
        }
        v.push(byte);
        ShortCStr::Arc {
            arc: Arc::new(v),
            offset: 0,
            length: length + 1,
        }
    }
}
