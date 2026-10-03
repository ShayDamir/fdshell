//! `--dump <type> [--offset N] [--length N]` argument parsing (no I/O): the
//! metadata item, the read start offset, and the optional byte cap.

mod apply;

pub(crate) use apply::apply;

use builtins::error::BuiltinError;
use error_stack::{Report, bail, ensure};

/// The `--dump` metadata item (uapi `FS_VERITY_METADATA_TYPE_*` codes).
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum MetadataType {
    MerkleTree,
    Descriptor,
    Signature,
}

impl MetadataType {
    /// The uapi code the `READ_METADATA` ioctl expects.
    pub(crate) fn code(self) -> u64 {
        match self {
            MetadataType::MerkleTree => sys::fsverity::FS_VERITY_METADATA_TYPE_MERKLE_TREE,
            MetadataType::Descriptor => sys::fsverity::FS_VERITY_METADATA_TYPE_DESCRIPTOR,
            MetadataType::Signature => sys::fsverity::FS_VERITY_METADATA_TYPE_SIGNATURE,
        }
    }
}

/// A parsed `--dump` request: item, start offset, and optional byte cap.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct DumpSpec {
    pub(crate) r#type: MetadataType,
    pub(crate) offset: u64,
    pub(crate) length: Option<u64>,
}

/// `tree`/`merkle_tree`, `descriptor`, or `signature`.
pub(crate) fn parse_metadata_type(v: &[u8]) -> Result<MetadataType, Report<BuiltinError>> {
    match v {
        b"tree" | b"merkle_tree" => Ok(MetadataType::MerkleTree),
        b"descriptor" => Ok(MetadataType::Descriptor),
        b"signature" => Ok(MetadataType::Signature),
        _ => bail!(BuiltinError::InvalidArgument("dump type")),
    }
}

/// A base-10 unsigned integer: rejects empty, non-digit, and overflow.
pub(crate) fn parse_uint(v: &[u8], what: &'static str) -> Result<u64, Report<BuiltinError>> {
    ensure!(!v.is_empty(), BuiltinError::InvalidArgument(what));
    let mut n: u64 = 0;
    for &c in v {
        let d = (c as char)
            .to_digit(10)
            .ok_or(BuiltinError::InvalidArgument(what))? as u64;
        n = n
            .checked_mul(10)
            .and_then(|m| m.checked_add(d))
            .ok_or(BuiltinError::InvalidArgument(what))?;
    }
    Ok(n)
}
