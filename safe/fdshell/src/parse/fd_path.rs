use sys::ShortCStr;

const DEV_FD: &[u8] = b"/dev/fd/";
const PROC_FD: &[u8] = b"/proc/self/fd/";

/// Map a `/dev/fd/N` (or `/proc/self/fd/N`) path to its fd number.
pub(super) fn fd_path_target(s: &ShortCStr) -> Option<i32> {
    let rest = s.strip_prefix(DEV_FD).or_else(|| s.strip_prefix(PROC_FD))?;
    let n = rest.parse::<i32>().ok()?;
    (n >= 0).then_some(n)
}
