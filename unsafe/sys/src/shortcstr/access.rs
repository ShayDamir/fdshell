#![allow(clippy::indexing_slicing)]

use alloc::ffi::CString;
use core::ffi::CStr;

use crate::shortcstr::ShortCStr;

impl ShortCStr {
    pub fn len(&self) -> usize {
        match self {
            ShortCStr::Inline { len, .. } => len.as_u8() as usize,
            ShortCStr::Static(_, _, length) => *length,
            ShortCStr::Rc { length, .. } => *length,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                // SAFETY: n ≤ INLINE_MAX < INLINE_CAP, set during construction.
                &buf[..n]
            }
            ShortCStr::Static(s, offset, length) => {
                let full = s.to_bytes();
                // SAFETY: offset + length ≤ full.len(), set during construction/subslicing.
                &full[*offset..offset + length]
            }
            ShortCStr::Rc { rc, offset, length } => {
                let full = rc.to_bytes();
                // SAFETY: offset + length ≤ full.len(), set during construction/subslicing.
                &full[*offset..offset + length]
            }
        }
    }

    pub fn as_c_str(&self) -> &CStr {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                // SAFETY: buf[n] is NUL by construction. No interior NUL.
                unsafe { CStr::from_bytes_with_nul_unchecked(&buf[..=n]) }
            }
            ShortCStr::Static(s, offset, length) => {
                let full = s.to_bytes_with_nul();
                // SAFETY: offset + length + 1 ≤ full.len(), NUL at full[offset + length].
                unsafe { CStr::from_bytes_with_nul_unchecked(&full[*offset..offset + length + 1]) }
            }
            ShortCStr::Rc { rc, offset, length } => {
                let full = rc.to_bytes_with_nul();
                // SAFETY: same as Static.
                unsafe { CStr::from_bytes_with_nul_unchecked(&full[*offset..offset + length + 1]) }
            }
        }
    }

    pub fn to_c_string(&self) -> CString {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                // SAFETY: buf[..n] has no interior NUL (validated in from_bytes).
                unsafe { CString::from_vec_unchecked(buf[..n].to_vec()) }
            }
            ShortCStr::Static(s, offset, length) => {
                // SAFETY: subslice of a CStr, no interior NUL.
                unsafe {
                    CString::from_vec_unchecked(s.to_bytes()[*offset..offset + length].to_vec())
                }
            }
            ShortCStr::Rc { rc, offset, length } => {
                // SAFETY: subslice of a CStr, no interior NUL.
                unsafe {
                    CString::from_vec_unchecked(rc.to_bytes()[*offset..offset + length].to_vec())
                }
            }
        }
    }
}
