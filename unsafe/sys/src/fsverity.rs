//! fs-verity ioctls (Linux >= 6.13): provision verity on an fd, measure the
//! resulting file digest, and read back the on-disk metadata (descriptor,
//! Merkle tree, signature). `ENABLE` builds the Merkle tree in-kernel;
//! `MEASURE` returns the file digest (the SHA over the on-disk
//! `fsverity_descriptor`); `READ_METADATA` is `pread()`-like over a metadata
//! item. `ENODATA` when the file is not a verity file. The pre-6.13 interface
//! (`FS_VERITY_FL` via `FS_IOC_SETFLAGS`) is removed in >= 6.13 and not wrapped.

mod read_metadata;
mod uapi;

use crate::{LocalFd, SyscallError, cvt};
use alloc::vec::Vec;

pub use uapi::{
    FS_IOC_ENABLE_VERITY, FS_IOC_MEASURE_VERITY, FS_IOC_READ_VERITY_METADATA,
    FS_VERITY_HASH_ALG_SHA256, FS_VERITY_HASH_ALG_SHA512, FS_VERITY_METADATA_TYPE_DESCRIPTOR,
    FS_VERITY_METADATA_TYPE_MERKLE_TREE, FS_VERITY_METADATA_TYPE_SIGNATURE, FsverityDigestHead,
    FsverityEnableArg, FsverityReadMetadataArg, MeasureBuf,
};

/// A measured fs-verity file digest; `digest.len() == size`.
#[derive(Clone, Debug, PartialEq)]
pub struct FsverityDigest {
    pub algorithm: u16,
    pub size: u16,
    pub digest: Vec<u8>,
}

impl LocalFd {
    /// `FS_IOC_ENABLE_VERITY`: provision fs-verity on the file behind `self`.
    ///
    /// `algorithm` is a uapi code (`FS_VERITY_HASH_ALG_*`); `block_size` is the
    /// Merkle-tree block size (a power of two). Version 1, no salt, no
    /// signature. The fd must be `O_RDONLY` and the caller must be able to
    /// write the inode.
    pub fn enable_verity(&self, algorithm: u32, block_size: u32) -> Result<(), SyscallError> {
        let arg = FsverityEnableArg {
            version: 1,
            hash_algorithm: algorithm,
            block_size,
            salt_size: 0,
            salt_ptr: 0,
            sig_size: 0,
            reserved1: 0,
            sig_ptr: 0,
            reserved2: [0; 11],
        };
        // SAFETY: `self` is a valid open fd; `arg` is a valid 128-byte buffer
        // the kernel reads (a write-only ioctl); the caller guarantees write
        // access.
        cvt(unsafe {
            libc::ioctl(
                self.as_raw(),
                FS_IOC_ENABLE_VERITY,
                core::ptr::addr_of!(arg) as *mut libc::c_void,
            ) as isize
        })?;
        Ok(())
    }

    /// `FS_IOC_MEASURE_VERITY`: the file digest of the verity file behind
    /// `self`.
    ///
    /// A 64-byte digest area is pre-sized to 64 (the sha512 maximum); the
    /// kernel writes back the header plus `size` digest bytes. `ENODATA` when
    /// not verity.
    pub fn measure_verity(&self) -> Result<FsverityDigest, SyscallError> {
        let buf = MeasureBuf {
            head: FsverityDigestHead {
                algorithm: 0,
                size: 64,
            },
            digest: [0u8; 64],
        };
        // SAFETY: `self` is a valid open fd; `buf` is a valid 68-byte buffer
        // the kernel fills (a read-write ioctl). The ioctl number encodes the
        // 4-byte header; the kernel writes header + digest_size bytes, both
        // within `buf`.
        cvt(unsafe {
            libc::ioctl(
                self.as_raw(),
                FS_IOC_MEASURE_VERITY,
                core::ptr::addr_of!(buf) as *mut libc::c_void,
            ) as isize
        })?;
        let size = buf.head.size as usize;
        let digest = buf.digest.get(..size).ok_or(SyscallError::Never)?.to_vec();
        Ok(FsverityDigest {
            algorithm: buf.head.algorithm,
            size: buf.head.size,
            digest,
        })
    }
}
