//! `FICLONE` ioctl: whole-file reflink — a copy-on-write clone of one file
//! into another. The ioctl is issued on the **destination** fd with the
//! **source** fd as the argument; on success the destination holds a CoW
//! clone of the source's content and is extended to the source's size (a
//! write to either side then copies the touched page).
//!
//! The uapi was redesigned in kernel 7.x: the old `_IOR(0x94, 0x42, int)`
//! number is removed (it fails `ENOTTY`), and the new `_IOW(0x94, 9, int)`
//! is the only interface wrapped here — the same "new interface only"
//! stance as `fsverity.rs` for the >= 6.13 redesign.
//!
//! Kernel constraints (verified on 7.2.4, btrfs): the destination must be
//! open for writing and the source for reading (else `EBADF`, checked
//! before the filesystem); a non-empty destination larger than the source
//! is refused (`EINVAL`, no shrinking); an empty source is a no-op that
//! leaves the destination size untouched; a source on another filesystem
//! fails `EXDEV` (a memfd is its own tmpfs instance, so cross-device on
//! any host).

use crate::{LocalFd, SyscallError, cvt};

/// uapi `<linux/fs.h>` (kernel 7.x redesign): `#define FICLONE _IOW(0x94,
/// 9, int)` — re-exported from `libc`, which defines the constant; the
/// pre-7.x `_IOR(0x94, 0x42, int)` number is removed (fails `ENOTTY`) and
/// not wrapped.
pub use libc::FICLONE;

/// Pin the command number so a libc/uapi drift fails at compile time.
const _: () = assert!(FICLONE == 0x40049409);

impl LocalFd {
    /// `FICLONE`: create a reflink copy of the file behind `self` (the
    /// source) at `dst` (the destination).
    ///
    /// `self` must be open for reading and `dst` for writing (else
    /// `EBADF`). On success `dst` holds a copy-on-write clone of the
    /// source's content, extended to the source's size; a non-empty `dst`
    /// larger than the source fails `EINVAL`, an empty source is a no-op,
    /// and a source on another filesystem fails `EXDEV`.
    pub fn ficlone_to(&self, dst: &LocalFd) -> Result<(), SyscallError> {
        // SAFETY: both are valid open fds by the `LocalFd` invariant; the
        // ioctl passes the source fd by value and the kernel reads only
        // that int (no pointers).
        cvt(unsafe { libc::ioctl(dst.as_raw(), FICLONE, self.as_raw()) as isize })?;
        Ok(())
    }
}
