use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::shortcstr::copy::copy_to_shortcstr;
use crate::shortcstr::{INLINE_CAP, ShortCStr};

pub(crate) fn push_fallback(this: &mut ShortCStr, byte: u8) {
    match this {
        ShortCStr::Inline { buf, .. } => {
            let mut v = Vec::with_capacity(INLINE_CAP * 2);
            for &b in buf.iter() {
                v.push(b);
            }
            v.push(byte);
            *this = ShortCStr::Arc {
                arc: Arc::new(v),
                offset: 0,
                length: INLINE_CAP + 1,
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
            *this = copy_to_shortcstr(src, o, l, byte);
        }
        ShortCStr::Static(s, offset, length) => {
            let src = s.to_bytes();
            let o = *offset;
            let l = *length;
            *this = copy_to_shortcstr(src, o, l, byte);
        }
    }
}
