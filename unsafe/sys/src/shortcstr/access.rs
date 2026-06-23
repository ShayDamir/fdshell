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
