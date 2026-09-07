//! One-line metadata summary printed by the `statx` builtin.

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::stat::{S_IFBLK, S_IFCHR, S_IFDIR, S_IFIFO, S_IFLNK, S_IFMT, S_IFREG, S_IFSOCK};
use sys::statx::Statx;

/// File-kind name for the `S_IFMT` bits of `mode`.
pub(crate) fn kind(mode: u32) -> &'static str {
    match mode & S_IFMT {
        S_IFREG => "file",
        S_IFDIR => "dir",
        S_IFLNK => "symlink",
        S_IFIFO => "fifo",
        S_IFCHR => "char",
        S_IFBLK => "block",
        S_IFSOCK => "sock",
        _ => "unknown",
    }
}

/// `kind=.. size=.. mode=.. ino=.. dev=MAJ:MIN mtime=..` plus a newline.
pub(crate) fn line(st: &Statx) -> Result<sys::ShortCStr, Report<BuiltinError>> {
    sys::format!(
        "kind={} size={} mode={:o} ino={} dev={}:{} mtime={}\n",
        kind(st.mode),
        st.size,
        st.mode & 0o7777,
        st.ino,
        st.dev_major,
        st.dev_minor,
        st.mtime_sec
    )
    .change_context(BuiltinError::Io)
}
