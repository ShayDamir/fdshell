//! `FS_IOC_READ_VERITY_METADATA`: `pread()`-like reads of a verity file's
//! on-disk metadata (descriptor, Merkle tree, signature).

use crate::{LocalFd, SyscallError, cvt};

use super::uapi::{FS_IOC_READ_VERITY_METADATA, FsverityReadMetadataArg};

impl LocalFd {
    /// `FS_IOC_READ_VERITY_METADATA`: at most `buf.len()` bytes of the metadata
    /// item `metadata_type` (`FS_VERITY_METADATA_TYPE_*`) from `offset`.
    ///
    /// `pread()`-like: returns the bytes read (`0` at the item's end).
    /// `ENODATA` when the file is not verity or the item is absent (e.g. the
    /// signature of an unsigned file).
    pub fn read_verity_metadata_into(
        &self,
        metadata_type: u64,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize, SyscallError> {
        let arg = FsverityReadMetadataArg {
            metadata_type,
            offset,
            length: buf.len() as u64,
            buf_ptr: buf.as_mut_ptr() as u64,
            reserved: 0,
        };
        // SAFETY: `self` is a valid open fd; `buf` is a valid `arg.length`-byte
        // buffer the kernel fills (a read-write ioctl).
        let n = cvt(unsafe {
            libc::ioctl(
                self.as_raw(),
                FS_IOC_READ_VERITY_METADATA,
                core::ptr::addr_of!(arg) as *mut libc::c_void,
            ) as isize
        })?;
        Ok(n as usize)
    }
}
