#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::string::String;
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
// openat2 rather than memfd. The two files are cleaned up at the end of each
// test, leaving no trace behind.
// ---------------------------------------------------------------------------

use alloc::format;
use core::sync::atomic::{AtomicUsize, Ordering};

use sys::fcntl::{O_CREAT, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY, SEEK_SET};
use sys::fileat::unlinkat;
use sys::rw::{lseek, read_all, write_all};

use super::copy_file_range_exec;
use sys::AtFd;
use sys::LocalFd;

// A monotonic counter, combined with the PID in `unique`, so parallel tests
// never share a file.
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A process-unique path: `cfra_<pid>_<tag>_<counter>`.
///
/// nextest runs each unit test in its own process, so a bare counter would
/// reset per process (two tests would share `cfra_src_0`); the PID separates
/// processes and the counter separates tests within one process.
fn unique(tag: &str) -> String {
    let pid = sys::env::getpid().as_raw();
    format!("cfra_{pid}_{tag}_{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// A fresh config naming `%in`/`%out`, with an explicit optional count.
fn cfg(count: Option<u64>) -> CopyFileRangeConfig {
    CopyFileRangeConfig {
        in_var: ShortCStr::from_vec(b"%in".to_vec()).unwrap(),
        out_var: ShortCStr::from_vec(b"%out".to_vec()).unwrap(),
        count,
    }
}

/// Open `path` (relative to the crate root) with `flags`; `O_CREAT` files get
/// the caller's mode via `open(2)` (0o666 here, so they stay readable).
fn open(path: &str, flags: i32) -> LocalFd {
    let p = CString::new(path).unwrap();
    sys::openat2::open(p.as_c_str(), flags).unwrap()
}

/// Create (or truncate) `path` to contain `bytes`, then return it opened
/// read-only at offset 0.
fn source(path: &str, bytes: &[u8]) -> LocalFd {
    let w = open(path, O_WRONLY | O_CREAT | O_TRUNC);
    write_all(&w, bytes).unwrap();
    drop(w);
    open(path, O_RDONLY)
}

/// Create (or truncate) an empty `path`, opened for writing at offset 0.
fn dest(path: &str) -> LocalFd {
    open(path, O_WRONLY | O_CREAT | O_TRUNC)
}

/// Remove `path` if present; cleanup is best-effort, so failures are ignored.
fn cleanup(path: &str) {
    let p = CString::new(path).unwrap();
    let _ = unlinkat(AtFd::cwd(), p.as_c_str(), 0);
}

/// An explicit `COUNT` equal to the source size copies every byte.
#[test]
fn copies_all_with_explicit_count() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"hello world");
    let out_fd = dest(&dst);
    let n = copy_file_range_exec(&cfg(Some(11)), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 11);
    cleanup(&src);
    cleanup(&dst);
}

/// Without a `COUNT`, all of the source is copied to EOF.
#[test]
fn copies_all_to_eof() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"hello world");
    let out_fd = dest(&dst);
    let n = copy_file_range_exec(&cfg(None), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 11);
    cleanup(&src);
    cleanup(&dst);
}

/// A `COUNT` smaller than the source copies only that many bytes.
#[test]
fn copies_partial() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"hello world");
    let out_fd = dest(&dst);
    let n = copy_file_range_exec(&cfg(Some(5)), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 5);
    cleanup(&src);
    cleanup(&dst);
}

/// An explicit `COUNT` of 0 copies nothing and reports 0.
#[test]
fn explicit_zero_count_copies_nothing() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"hello world");
    let out_fd = dest(&dst);
    let n = copy_file_range_exec(&cfg(Some(0)), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 0);
    cleanup(&src);
    cleanup(&dst);
}

/// An empty source with no `COUNT` copies nothing and reports 0.
#[test]
fn empty_source_copies_nothing() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"");
    let out_fd = dest(&dst);
    let n = copy_file_range_exec(&cfg(None), &in_fd, &out_fd).unwrap();
    assert_eq!(n, 0);
    cleanup(&src);
    cleanup(&dst);
}

/// A `COUNT` larger than the available bytes fails with `count`, since the
/// caller asked for exactly that many and fewer were copied.
#[test]
fn count_exceeding_available_is_invalid_argument() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"hello world");
    let out_fd = dest(&dst);
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(matches!(
        e.current_context(),
        BuiltinError::InvalidArgument("count")
    ));
    cleanup(&src);
    cleanup(&dst);
}

/// Create (or truncate) `path` containing `bytes`, opened read-write at
/// offset 0 — a pre-filled destination the restore path must undo.
fn dest_with(path: &str, bytes: &[u8]) -> LocalFd {
    let fd = open(path, O_RDWR | O_CREAT | O_TRUNC);
    write_all(&fd, bytes).unwrap();
    lseek(&fd, 0, SEEK_SET).unwrap();
    fd
}

/// The current offset of `fd`.
fn offset_of(fd: &LocalFd) -> i64 {
    lseek(fd, 0, sys::fcntl::SEEK_CUR).unwrap()
}

/// Re-open `path` read-only and read its whole content (tests stay small).
fn contents(path: &str) -> Vec<u8> {
    let fd = open(path, O_RDONLY);
    let mut buf = [0u8; 256];
    let n = read_all(&fd, &mut buf).unwrap();
    buf.iter().take(n).copied().collect()
}

/// A `COUNT` larger than the available bytes fails with `count`, and the
/// destination is left unchanged: size truncated back, offset restored.
#[test]
fn short_exact_copy_restores_size_and_offset() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"hello world");
    let out_fd = dest_with(&dst, b"hi");
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(is(&e, "count"));
    assert_eq!(offset_of(&out_fd), 0);
    assert_eq!(contents(&dst), b"hi");
    cleanup(&src);
    cleanup(&dst);
}

/// A short exact copy started at a non-zero destination offset restores that
/// offset and truncates the file back to its pre-copy size.
#[test]
fn short_exact_copy_restores_nonzero_offset() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"hello world");
    let out_fd = dest_with(&dst, b"0123456789");
    lseek(&out_fd, 5, SEEK_SET).unwrap();
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(is(&e, "count"));
    assert_eq!(offset_of(&out_fd), 5);
    assert_eq!(contents(&dst), b"0123456789");
    cleanup(&src);
    cleanup(&dst);
}

/// A `COUNT` copy into a destination the kernel copy cannot write to
/// (`/dev/null`, like a pipe) is a `copy_file_range` syscall failure — the
/// syscall is rejected before any copy, so no restore (and no `count` error)
/// is reached.
#[test]
fn copy_to_dev_null_is_syscall_error() {
    let src = unique("cfra_src");
    let in_fd = source(&src, b"hello world");
    let out_fd = open("/dev/null", O_WRONLY);
    let e = copy_file_range_exec(&cfg(Some(100)), &in_fd, &out_fd).unwrap_err();
    assert!(matches!(e.current_context(), BuiltinError::Syscall));
    cleanup(&src);
}

/// An empty source with a `COUNT` fails with `count`; the destination is
/// left unchanged (a no-op restore).
#[test]
fn empty_source_with_count_is_count_error() {
    let src = unique("cfra_src");
    let dst = unique("cfra_dst");
    let in_fd = source(&src, b"");
    let out_fd = dest_with(&dst, b"xy");
    let e = copy_file_range_exec(&cfg(Some(5)), &in_fd, &out_fd).unwrap_err();
    assert!(is(&e, "count"));
    assert_eq!(offset_of(&out_fd), 0);
    assert_eq!(contents(&dst), b"xy");
    cleanup(&src);
    cleanup(&dst);
}
