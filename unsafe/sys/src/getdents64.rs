//! `getdents64(2)` directory listing.
//!
//! Reads directory entries into a caller-provided buffer. Each record begins
//! with a fixed header followed by the NUL-terminated name, and records are
//! delimited by each record's own `d_reclen` length (not a fixed struct size).
//! Not exported through `libc`, so the syscall number is invoked directly.

use crate::{SyscallError, cvt};

/// Record layout, matching the kernel `struct dirent64` on x86_64. `d_name` is
/// a flexible trailing array at offset `D_NAME`; advance between records by each
/// record's `d_reclen` (see [`Iter`]).
#[repr(C)]
struct DirEntry64 {
    d_ino: u64,
    d_off: i64,
    d_reclen: u16,
    d_type: u8,
    d_name: [u8; 256],
}

const D_RECLEN: usize = 16;
const D_TYPE: usize = 18;
const D_NAME: usize = 19;

/// One entry parsed out of a [`getdents`] buffer: the name (truncated at its
/// NUL) and the kernel file type (`S_IF*` from `stat.h`).
pub struct DirEntry<'a> {
    pub name: &'a [u8],
    pub d_type: u8,
}

/// Read directory entries from `fd` into `buf`, returning the number of bytes
/// placed in `buf` (`0` at end of directory, error otherwise).
pub fn getdents(fd: i32, buf: &mut [u8]) -> Result<usize, SyscallError> {
    // SAFETY: `buf.as_mut_ptr()` addresses `buf.len()` bytes valid for the
    // duration of the call; `getdents64` writes records into it and reads only
    // the fd number. `SYS_getdents64` (217) is valid on Linux ≥5.3 x86_64.
    let ret = cvt(unsafe {
        libc::syscall(
            libc::SYS_getdents64,
            fd,
            buf.as_mut_ptr().cast::<DirEntry64>(),
            buf.len(),
        ) as isize
    })?;
    Ok(ret as usize)
}

/// Iterate the entries packed into the first `n` bytes of a [`getdents`] buffer.
pub struct Iter<'a> {
    buf: &'a [u8],
    off: usize,
}

impl<'a> Iter<'a> {
    /// Iterate the entries within the first `n` bytes of `buf`.
    pub fn new(buf: &'a [u8], n: usize) -> Self {
        let buf = buf.get(..n).unwrap_or(&[]);
        Iter { buf, off: 0 }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = DirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // A full header (through `d_type` at offset 18) must be present.
        let reclen = self
            .buf
            .get(self.off + D_RECLEN..self.off + D_RECLEN + 2)?
            .try_into()
            .ok()?;
        let reclen = u16::from_le_bytes(reclen);
        // `d_reclen` bounds the name; bail if it runs past the buffer.
        let raw = self
            .buf
            .get(self.off + D_NAME..self.off + reclen as usize)?;
        let name = raw.split(|&b| b == 0).next().unwrap_or(&[]);
        let d_type = *self.buf.get(self.off + D_TYPE)?;
        let next = self.off + reclen as usize;
        if next <= self.off {
            return None;
        }
        self.off = next;
        Some(DirEntry { name, d_type })
    }
}
