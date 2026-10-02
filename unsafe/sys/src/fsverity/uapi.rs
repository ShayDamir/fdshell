//! uapi `fsverity.h` layout: hash-algorithm codes, ioctl numbers, and the
//! kernel-side structs. The size/offset asserts are load-bearing — the
//! `MEASURE` ioctl number encodes the 4-byte header, not a fixed digest array.

/// uapi `fsverity.h` hash-algorithm codes, 1-based (kernel `fsverity_hash_algs`).
pub const FS_VERITY_HASH_ALG_SHA256: u32 = 1;
pub const FS_VERITY_HASH_ALG_SHA512: u32 = 2;

/// uapi `struct fsverity_enable_arg` (128 bytes); the `ENABLE` ioctl argument.
#[repr(C)]
pub struct FsverityEnableArg {
    pub version: u32,
    pub hash_algorithm: u32,
    pub block_size: u32,
    pub salt_size: u32,
    pub salt_ptr: u64,
    pub sig_size: u32,
    pub reserved1: u32,
    pub sig_ptr: u64,
    pub reserved2: [u64; 11],
}

/// uapi `struct fsverity_digest` header. The uapi struct ends in a flexible
/// array member, so `size_of` is this 4-byte header — the value the `MEASURE`
/// ioctl number encodes (a 64-byte `digest[64]` field encodes the wrong number
/// and fails `ENOTTY`).
#[repr(C)]
pub struct FsverityDigestHead {
    pub algorithm: u16,
    pub size: u16,
}

const _: () = assert!(core::mem::size_of::<FsverityEnableArg>() == 128);
const _: () = assert!(core::mem::offset_of!(FsverityEnableArg, version) == 0);
const _: () = assert!(core::mem::offset_of!(FsverityEnableArg, salt_ptr) == 16);
const _: () = assert!(core::mem::size_of::<FsverityDigestHead>() == 4);

pub const FS_IOC_ENABLE_VERITY: libc::Ioctl = libc::_IOW::<FsverityEnableArg>(b'f' as u32, 133);
pub const FS_IOC_MEASURE_VERITY: libc::Ioctl = libc::_IOWR::<FsverityDigestHead>(b'f' as u32, 134);

/// `MEASURE` buffer: the 4-byte header followed by the 64-byte digest area.
#[repr(C)]
pub struct MeasureBuf {
    pub head: FsverityDigestHead,
    pub digest: [u8; 64],
}
