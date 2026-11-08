// mutants: -C --test -C shortcstr -- -E 'binary(shortcstr)'

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
            ShortCStr::Arc { length, .. } => *length,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the raw span `(start, end)` this string was cut from is longer
    /// than the string itself, i.e. the span carried bytes the string dropped
    /// (quotes, escapes). An empty quoted region contributes no bytes to the
    /// string, so such a span is the only place the fact can live.
    pub fn is_shorter_than_span(&self, start: usize, end: usize) -> bool {
        end - start > self.len()
    }
}
