use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::shortcstr::copy::copy_to_shortcstr;
use crate::shortcstr::{INLINE_CAP, ShortCStr};

pub(crate) fn extend_from_slice_fallback(this: &mut ShortCStr, bytes: &[u8]) {
    match this {
        ShortCStr::Inline { buf, len } => {
            let mut v = Vec::with_capacity(INLINE_CAP * 2);
            let len = len.as_u8() as usize;
            for &b in buf.iter().take(len) {
                v.push(b);
            }
            v.extend_from_slice(bytes);
            *this = ShortCStr::Arc {
                arc: Arc::new(v),
                offset: 0,
                length: len + bytes.len(),
            };
        }
        ShortCStr::Arc {
            arc,
            offset,
            length,
        } => {
            let src: &[u8] = arc;
            let o = *offset;
            let l = *length;
            *this = copy_to_shortcstr(src, o, l, bytes);
        }
        ShortCStr::Static(s, offset, length) => {
            let src = s.to_bytes();
            let o = *offset;
            let l = *length;
            *this = copy_to_shortcstr(src, o, l, bytes);
        }
    }
}
