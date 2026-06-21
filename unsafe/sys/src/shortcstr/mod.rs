use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::CStr;

mod access;
mod copy;
mod error;
mod from;
mod get;
mod push;
mod size;
mod traits;

pub use error::ShortCStrError;
pub(crate) use from::from_inline;
pub use size::InlineSize;

pub(crate) const INLINE_MAX: u8 = 30;
const INLINE_CAP: usize = INLINE_MAX as usize;

pub enum ShortCStr {
    Inline {
        len: InlineSize,
        buf: [u8; INLINE_CAP],
    },
    Static(&'static CStr, usize, usize),
    Arc {
        arc: Arc<Vec<u8>>,
        offset: usize,
        length: usize,
    },
}

/// A sealed C-string view of a [`ShortCStr`].
///
/// Ensures a NUL terminator at the end of the subslice via
/// [`push_unchecked`], enabling zero-copy [`AsRef<CStr>`].
pub struct RefCStr(ShortCStr);

impl From<ShortCStr> for RefCStr {
    fn from(mut value: ShortCStr) -> Self {
        // SAFETY: push_unchecked(0) seals the NUL terminator.
        // Rule 2 handles tail-slice Static as a no-op.
        unsafe { value.push_unchecked(0) };
        RefCStr(value)
    }
}

impl AsRef<CStr> for RefCStr {
    fn as_ref(&self) -> &CStr {
        // as_cstr_bytes() always returns Ok for a RefCStr because
        // RefCStr::from guarantees push_unchecked(0) was called (or the
        // Static variant already has a NUL terminator), and all
        // offsets/lengths are validated at construction.
        let bytes = match self.0.as_cstr_bytes() {
            Ok(b) => b,
            Err(_) => {
                // SAFETY: The Err branch is unreachable — the invariants
                // described above guarantee as_cstr_bytes() returns Ok.
                unsafe { core::hint::unreachable_unchecked() }
            }
        };
        // SAFETY: RefCStr::from guarantees NUL at end of the slice.
        unsafe { CStr::from_bytes_with_nul_unchecked(bytes) }
    }
}

impl core::ops::Deref for RefCStr {
    type Target = CStr;
    fn deref(&self) -> &CStr {
        self.as_ref()
    }
}
