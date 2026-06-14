use core::cell::{Cell, UnsafeCell};
use core::marker::PhantomData;

mod ref_mut;
#[cfg(test)]
mod tests;

pub use self::ref_mut::{Ref, RefMut};

use crate::errno::EINVAL;

/// Interior mutability with fork-aware borrow tracking.
///
/// Like `RefCell`, but all borrows are **fallible** (`Result`) rather than
/// panicking — callers can detect and handle borrowing clashes gracefully.
///
/// # Fork isolation
///
/// After `fork(2)`, the child has its own independent copy of the address
/// space. Borrows held in the parent are meaningless in the child — the
/// inherited borrow count is stale and must be reset via
/// [`reset_after_fork`](Self::reset_after_fork) before the child can borrow.
/// Mutations in the child never propagate to the parent.
pub struct ForkCell<T> {
    /// Borrow tracking: 0 = free, >0 = shared borrows, <0 = exclusive mutable borrow.
    count: Cell<isize>,
    value: UnsafeCell<T>,
    /// Suppresses auto-derived `Send` — single-threaded interior mutability (like `RefCell`).
    _nosend: PhantomData<*const ()>,
}

impl<T> ForkCell<T> {
    /// Create a new `ForkCell` wrapping `val`.
    pub const fn new(val: T) -> Self {
        ForkCell {
            count: Cell::new(0),
            value: UnsafeCell::new(val),
            _nosend: PhantomData,
        }
    }

    /// Shared borrow. Returns `Err(EINVAL)` if already mutably borrowed.
    pub fn borrow(&self) -> Result<Ref<'_, T>, i32> {
        let cur = self.count.get();
        if cur < 0 {
            return Err(EINVAL);
        }
        self.count.set(cur + 1);
        Ok(Ref { cell: self })
    }

    /// Exclusive borrow. Returns `Err(EINVAL)` if already borrowed (shared or exclusive).
    pub fn borrow_mut(&self) -> Result<RefMut<'_, T>, i32> {
        let cur = self.count.get();
        if cur != 0 {
            return Err(EINVAL);
        }
        self.count.set(-1);
        Ok(RefMut { cell: self })
    }

    /// Exclusive access when `&mut self` guarantees there's only one reference.
    pub fn get_mut(&mut self) -> &mut T {
        // SAFETY: &mut ForkCell means no other references to this ForkCell exist,
        // so the borrow count is necessarily 0. Direct access without runtime check.
        unsafe { &mut *self.value.get() }
    }

    /// Reset borrow tracking after a fork in the child process.
    ///
    /// # Safety
    /// Must be called **only** in the child process after `fork(2)`. The parent's
    /// borrow count is meaningless in the new process because:
    /// - The parent's guards (Ref/RefMut) are in a different address space
    /// - The child has exclusive ownership of its memory copy
    /// - No other reference to this ForkCell exists in the child
    pub unsafe fn reset_after_fork(&self) {
        self.count.set(0);
    }

    // Test helpers — only available in the same crate (or with test cfg).
    #[cfg(test)]
    pub(crate) fn borrow_count(&self) -> isize {
        self.count.get()
    }
}
