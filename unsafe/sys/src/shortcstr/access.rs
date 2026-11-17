// mutants: -C --test -C shortcstr -- -E 'binary(shortcstr)'

use crate::shortcstr::{ShortCStr, ShortCStrError};

impl ShortCStr {
    pub fn as_bytes(&self) -> Result<&[u8], ShortCStrError> {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                buf.get(..n).ok_or(ShortCStrError::BadState)
            }
            ShortCStr::Static(s, offset, length) => {
                let end = offset + length;
                s.to_bytes()
                    .get(*offset..end)
                    .ok_or(ShortCStrError::BadState)
            }
            ShortCStr::Arc {
                arc,
                offset,
                length,
            } => {
                let end = offset + length;
                arc.get(*offset..end).ok_or(ShortCStrError::BadState)
            }
        }
    }

    /// The byte at `offset`, `None` past the end — the single-byte answer of
    /// `slice::get`, and the documented answer for a `BadState` value is
    /// `None` (STYLE §6.4: byte reads go through the type, not `as_bytes()`).
    pub fn get_byte(&self, offset: usize) -> Option<u8> {
        self.as_bytes().ok()?.get(offset).copied()
    }

    /// The first byte, `None` for an empty string — `slice::first`'s answer,
    /// and the documented `BadState` answer is `None`.
    pub fn first_byte(&self) -> Option<u8> {
        self.get_byte(0)
    }

    pub(crate) fn as_cstr_bytes(&self) -> Result<&[u8], ShortCStrError> {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                buf.get(..n).ok_or(ShortCStrError::BadState)
            }
            ShortCStr::Arc {
                arc,
                offset,
                length,
            } => arc
                .get(*offset..offset + length)
                .ok_or(ShortCStrError::BadState),
            ShortCStr::Static(s, offset, ..) => s
                .to_bytes_with_nul()
                .get(*offset..)
                .ok_or(ShortCStrError::BadState),
        }
    }
}

impl Default for ShortCStr {
    fn default() -> Self {
        Self::new()
    }
}
