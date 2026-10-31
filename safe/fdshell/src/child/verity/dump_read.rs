//! The `--dump` read loop: streams the metadata item in 64 KiB chunks,
//! emitting hex rows to stdout as it goes (memory stays bounded; a Merkle
//! tree can be multi-MB). A syscall error (e.g. `ENODATA` for a non-verity
//! file) is propagated, so the exit code is the errno.

use core::cmp::min;

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::{LocalFd, OUT};

use super::dump::DumpSpec;
use super::emit::write_hex_rows;

/// Read chunk size (64 KiB): bounds the dump's memory to one chunk at a time.
/// A plain literal — the recorded `*` mutant was test-inseparable (hex rows are
/// chunk-boundary-independent), and a literal leaves no operator to mutate
/// (task #477).
const CHUNK: usize = 65_536;

pub(crate) fn dump_metadata(fd: &LocalFd, spec: &DumpSpec) -> Result<i32, Report<BuiltinError>> {
    let mut offset = spec.offset;
    let mut remaining = spec.length;
    let mut chunk = [0u8; CHUNK];
    loop {
        let to_read = match remaining {
            Some(r) => min(r as usize, CHUNK),
            None => CHUNK,
        };
        if to_read == 0 {
            break;
        }
        let buf = chunk.get_mut(..to_read).ok_or(BuiltinError::Never)?;
        let n = fd
            .read_verity_metadata_into(spec.r#type.code(), offset, buf)
            .change_context(BuiltinError::Syscall)?;
        if n == 0 {
            break;
        }
        let data = chunk.get(..n).ok_or(BuiltinError::Never)?;
        let rows = write_hex_rows(offset, data).change_context(BuiltinError::Io)?;
        OUT.write_str(&rows).change_context(BuiltinError::Io)?;
        offset += n as u64;
        if let Some(r) = &mut remaining {
            *r -= n as u64;
        }
    }
    Ok(0)
}
