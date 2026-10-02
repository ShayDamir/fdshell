//! The parsed `verity` arguments: the target fd variable and the
//! enable/algo/digest options.

use alloc::vec::Vec;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub(super) struct VerityConfig {
    pub(super) var: ShortCStr,
    pub(super) enable: bool,
    pub(super) algo: u32,
    pub(super) expected: Option<Vec<u8>>,
}
