/// Guard returned by `borrow()`. Drops release the shared borrow.
pub struct Ref<'a, T> {
    pub(super) cell: &'a crate::fork_cell::ForkCell<T>,
}

impl<T> core::ops::Deref for Ref<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: borrow count (≥1 shared borrows) guarantees safe shared access.
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> Drop for Ref<'_, T> {
    fn drop(&mut self) {
        let cur = self.cell.count.get();
        self.cell.count.set(cur - 1);
    }
}

/// Guard returned by `borrow_mut()`. Drops release the exclusive borrow.
pub struct RefMut<'a, T> {
    pub(super) cell: &'a crate::fork_cell::ForkCell<T>,
}

impl<T> core::ops::Deref for RefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: borrow count == -1 (exclusive) guarantees safe shared access.
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> core::ops::DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: borrow count == -1 (exclusive) guarantees safe mutable access.
        unsafe { &mut *self.cell.value.get() }
    }
}

impl<T> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        self.cell.count.set(0);
    }
}
