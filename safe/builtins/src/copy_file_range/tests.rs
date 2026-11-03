#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use core::ffi::CStr;
use error_stack::Report;
use sys::ShortCStr;

use crate::error::BuiltinError;

use super::parse::{CopyFileRangeConfig, copy_file_range_parse};

/// Build the substituted `refs` (command line, checked for `--help`) and the
/// expanded `args` (the `%var` tokens, so `%` is intact), then parse; the
/// returned config borrows both. Matches the pattern in `child/flock/tests.rs`.
fn parse(args: &[&str]) -> Result<CopyFileRangeConfig, Report<BuiltinError>> {
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    copy_file_range_parse(&refs, &origs)
}

fn is(e: &Report<BuiltinError>, what: &'static str) -> bool {
    matches!(
        e.current_context(),
        BuiltinError::InvalidArgument(s) if *s == what
    )
}

#[test]
fn parses_two_fd_vars() {
    let cfg = parse(&["%in", "%out"]).unwrap();
    assert_eq!(cfg.in_var.as_bytes().unwrap(), b"in");
    assert_eq!(cfg.out_var.as_bytes().unwrap(), b"out");
    assert_eq!(cfg.count, None);
}

#[test]
fn parses_optional_count() {
    let cfg = parse(&["%in", "%out", "100"]).unwrap();
    assert_eq!(cfg.count, Some(100));
}

// A `COUNT` is a bare positional number; the `=` form only applies to flags.
#[test]
fn count_equals_form_is_rejected() {
    assert!(is(
        &parse(&["%in", "%out", "count=100"]).unwrap_err(),
        "count"
    ));
}

#[test]
fn parses_zero_count() {
    let cfg = parse(&["%in", "%out", "0"]).unwrap();
    assert_eq!(cfg.count, Some(0));
}

#[test]
fn help_on_help_flag() {
    for args in [&["%in", "%out", "--help"][..], &["%in", "%out", "-h"][..]] {
        assert!(matches!(
            parse(args).unwrap_err().current_context(),
            BuiltinError::Help
        ));
    }
}

#[test]
fn requires_in_var() {
    assert!(matches!(
        parse(&[]).unwrap_err().current_context(),
        BuiltinError::MissingArgument("in fd var")
    ));
}

#[test]
fn requires_out_var() {
    assert!(matches!(
        parse(&["%in"]).unwrap_err().current_context(),
        BuiltinError::MissingArgument("out fd var")
    ));
}

#[test]
fn in_var_must_have_percent() {
    assert!(is(&parse(&["in", "%out"]).unwrap_err(), "in fd var"));
}

#[test]
fn in_var_must_not_be_empty() {
    assert!(is(&parse(&["%", "%out"]).unwrap_err(), "in fd var"));
}

#[test]
fn in_var_must_not_embed_percent() {
    assert!(is(&parse(&["%a%b", "%out"]).unwrap_err(), "in fd var"));
}

#[test]
fn out_var_must_have_percent() {
    assert!(is(&parse(&["%in", "out"]).unwrap_err(), "out fd var"));
}

#[test]
fn extra_positional_is_invalid_arg() {
    assert!(is(
        &parse(&["%in", "%out", "100", "extra"]).unwrap_err(),
        "arg"
    ));
}

#[test]
fn count_must_be_numeric() {
    assert!(is(&parse(&["%in", "%out", "abc"]).unwrap_err(), "count"));
}

#[test]
fn count_must_not_be_negative() {
    assert!(is(&parse(&["%in", "%out", "-5"]).unwrap_err(), "count"));
}

#[test]
fn count_must_not_overflow() {
    assert!(is(
        &parse(&["%in", "%out", "99999999999999999999911"]).unwrap_err(),
        "count"
    ));
}

// ---------------------------------------------------------------------------
// Exec path (`copy_file_range_exec`): driven with real `LocalFds`.
//
// These create the files on the real filesystem via the openat2 / write /
// lseek syscalls: the tests exercise regular files, so the fds come from
// openat2 rather than memfd. Each test works in its own scratch dir under the
// temp dir, so a failing test's teardown still runs on unwind: the dir's
// `Drop` owns every file in it (LESSONS, task #255).
// ---------------------------------------------------------------------------

use alloc::format;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::ops::Deref;
use std::path::{Path, PathBuf};

use sys::fcntl::{O_CREAT, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY, SEEK_SET};

use super::{copy_file_range_exec, remaining};
use sys::LocalFd;

// A monotonic counter, combined with the PID in `scratch`, so parallel tests
// never share a directory.
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A per-test scratch dir (pid + counter) under the temp dir, removed on drop.
/// pid + counter is unique among *live* processes only, so the dir must die
/// with its test process: pids are recycled, and a dir that outlives its
/// process is a later run's inherited dir with that run's files in it
/// (LESSONS, task #255). Removal is `.ok()`-tolerant: teardown never panics.
struct Scratch(PathBuf);

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// The scratch dir of this test; bind it by name, since a `let _ = …` binding
/// drops at once and deletes the dir mid-test (LESSONS, task #255).
fn scratch() -> Scratch {
    let c = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-cfra-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// A fresh config naming `%in`/`%out`, with an explicit optional count.
fn cfg(count: Option<u64>) -> CopyFileRangeConfig {
    CopyFileRangeConfig {
        in_var: ShortCStr::from_vec(b"%in".to_vec()).unwrap(),
        out_var: ShortCStr::from_vec(b"%out".to_vec()).unwrap(),
        count,
    }
}

/// Open `path` (a path inside the scratch dir) with `flags`; `O_CREAT` files
/// get the caller's mode via `open(2)` (0o666 here, so they stay readable).
fn open(path: &Path, flags: i32) -> LocalFd {
    let p = CString::new(path.to_str().unwrap().as_bytes()).unwrap();
    sys::openat2::open(p.as_c_str(), flags).unwrap()
}

/// Create (or truncate) `path` to contain `bytes`, then return it opened
/// read-only at offset 0.
fn source(path: &Path, bytes: &[u8]) -> LocalFd {
    let w = open(path, O_WRONLY | O_CREAT | O_TRUNC);
    w.write_all(bytes).unwrap();
    drop(w);
    open(path, O_RDONLY)
}

/// Create (or truncate) an empty `path`, opened for writing at offset 0.
fn dest(path: &Path) -> LocalFd {
    open(path, O_WRONLY | O_CREAT | O_TRUNC)
}

/// An explicit `COUNT` equal to the source size copies every byte.
#[test]
fn copies_all_with_explicit_count() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = dest(&scratch.join("dst"));
    let n = copy_file_range_exec(&cfg(Some(11)), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 11);
}

/// Without a `COUNT`, all of the source is copied to EOF.
#[test]
fn copies_all_to_eof() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = dest(&scratch.join("dst"));
    let n = copy_file_range_exec(&cfg(None), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 11);
}

/// A `COUNT` smaller than the source copies only that many bytes.
#[test]
fn copies_partial() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = dest(&scratch.join("dst"));
    let n = copy_file_range_exec(&cfg(Some(5)), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 5);
}

/// An explicit `COUNT` of 0 copies nothing and reports 0.
#[test]
fn explicit_zero_count_copies_nothing() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = dest(&scratch.join("dst"));
    let n = copy_file_range_exec(&cfg(Some(0)), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 0);
}

/// An empty source with no `COUNT` copies nothing and reports 0.
#[test]
fn empty_source_copies_nothing() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"");
    let out_fd = dest(&scratch.join("dst"));
    let n = copy_file_range_exec(&cfg(None), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 0);
}

/// A `COUNT` larger than the available bytes fails with `count`, since the
/// caller asked for exactly that many and fewer were copied.
#[test]
fn count_exceeding_available_is_invalid_argument() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = dest(&scratch.join("dst"));
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(matches!(
        e.current_context(),
        BuiltinError::InvalidArgument("count")
    ));
}

/// Create (or truncate) `path` containing `bytes`, opened read-write at
/// offset 0 — a pre-filled destination the restore path must undo.
fn dest_with(path: &Path, bytes: &[u8]) -> LocalFd {
    let fd = open(path, O_RDWR | O_CREAT | O_TRUNC);
    fd.write_all(bytes).unwrap();
    fd.lseek(0, SEEK_SET).unwrap();
    fd
}

/// The current offset of `fd`.
fn offset_of(fd: &LocalFd) -> i64 {
    fd.lseek(0, sys::fcntl::SEEK_CUR).unwrap()
}

/// Re-open `path` read-only and read its whole content (tests stay small).
fn contents(path: &Path) -> Vec<u8> {
    let fd = open(path, O_RDONLY);
    let mut buf = [0u8; 256];
    let n = fd.read_all(&mut buf).unwrap();
    buf.iter().take(n).copied().collect()
}

/// A `COUNT` larger than the available bytes fails with `count`, and the
/// destination is left unchanged: size truncated back, offset restored.
#[test]
fn short_exact_copy_restores_size_and_offset() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = dest_with(&scratch.join("dst"), b"hi");
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(is(&e, "count"));
    assert_eq!(offset_of(&out_fd), 0);
    assert_eq!(contents(&scratch.join("dst")), b"hi");
}

/// A short exact copy started at a non-zero destination offset restores that
/// offset and truncates the file back to its pre-copy size.
#[test]
fn short_exact_copy_restores_nonzero_offset() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = dest_with(&scratch.join("dst"), b"0123456789");
    out_fd.lseek(5, SEEK_SET).unwrap();
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(is(&e, "count"));
    assert_eq!(offset_of(&out_fd), 5);
    assert_eq!(contents(&scratch.join("dst")), b"0123456789");
}

/// A `COUNT` copy into a destination the kernel copy cannot write to
/// (`/dev/null`, like a pipe) is a `copy_file_range` syscall failure — the
/// syscall is rejected before any copy, so no restore (and no `count` error)
/// is reached.
#[test]
fn copy_to_dev_null_is_syscall_error() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"hello world");
    let out_fd = open(Path::new("/dev/null"), O_WRONLY);
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(matches!(e.current_context(), BuiltinError::Syscall));
}

/// An empty source with a `COUNT` fails with `count`; the destination is
/// left unchanged (a no-op restore).
#[test]
fn empty_source_with_count_is_count_error() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"");
    let out_fd = dest_with(&scratch.join("dst"), b"xy");
    let e = copy_file_range_exec(&cfg(Some(5)), &in_fd, &out_fd).unwrap_err();
    assert!(is(&e, "count"));
    assert_eq!(offset_of(&out_fd), 0);
    assert_eq!(contents(&scratch.join("dst")), b"xy");
}

/// `remaining`'s byte count is pinned at **its own computation site**: its only
/// consumer hands the value to a `copy_file_range` total that the kernel clamps
/// to the bytes really available, so nothing downstream can see the difference
/// (CATCHING_MUTANTS rule 4/9 — this call is the only place the value is
/// observable). Both sides of the subtraction are pinned: the zero offset, where
/// the two forms agree, and a nonzero one, where they cannot.
#[test]
fn remaining_counts_the_bytes_past_the_position() {
    let scratch = scratch();
    let in_fd = source(&scratch.join("src"), b"0123456789ab");
    assert_eq!(
        remaining(&in_fd).unwrap(),
        12,
        "a position at 0 owns the file"
    );
    in_fd.lseek(5, SEEK_SET).unwrap();
    assert_eq!(
        remaining(&in_fd).unwrap(),
        7,
        "a position past 0 owns the tail"
    );
    assert_eq!(
        offset_of(&in_fd),
        5,
        "the position is left where the caller put it"
    );
}
