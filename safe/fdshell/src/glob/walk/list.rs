//! Listing an open directory and building result strings.

use alloc::vec::Vec;
use sys::{LocalFd, ShortCStr};

/// All entry names of an open directory, one 4 KiB getdents64 pass at a time.
pub(super) fn list(dirfd: &LocalFd) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    while let Ok(Some(batch)) = dirfd.dir_batch() {
        for entry in batch.iter() {
            names.push(entry.name.to_vec());
        }
    }
    names
}

/// `prefix` + name + `/`, as one path; `None` on a NUL byte or overflow.
pub(super) fn join(prefix: &ShortCStr, name: &ShortCStr) -> Option<ShortCStr> {
    let mut out = prefix.clone();
    out.push(name.clone());
    out.push_byte(b'/').ok()?;
    Some(out)
}

/// One matched entry: `prefix` + name, plus a trailing `/` when the pattern
/// demanded a directory.
pub(super) fn push_result(
    out: &mut Vec<Vec<u8>>,
    prefix: &ShortCStr,
    name: &ShortCStr,
    slash: bool,
) {
    let Some(mut r) = prefix.as_bytes().ok().map(|p| p.to_vec()) else {
        return;
    };
    if let Ok(nb) = name.as_bytes() {
        r.extend_from_slice(nb);
    }
    if slash {
        r.push(b'/');
    }
    out.push(r);
}
