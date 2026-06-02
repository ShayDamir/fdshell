use alloc::rc::Rc;

mod access;
mod from;
mod get;
mod size;
mod traits;

pub(crate) use from::from_inline;
pub use size::InlineSize;

pub(crate) const INLINE_MAX: u8 = 30;
const INLINE_CAP: usize = INLINE_MAX as usize;

pub enum ShortCStr {
    Inline {
        len: InlineSize,
        buf: [u8; INLINE_CAP],
    },
    Static(&'static [u8], usize, usize),
    Rc {
        rc: Rc<[u8]>,
        offset: usize,
        length: usize,
    },
}
