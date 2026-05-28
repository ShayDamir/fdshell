use crate::{ExportedFd, ImportedFd, LocalFd};
use core::marker::PhantomData;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct AtFd<'a>(i32, PhantomData<&'a ()>);

impl<'a> AtFd<'a> {
    pub const fn cwd() -> Self {
        Self(libc::AT_FDCWD, PhantomData)
    }

    /// # Safety
    /// `raw` must be a valid fd number that stays valid for `'a`.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw, PhantomData)
    }

    pub fn as_raw(&self) -> i32 {
        self.0
    }
}

impl<'a> From<&'a LocalFd> for AtFd<'a> {
    fn from(fd: &'a LocalFd) -> Self {
        Self(fd.as_raw(), PhantomData)
    }
}

impl<'a> From<&'a ImportedFd> for AtFd<'a> {
    fn from(dup: &'a ImportedFd) -> Self {
        Self(dup.as_raw(), PhantomData)
    }
}

impl<'a> From<&'a ExportedFd> for AtFd<'a> {
    fn from(dup: &'a ExportedFd) -> Self {
        Self(dup.as_raw(), PhantomData)
    }
}
