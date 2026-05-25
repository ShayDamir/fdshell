#![allow(clippy::indexing_slicing)]

use alloc::rc::Rc;
use core::ffi::CStr;

mod access;
mod from;
mod get;
mod size;
mod traits;
mod verify;

pub(crate) use from::{from_inline, from_long};
pub use size::InlineSize;

const INLINE_CAP: usize = 31;
pub(crate) const INLINE_MAX: u8 = 30;

pub enum ShortCStr {
    Inline {
        len: InlineSize,
        buf: [u8; INLINE_CAP],
    },
    Static(&'static CStr, usize, usize),
    Rc {
        rc: Rc<CStr>,
        offset: usize,
        length: usize,
    },
}
