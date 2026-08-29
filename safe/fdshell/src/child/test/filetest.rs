//! File-system operators of `test`. Each operand is a path or a `%var` fd:
//! kinds, size, mode bits (`-e -f -d -b -c -p -S -L -s -g -k`), the terminal
//! test (`-t`), permissions (`-r -w -x`), and the binary comparisons
//! (`-nt -ot -ef -fdeq -fdne`).

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;
use sys::stat::{
    FileStat, S_IFBLK, S_IFCHR, S_IFDIR, S_IFIFO, S_IFLNK, S_IFMT, S_IFREG, S_IFSOCK, S_ISGID,
    S_ISVTX,
};

use super::perm::access_test;
use super::stat::{fd_var, stat_operand};

/// Unary file tests: `-e -f -d -b -c -p -S -L -s -g -k -t -r -w -x`.
pub(super) fn file_test(
    op: &[u8],
    arg: &CStr,
    orig: Option<&ShortCStr>,
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let ok = if op == b"-t" {
        tty_test(arg, orig, state)
    } else if op == b"-r" || op == b"-w" || op == b"-x" {
        access_test(op, arg, orig, state)?
    } else {
        let st = stat_operand(arg, orig, state, op != b"-L")?;
        stat_test(op, st.as_ref())?
    };
    Ok(usize::from(!ok) as i32)
}

/// Binary file tests: `-nt -ot -ef -fdeq -fdne`.
pub(super) fn file_binary_test(
    lhs: &CStr,
    op: &[u8],
    lorig: Option<&ShortCStr>,
    rhs: &CStr,
    rorig: Option<&ShortCStr>,
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let l = stat_operand(lhs, lorig, state, true)?;
    let r = stat_operand(rhs, rorig, state, true)?;
    let ok = match (l, r) {
        (Some(a), Some(b)) => binary_op(op, &a, &b)?,
        _ => false,
    };
    Ok(usize::from(!ok) as i32)
}

/// `stat`-derived unary tests; a `None` operand is false.
pub(super) fn stat_test(op: &[u8], st: Option<&FileStat>) -> Result<bool, Report<BuiltinError>> {
    match st {
        None => Ok(false),
        Some(st) => match op {
            b"-e" => Ok(true),
            b"-f" => Ok(kind(st, S_IFREG)),
            b"-d" => Ok(kind(st, S_IFDIR)),
            b"-b" => Ok(kind(st, S_IFBLK)),
            b"-c" => Ok(kind(st, S_IFCHR)),
            b"-p" => Ok(kind(st, S_IFIFO)),
            b"-S" => Ok(kind(st, S_IFSOCK)),
            b"-L" => Ok(kind(st, S_IFLNK)),
            b"-s" => Ok(st.size > 0),
            b"-g" => Ok(st.mode & S_ISGID != 0),
            b"-k" => Ok(st.mode & S_ISVTX != 0),
            _ => bail!(BuiltinError::Never),
        },
    }
}

pub(super) fn kind(st: &FileStat, file_type: u32) -> bool {
    st.mode & S_IFMT == file_type
}

/// Binary comparison of two stat results.
pub(super) fn binary_op(
    op: &[u8],
    l: &FileStat,
    r: &FileStat,
) -> Result<bool, Report<BuiltinError>> {
    Ok(match op {
        b"-nt" => l.mtime > r.mtime,
        b"-ot" => l.mtime < r.mtime,
        b"-ef" | b"-fdeq" => same_inode(l, r),
        b"-fdne" => !same_inode(l, r),
        _ => bail!(BuiltinError::Never),
    })
}

fn same_inode(l: &FileStat, r: &FileStat) -> bool {
    l.dev == r.dev && l.ino == r.ino
}

/// `-t`: an fd var is true iff it is a terminal; a plain path is never one.
fn tty_test(_arg: &CStr, orig: Option<&ShortCStr>, state: &ShellState) -> bool {
    fd_var(orig, state).is_some_and(sys::tty::isatty)
}
