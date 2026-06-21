//! `ForkCellError` — typed borrow-failure error for `ForkCell`.
//!
//! Two variants because the blocking borrow may be shared or exclusive, and
//! the caller might want to distinguish them (e.g. for diagnostics, or to
//! decide whether to retry after a short delay vs. abort).

use core::fmt;

/// Borrow operation on a [`ForkCell`](super::ForkCell) failed.
///
/// Variants identify the *conflicting* borrow that is already active.
#[derive(Debug)]
pub enum ForkCellError {
    /// Tried to obtain a shared (`borrow()`) or exclusive (`borrow_mut()`)
    /// reference, but an exclusive (mutable) borrow is already active.
    ExclusiveBorrowActive,
    /// Tried to obtain an exclusive (`borrow_mut()`) reference, but one or
    /// more shared borrows are already active.
    SharedBorrowActive,
}

impl fmt::Display for ForkCellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExclusiveBorrowActive => {
                write!(f, "cannot borrow – exclusive borrow already active")
            }
            Self::SharedBorrowActive => {
                write!(f, "cannot borrow mutably – shared borrows are active")
            }
        }
    }
}

impl core::error::Error for ForkCellError {}
