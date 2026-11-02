//! Rendering one element of a sequence: bash's `itos` plus ST_ZINT zero-padding.

use alloc::vec;
use alloc::vec::Vec;

use crate::brace_expand::seq::{SeqKind, SeqSpec};

impl SeqSpec {
    /// Render one element (bash's `itos` plus ST_ZINT zero-padding).
    pub(super) fn render(&self, n: i64) -> Vec<u8> {
        match self.kind {
            SeqKind::Char => vec![n as u8],
            SeqKind::Int => alloc::format!("{n}").into_bytes(),
            SeqKind::ZInt => render_zint(n, self.width),
        }
    }
}

/// Zero-padded integer (bash's ST_ZINT): pad with `0` to `width`, keeping a
/// leading `-` for negatives.
fn render_zint(n: i64, width: usize) -> Vec<u8> {
    let text = alloc::format!("{n}");
    if text.len() >= width {
        return text.into_bytes();
    }
    let mut out: Vec<u8> = Vec::with_capacity(width);
    if n < 0 {
        out.push(b'-');
        let digits = alloc::format!("{}", n.unsigned_abs());
        out.extend(core::iter::repeat_n(b'0', width - out.len() - digits.len()));
        out.extend_from_slice(digits.as_bytes());
    } else {
        out.extend(core::iter::repeat_n(b'0', width - text.len()));
        out.extend_from_slice(text.as_bytes());
    }
    out
}
