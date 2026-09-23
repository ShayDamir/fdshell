//! Direct resolution of literal components and descent into subdirectories.

use alloc::vec::Vec;
use sys::fcntl::{AT_SYMLINK_NOFOLLOW, O_DIRECTORY};
use sys::openat2::{OpenHow, openat2};
use sys::{LocalFd, ShortCStr};

use super::list;
use super::split::Parts;

/// A component with no pattern bytes: resolve it directly, no listing.
pub(super) fn literal_component(
    dirfd: &LocalFd,
    idx: usize,
    parts: &Parts,
    last: bool,
    prefix: &ShortCStr,
    out: &mut Vec<Vec<u8>>,
    stack: &mut Vec<(LocalFd, usize, ShortCStr)>,
) {
    let Some(comp) = parts.comps.get(idx) else {
        return;
    };
    let dir = OpenHow::new(O_DIRECTORY as u64, 0);
    let Ok(name) = ShortCStr::from_vec(comp.pat.clone()) else {
        return;
    };
    if last && !parts.dir_only {
        // The name came from the word, not a listing: it must exist. A
        // broken symlink exists (bash matches it), so stat the link itself.
        let exported = name.export();
        if sys::statx::statx(dirfd.at(), exported.as_ref(), AT_SYMLINK_NOFOLLOW).is_ok() {
            list::push_result(out, prefix, &name, false);
        }
        return;
    }
    let Ok(fd) = openat2(dirfd.at(), name.export(), &dir) else {
        return;
    };
    if last {
        list::push_result(out, prefix, &name, true);
    } else {
        descend(stack, idx, prefix, &name, fd);
    }
}

/// Push the next component for the freshly opened `fd`, under `name`.
pub(super) fn descend(
    stack: &mut Vec<(LocalFd, usize, ShortCStr)>,
    idx: usize,
    prefix: &ShortCStr,
    name: &ShortCStr,
    fd: LocalFd,
) {
    if let Some(child) = list::join(prefix, name) {
        stack.push((fd, idx + 1, child));
    }
}
