use crate::LocalFd;

/// True when `fd` refers to a terminal device. Returns a `bool` (not a
/// `Result` via `cvt`) deliberately: `isatty` signals "not a tty" with `0`,
/// not `-1`, so `cvt`'s -1 check would never fire and the `-t` predicate wants
/// exactly a bool.
pub fn isatty(fd: &LocalFd) -> bool {
    // SAFETY: `fd.as_raw()` is a valid fd by the `LocalFd` invariant; `isatty`
    // on an invalid fd returns 0 (treated as "not a terminal"), never panics.
    unsafe { libc::isatty(fd.as_raw()) != 0 }
}
