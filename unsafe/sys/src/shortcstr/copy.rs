use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::shortcstr::{INLINE_CAP, InlineSize, ShortCStr};

/// Copy `src[offset..offset+length]` + `extra` into a new ShortCStr,
/// choosing Inline or Arc based on total length.
pub(crate) fn copy_to_shortcstr(
    src: &[u8],
    offset: usize,
    length: usize,
    extra: &[u8],
) -> ShortCStr {
    let total = length + extra.len();
    if total <= INLINE_CAP {
        let mut buf = [0u8; INLINE_CAP];
        for (d, b) in buf.iter_mut().zip(
            src.iter()
                .skip(offset)
                .take(length)
                .copied()
                .chain(extra.iter().copied()),
        ) {
            *d = b;
        }
        // SAFETY: total ≤ INLINE_CAP ≤ INLINE_MAX
        let len = unsafe { InlineSize::from_u8(total as u8) };
        ShortCStr::Inline { len, buf }
    } else {
        let mut v = Vec::with_capacity(total);
        for &b in src.iter().skip(offset).take(length) {
            v.push(b);
        }
        v.extend_from_slice(extra);
        ShortCStr::Arc {
            arc: Arc::new(v),
            offset: 0,
            length: total,
        }
    }
}
