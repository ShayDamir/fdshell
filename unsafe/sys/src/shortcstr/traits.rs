use alloc::ffi::CString;
use alloc::sync::Arc;
use core::ffi::CStr;
use core::hash::{Hash, Hasher};
use core::num::NonZeroU8;

use crate::shortcstr::ShortCStr;

/// Types whose bytes are guaranteed to contain no NUL (zero) bytes.
///
/// Implementors must ensure all bytes returned by `as_non_zero_bytes` are non-zero.
/// The method body requires `unsafe` (for the repr-transparent cast), which is where
/// the implementor assumes responsibility for the invariant.
pub trait NoNul {
    fn as_non_zero_bytes(&self) -> &[NonZeroU8];
}

impl NoNul for CStr {
    fn as_non_zero_bytes(&self) -> &[NonZeroU8] {
        let bytes = self.to_bytes();
        // SAFETY: CStr cannot contain NUL bytes in its data range by definition.
        unsafe { core::slice::from_raw_parts(bytes.as_ptr() as *const NonZeroU8, bytes.len()) }
    }
}

// If T is NoNul, &T preserves the no-NUL invariant through deref.
impl<T: NoNul + ?Sized> NoNul for &T {
    fn as_non_zero_bytes(&self) -> &[NonZeroU8] {
        (*self).as_non_zero_bytes()
    }
}

// CString cannot contain NUL bytes in its data range by definition.
impl NoNul for CString {
    fn as_non_zero_bytes(&self) -> &[NonZeroU8] {
        let bytes = self.as_bytes();
        // SAFETY: CString cannot contain NUL bytes in its data range by definition.
        unsafe { core::slice::from_raw_parts(bytes.as_ptr() as *const NonZeroU8, bytes.len()) }
    }
}

// ShortCStr's type invariant guarantees no NUL bytes in its data range.
impl NoNul for ShortCStr {
    fn as_non_zero_bytes(&self) -> &[NonZeroU8] {
        // SAFETY: NonZeroU8 is repr(transparent) with u8.
        // ShortCStr's type invariant guarantees no NUL bytes in its data range.
        unsafe {
            let bytes = match self {
                ShortCStr::Inline { len, buf } => {
                    let n = len.as_u8() as usize;
                    // SAFETY: Inline variant always has valid length ≤ buf.len().
                    buf.get(..n).unwrap_or(&[])
                }
                ShortCStr::Static(s, offset, length) => {
                    let end = offset + length;
                    // SAFETY: Static variant always has valid offset/length within s.
                    s.to_bytes().get(*offset..end).unwrap_or(&[])
                }
                ShortCStr::Arc {
                    arc,
                    offset,
                    length,
                } => {
                    let end = offset + length;
                    // SAFETY: Arc variant always has valid offset/length within arc.
                    arc.get(*offset..end).unwrap_or(&[])
                }
            };
            core::slice::from_raw_parts(bytes.as_ptr() as *const NonZeroU8, bytes.len())
        }
    }
}

impl Clone for ShortCStr {
    fn clone(&self) -> Self {
        match self {
            ShortCStr::Inline { len, buf } => ShortCStr::Inline {
                len: *len,
                buf: *buf,
            },
            ShortCStr::Static(s, offset, length) => ShortCStr::Static(s, *offset, *length),
            ShortCStr::Arc {
                arc,
                offset,
                length,
            } => ShortCStr::Arc {
                arc: Arc::clone(arc),
                offset: *offset,
                length: *length,
            },
        }
    }
}

impl PartialEq for ShortCStr {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_bytes(), other.as_bytes()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ShortCStr {}

impl Hash for ShortCStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Ok(b) = self.as_bytes() {
            b.hash(state);
        }
    }
}

impl From<&'static CStr> for ShortCStr {
    fn from(s: &'static CStr) -> Self {
        ShortCStr::Static(s, 0, s.count_bytes())
    }
}
