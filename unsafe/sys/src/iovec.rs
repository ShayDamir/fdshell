#![forbid(unsafe_code)]

use core::marker::PhantomData;

#[repr(transparent)]
pub struct IoVec<'a>(libc::iovec, PhantomData<&'a [u8]>);

#[repr(transparent)]
pub struct IoVecMut<'a>(libc::iovec, PhantomData<&'a mut [u8]>);

impl<'a> IoVec<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        IoVec(
            libc::iovec {
                iov_base: buf.as_ptr().cast_mut().cast(),
                iov_len: buf.len(),
            },
            PhantomData,
        )
    }

    pub fn as_mut_ptr(&mut self) -> *mut libc::iovec {
        &raw mut self.0
    }
}

impl<'a> IoVecMut<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        IoVecMut(
            libc::iovec {
                iov_base: buf.as_mut_ptr().cast(),
                iov_len: buf.len(),
            },
            PhantomData,
        )
    }
}
