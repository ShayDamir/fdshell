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
const _: () = assert!(core::mem::size_of::<FsverityReadMetadataArg>() == 40);
const _: () = assert!(FS_IOC_READ_VERITY_METADATA == 0xC0286687);

pub const FS_IOC_ENABLE_VERITY: libc::Ioctl = libc::_IOW::<FsverityEnableArg>(b'f' as u32, 133);
pub const FS_IOC_MEASURE_VERITY: libc::Ioctl = libc::_IOWR::<FsverityDigestHead>(b'f' as u32, 134);

/// uapi `struct fsverity_read_metadata_arg`: fixed 40 bytes, no flexible
/// array member — the whole struct is the sized prefix the `READ_METADATA`
/// ioctl number encodes.
#[repr(C)]
pub struct FsverityReadMetadataArg {
    pub metadata_type: u64,
    pub offset: u64,
    pub length: u64,
    pub buf_ptr: u64,
    pub reserved: u64,
}

/// uapi `FS_VERITY_METADATA_TYPE_*`: the metadata items `READ_METADATA` reads.
pub const FS_VERITY_METADATA_TYPE_MERKLE_TREE: u64 = 1;
pub const FS_VERITY_METADATA_TYPE_DESCRIPTOR: u64 = 2;
pub const FS_VERITY_METADATA_TYPE_SIGNATURE: u64 = 3;

pub const FS_IOC_READ_VERITY_METADATA: libc::Ioctl =
    libc::_IOWR::<FsverityReadMetadataArg>(b'f' as u32, 135);

/// `MEASURE` buffer: the 4-byte header followed by the 64-byte digest area.
#[repr(C)]
pub struct MeasureBuf {
    pub head: FsverityDigestHead,
    pub digest: [u8; 64],
}
