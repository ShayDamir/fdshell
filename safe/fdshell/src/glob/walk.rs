//! Component walk for one pattern word (explicit work stack, no recursion).

mod descend;
mod list;
mod split;

use alloc::vec;
use alloc::vec::Vec;
use sys::fcntl::O_DIRECTORY;
use sys::openat2::{OpenHow, openat2};
use sys::{AtFd, ShortCStr};

use super::r#match as m;

/// All matching full result strings, bytewise-sorted; empty when nothing
/// matches. A directory that cannot be opened or listed drops its branch
/// silently (bash behavior: no error, no stderr).
pub(super) fn walk(word: &ShortCStr, mask: &[bool]) -> Vec<Vec<u8>> {
    let Ok(bytes) = word.as_bytes() else {
        return Vec::new();
    };
    let parts = split::split(bytes, mask);
    let dir = OpenHow::new(O_DIRECTORY as u64, 0);
    let root = if parts.absolute {
        openat2(AtFd::cwd(), c"/", &dir)
    } else {
        openat2(AtFd::cwd(), c".", &dir)
    };
    let Ok(root) = root else {
        return Vec::new();
    };
    let prefix = ShortCStr::from(if parts.absolute { c"/" } else { c"" });
    let mut stack = vec![(root, 0usize, prefix)];
    let mut out = Vec::new();
    while let Some((dirfd, idx, prefix)) = stack.pop() {
        let Some(comp) = parts.comps.get(idx) else {
            // Only reachable for a zero-component word: the pattern is just
            // `/` and the root (a directory by construction) is the result.
            out.push(prefix.as_bytes().unwrap_or(b"").to_vec());
            continue;
        };
        let last = idx + 1 == parts.comps.len();
        if comp.literal {
            descend::literal_component(&dirfd, idx, &parts, last, &prefix, &mut out, &mut stack);
            continue;
        }
        for name in list::list(&dirfd) {
            if name == b"." || name == b".." {
                continue;
            }
            if !m::match_component(&comp.pat, &comp.mask, &name) {
                continue;
            }
            let Ok(entry) = ShortCStr::from_vec(name) else {
                continue;
            };
            if last {
                // A listed entry exists by construction; only the directory
                // constraint needs a check.
                if !parts.dir_only {
                    list::push_result(&mut out, &prefix, &entry, false);
                } else if openat2(dirfd.at(), entry.export(), &dir).is_ok() {
                    list::push_result(&mut out, &prefix, &entry, true);
                }
            } else if let Ok(fd) = openat2(dirfd.at(), entry.export(), &dir) {
                descend::descend(&mut stack, idx, &prefix, &entry, fd);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests;
