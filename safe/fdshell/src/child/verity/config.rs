//! The parsed `verity` arguments: the target fd variable and the
//! enable/algo/digest/dump options.

use alloc::vec::Vec;
use sys::ShortCStr;

use super::dump::DumpSpec;

#[cfg_attr(test, derive(Debug))]
pub(super) struct VerityConfig {
    pub(super) var: ShortCStr,
    pub(super) enable: bool,
    pub(super) algo: u32,
    pub(super) expected: Option<Vec<u8>>,
    pub(super) dump: Option<DumpSpec>,
}
