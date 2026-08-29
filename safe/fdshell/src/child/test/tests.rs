#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::CStr;

use builtins::error::BuiltinError;
use sys::fcntl::{O_DIRECTORY, O_RDONLY};
use sys::stat::{
    FileStat, S_IFBLK, S_IFCHR, S_IFDIR, S_IFIFO, S_IFLNK, S_IFREG, S_IFSOCK, S_ISGID, S_ISVTX,
};
use sys::{Origin, ShortCStr, Trace};

use crate::state::{FdVar, ShellState};
use std::format;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::eval;

/// Build substituted (`refs`) and original (`origs`) argument views for `args`
/// and call `f`; the backing strings live for the duration of the call.
fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

fn run(args: &[&str], state: &ShellState) -> Result<i32, error_stack::Report<BuiltinError>> {
    with_refs(args, |refs, origs| eval(refs, origs, state))
}

/// A unique temp path, namespaced by pid so parallel tests do not collide.
fn tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fdshell-test-{}.{}", std::process::id(), name))
}

fn cstr(path: &Path) -> CString {
    CString::new(path.to_str().unwrap()).unwrap()
}

fn make_file(path: &Path, mode: u32) {
    std::fs::write(path, b"data").unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// Create (or rewrite) a file and pin its mtime to `secs` since the epoch.
fn touch(path: &Path, secs: i64) {
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64);
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .unwrap();
    f.set_times(std::fs::FileTimes::new().set_modified(t))
        .unwrap();
}

fn ins(state: &mut ShellState, name: &'static CStr, fd: sys::LocalFd) {
    state.fds.insert(
        name.into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
}

fn fst(mode: u32, size: u64, mtime: i64, dev: u64, ino: u64) -> FileStat {
    FileStat {
        ino,
        mode,
        dev,
        rdev: 0,
        size,
        mtime,
    }
}

#[test]
fn single_string_truthiness() {
    let state = ShellState::new();
    assert_eq!(run(&["x"], &state).unwrap(), 0);
    assert_eq!(run(&[""], &state).unwrap(), 1);
    // A lone operator word is a non-empty string: true (bash rule).
    assert_eq!(run(&["-f"], &state).unwrap(), 0);
}

#[test]
fn zero_operands_is_false() {
    let state = ShellState::new();
    assert_eq!(run(&[], &state).unwrap(), 1);
}

#[test]
fn string_equality() {
    let state = ShellState::new();
    assert_eq!(run(&["a", "=", "a"], &state).unwrap(), 0);
    assert_eq!(run(&["a", "=", "b"], &state).unwrap(), 1);
    assert_eq!(run(&["a", "!=", "b"], &state).unwrap(), 0);
    assert_eq!(run(&["a", "!=", "a"], &state).unwrap(), 1);
}

#[test]
fn integer_comparisons() {
    let state = ShellState::new();
    assert_eq!(run(&["1", "-eq", "1"], &state).unwrap(), 0);
    assert_eq!(run(&["1", "-ne", "2"], &state).unwrap(), 0);
    assert_eq!(run(&["1", "-lt", "2"], &state).unwrap(), 0);
    assert_eq!(run(&["2", "-le", "2"], &state).unwrap(), 0);
    assert_eq!(run(&["-5", "-gt", "-6"], &state).unwrap(), 0);
    assert_eq!(run(&["2", "-ge", "3"], &state).unwrap(), 1);
    // Equality boundaries: strictness of -lt/-gt, inclusiveness of -ge.
    assert_eq!(run(&["1", "-lt", "1"], &state).unwrap(), 1);
    assert_eq!(run(&["1", "-gt", "1"], &state).unwrap(), 1);
    assert_eq!(run(&["2", "-ge", "2"], &state).unwrap(), 0);
}

#[test]
fn string_empty_tests() {
    let state = ShellState::new();
    assert_eq!(run(&["-z", ""], &state).unwrap(), 0);
    assert_eq!(run(&["-z", "x"], &state).unwrap(), 1);
    assert_eq!(run(&["-n", ""], &state).unwrap(), 1);
    assert_eq!(run(&["-n", "x"], &state).unwrap(), 0);
}

#[test]
fn non_integer_operand_is_error() {
    let state = ShellState::new();
    assert!(matches!(
        run(&["a", "-eq", "1"], &state)
            .unwrap_err()
            .current_context(),
        BuiltinError::TestNonInteger
    ));
}

#[test]
fn malformed_expressions() {
    let state = ShellState::new();
    let cases: &[&[&str]] = &[
        &["a", "b", "c", "d"],
        &["a", "b"],
        &["a", "~", "b"],
        &["-nt", "a"], // a file-binary op with a single operand is not unary
    ];
    for args in cases {
        assert!(
            matches!(
                run(args, &state).unwrap_err().current_context(),
                BuiltinError::TestUsage
            ),
            "expected TestUsage for {args:?}"
        );
    }
}

#[test]
fn unary_and_binary_predicates() {
    let unary: &[&[u8]] = &[
        b"-e", b"-f", b"-d", b"-b", b"-c", b"-p", b"-S", b"-L", b"-s", b"-r", b"-w", b"-x", b"-g",
        b"-k", b"-t", b"-z", b"-n",
    ];
    for op in unary {
        assert!(super::ops::is_unary(op), "is_unary({op:?})");
    }
    assert!(!super::ops::is_unary(b"-nt"));
    assert!(!super::ops::is_unary(b"-ef"));
    let binary: &[&[u8]] = &[b"-nt", b"-ot", b"-ef", b"-fdeq", b"-fdne"];
    for op in binary {
        assert!(super::ops::is_file_binary(op), "is_file_binary({op:?})");
    }
    assert!(!super::ops::is_file_binary(b"-eq"));
    assert!(!super::ops::is_file_binary(b"-e"));
}

#[test]
fn unknown_ops_are_never() {
    let s = fst(S_IFREG, 1, 0, 1, 1);
    // string_test only handles -z/-n.
    assert!(matches!(
        super::ops::string_test(b"-f", c"x")
            .unwrap_err()
            .current_context(),
        BuiltinError::Never
    ));
    // stat_test only handles the stat-derived file ops.
    assert!(matches!(
        super::filetest::stat_test(b"-z", Some(&s))
            .unwrap_err()
            .current_context(),
        BuiltinError::Never
    ));
    // binary_op only handles the file-binary ops.
    assert!(matches!(
        super::filetest::binary_op(b"-eq", &s, &s)
            .unwrap_err()
            .current_context(),
        BuiltinError::Never
    ));
}

#[test]
fn kind_and_mode_ops() {
    // A kind op is true only for its own S_IF* type.
    let kinds: [(&[u8], u32); 7] = [
        (b"-f", S_IFREG),
        (b"-d", S_IFDIR),
        (b"-b", S_IFBLK),
        (b"-c", S_IFCHR),
        (b"-p", S_IFIFO),
        (b"-S", S_IFSOCK),
        (b"-L", S_IFLNK),
    ];
    for (op, want) in kinds {
        let hit = fst(want, 0, 0, 1, 1);
        let miss = fst(S_IFREG, 0, 0, 1, 1);
        assert!(
            super::filetest::stat_test(op, Some(&hit)).unwrap(),
            "{op:?} on {want:#o}"
        );
        if want != S_IFREG {
            assert!(
                !super::filetest::stat_test(op, Some(&miss)).unwrap(),
                "{op:?} on wrong type"
            );
        }
    }
    // -e is true for any stat result.
    assert!(super::filetest::stat_test(b"-e", Some(&fst(S_IFSOCK, 0, 0, 1, 1))).unwrap());
    // Size: 0 is false, >0 is true.
    assert!(!super::filetest::stat_test(b"-s", Some(&fst(S_IFREG, 0, 0, 1, 1))).unwrap());
    assert!(super::filetest::stat_test(b"-s", Some(&fst(S_IFREG, 1, 0, 1, 1))).unwrap());
    // Mode bits.
    assert!(super::filetest::stat_test(b"-g", Some(&fst(S_IFREG | S_ISGID, 0, 0, 1, 1))).unwrap());
    assert!(!super::filetest::stat_test(b"-g", Some(&fst(S_IFREG, 0, 0, 1, 1))).unwrap());
    assert!(super::filetest::stat_test(b"-k", Some(&fst(S_IFREG | S_ISVTX, 0, 0, 1, 1))).unwrap());
    assert!(!super::filetest::stat_test(b"-k", Some(&fst(S_IFREG, 0, 0, 1, 1))).unwrap());
    // A `None` stat is false for every stat-derived op.
    assert!(!super::filetest::stat_test(b"-f", None).unwrap());
    assert!(!super::filetest::stat_test(b"-e", None).unwrap());
}

#[test]
fn file_tests_on_paths() {
    let state = ShellState::new();
    let file = tmp("path-file");
    std::fs::write(&file, b"x").unwrap();
    let link = tmp("path-link");
    symlink(&file, &link).unwrap();
    let dir = std::env::temp_dir();

    let fs = file.to_str().unwrap();
    let ls = link.to_str().unwrap();
    let ds = dir.to_str().unwrap();
    assert_eq!(run(&["-f", fs], &state).unwrap(), 0);
    assert_eq!(run(&["-d", ds], &state).unwrap(), 0);
    assert_eq!(run(&["-e", fs], &state).unwrap(), 0);
    // /dev/null is a character device.
    assert_eq!(run(&["-c", "/dev/null"], &state).unwrap(), 0);
    assert_eq!(run(&["-b", "/dev/null"], &state).unwrap(), 1);
    // A symlink is `-L`; `-f`/`-d` follow it to the (regular) target.
    assert_eq!(run(&["-L", ls], &state).unwrap(), 0);
    assert_eq!(run(&["-f", ls], &state).unwrap(), 0);
    assert_eq!(run(&["-d", ls], &state).unwrap(), 1);
    // Non-empty file is `-s`; a directory is not a regular file.
    assert_eq!(run(&["-s", fs], &state).unwrap(), 0);
    assert_eq!(run(&["-f", ds], &state).unwrap(), 1);
    assert_eq!(
        run(&["-e", "/nonexistent-fdshell-test"], &state).unwrap(),
        1
    );
    // `lstat` on a missing path is `None`, so `-L` (and every other kind) is false.
    assert_eq!(
        run(&["-L", "/nonexistent-fdshell-test"], &state).unwrap(),
        1
    );
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn file_tests_on_fd_vars() {
    let mut state = ShellState::new();
    let file = tmp("fd-file");
    std::fs::write(&file, b"x").unwrap();
    let link = tmp("fd-link");
    symlink(&file, &link).unwrap();
    let dir = std::env::temp_dir();

    let file_fd = sys::openat2::open(cstr(&file), O_RDONLY).unwrap();
    let link_fd = sys::openat2::open(cstr(&link), O_RDONLY).unwrap();
    let dir_fd = sys::openat2::open(cstr(&dir), O_DIRECTORY).unwrap();
    ins(&mut state, c"f", file_fd);
    ins(&mut state, c"l", link_fd);
    ins(&mut state, c"d", dir_fd);

    // Without the original `%var`, the substituted value is just a path.
    assert_eq!(run(&["-f", "0"], &state).unwrap(), 1);

    assert_eq!(
        with_refs(&["-f", "%f"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-d", "%d"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-e", "%f"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-s", "%f"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    // Opening a symlink follows it: `-L` is false, `-f` is true.
    assert_eq!(
        with_refs(&["-L", "%l"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
    assert_eq!(
        with_refs(&["-f", "%l"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    // An unset variable is not a file.
    assert_eq!(
        with_refs(&["-e", "%missing"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn permission_tests_on_paths() {
    let state = ShellState::new();
    let rw = tmp("perm-rw");
    make_file(&rw, 0o666);
    let x = tmp("perm-x");
    make_file(&x, 0o111);
    let rws = rw.to_str().unwrap();
    let xs = x.to_str().unwrap();
    assert_eq!(run(&["-r", rws], &state).unwrap(), 0);
    assert_eq!(run(&["-w", rws], &state).unwrap(), 0);
    assert_eq!(run(&["-x", rws], &state).unwrap(), 1);
    assert_eq!(run(&["-x", xs], &state).unwrap(), 0);
    let _ = std::fs::remove_file(&rw);
    let _ = std::fs::remove_file(&x);
}

#[test]
fn permission_tests_on_fd_vars() {
    let mut state = ShellState::new();
    let rw = tmp("perm-fd");
    make_file(&rw, 0o666);
    let fd = sys::openat2::open(cstr(&rw), O_RDONLY).unwrap();
    ins(&mut state, c"p", fd);
    // A 0666 file has read+write but no execute bit: `-x` is false for all uids.
    assert_eq!(
        with_refs(&["-r", "%p"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-w", "%p"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-x", "%p"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
    let _ = std::fs::remove_file(&rw);
}

#[test]
fn tty_test_false_for_regular_file() {
    let mut state = ShellState::new();
    let file = tmp("tty-file");
    std::fs::write(&file, b"x").unwrap();
    let fd = sys::openat2::open(cstr(&file), O_RDONLY).unwrap();
    ins(&mut state, c"f", fd);
    // A regular file is not a terminal; unset vars and paths are not either.
    assert_eq!(
        with_refs(&["-t", "%f"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
    assert_eq!(
        with_refs(&["-t", "%missing"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
    assert_eq!(run(&["-t", "0"], &state).unwrap(), 1);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn tty_test_true_for_pty() {
    let mut state = ShellState::new();
    let (_master, slave) = sys::pty::openpty().unwrap();
    ins(&mut state, c"pty", slave);
    // The pty slave is a terminal (and a character device), not a regular file.
    assert_eq!(
        with_refs(&["-t", "%pty"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-c", "%pty"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-f", "%pty"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
}

#[test]
fn binary_mtime_tests() {
    let state = ShellState::new();
    let (fa, fb, fc) = (tmp("mt-a"), tmp("mt-b"), tmp("mt-c"));
    touch(&fa, 100);
    touch(&fb, 200);
    touch(&fc, 100); // same mtime as fa
    let (pa, pb, pc) = (
        fa.to_str().unwrap(),
        fb.to_str().unwrap(),
        fc.to_str().unwrap(),
    );
    assert_eq!(
        run(&[pb, "-nt", pa], &state).unwrap(),
        0,
        "fb newer than fa"
    );
    assert_eq!(
        run(&[pa, "-ot", pb], &state).unwrap(),
        0,
        "fa older than fb"
    );
    assert_eq!(run(&[pa, "-nt", pb], &state).unwrap(), 1);
    assert_eq!(run(&[pb, "-ot", pa], &state).unwrap(), 1);
    // Equal mtime: neither newer nor older.
    assert_eq!(run(&[pa, "-nt", pc], &state).unwrap(), 1);
    assert_eq!(run(&[pa, "-ot", pc], &state).unwrap(), 1);
    for p in [&fa, &fb, &fc] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn binary_same_inode_tests() {
    let mut state = ShellState::new();
    let fa = tmp("same-a");
    let fb = tmp("same-b");
    std::fs::write(&fa, b"x").unwrap();
    std::fs::write(&fb, b"y").unwrap();

    // Same file opened twice shares (dev, ino); the two files do not.
    ins(
        &mut state,
        c"a1",
        sys::openat2::open(cstr(&fa), O_RDONLY).unwrap(),
    );
    ins(
        &mut state,
        c"a2",
        sys::openat2::open(cstr(&fa), O_RDONLY).unwrap(),
    );
    ins(
        &mut state,
        c"b1",
        sys::openat2::open(cstr(&fb), O_RDONLY).unwrap(),
    );

    assert_eq!(
        with_refs(&["%a1", "-ef", "%a2"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["%a1", "-fdeq", "%a2"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["%a1", "-fdne", "%a1"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
    assert_eq!(
        with_refs(&["%a1", "-ef", "%b1"], |r, o| eval(r, o, &state)).unwrap(),
        1
    );
    assert_eq!(
        with_refs(&["%a1", "-fdne", "%b1"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    // Path form of the same comparison.
    let (sa, sb) = (fa.to_str().unwrap(), fb.to_str().unwrap());
    assert_eq!(run(&[sa, "-ef", sa], &state).unwrap(), 0);
    assert_eq!(run(&[sa, "-ef", sb], &state).unwrap(), 1);
    // One unstat-able operand makes the comparison false, not an error.
    assert_eq!(
        run(&["/nonexistent-fdshell-test", "-ef", sa], &state).unwrap(),
        1
    );
    // A pipe's two ends share a single inode.
    let (pr, pw) = sys::pipe::pipe2(0).unwrap();
    ins(&mut state, c"pr", pr);
    ins(&mut state, c"pw", pw);
    assert_eq!(
        with_refs(&["%pr", "-ef", "%pw"], |r, o| eval(r, o, &state)).unwrap(),
        0
    );
    let _ = std::fs::remove_file(&fa);
    let _ = std::fs::remove_file(&fb);
}
