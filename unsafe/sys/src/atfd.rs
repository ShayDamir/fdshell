use crate::{DupFd, Fd};
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

impl<'a> From<&'a Fd> for AtFd<'a> {
    fn from(fd: &'a Fd) -> Self {
        Self(fd.as_raw(), PhantomData)
    }
}

impl<'a> From<&'a DupFd> for AtFd<'a> {
    fn from(dup: &'a DupFd) -> Self {
        Self(dup.as_raw(), PhantomData)
    }
}
