#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::Report;
use std::ffi::CString;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(
        args,
        |a| match builtins::symlinkat::parse::symlinkat_parse(a) {
            Err(e) => {
                let ctx = e.current_context();
                match (ctx, expected) {
                    (BuiltinError::Help, BuiltinError::Help) => {}
                    (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                    _ => panic!("unexpected error: {ctx}"),
                }
            }
            _ => panic!("expected Err"),
        },
    );
}

fn assert_invalid_arg(args: &[&str]) {
    assert_err(args, BuiltinError::InvalidArgument("x"));
}

fn assert_ok<F: FnOnce(&builtins::symlinkat::parse::SymlinkatConfig)>(args: &[&str], f: F) {
    with_args(
        args,
        |a| match builtins::symlinkat::parse::symlinkat_parse(a) {
            Ok(cfg) => f(&cfg),
            Err(e) => panic!("expected Ok, got Err({e})"),
        },
    );
}

#[test]
fn basic() {
    assert_ok(&["target", "link"], |cfg| {
        assert!(cfg.dirfd.is_none());
        assert_eq!(cfg.target.to_bytes(), b"target");
        assert_eq!(cfg.linkpath.to_bytes(), b"link");
    });
}

#[test]
fn help_long() {
    assert_err(&["--help"], BuiltinError::Help);
}

#[test]
fn help_short() {
    assert_err(&["-h"], BuiltinError::Help);
}

#[test]
fn empty_args() {
    assert_err(&[], BuiltinError::Help);
}

#[test]
fn bad_flag() {
    assert_invalid_arg(&["--bad", "x", "y"]);
}

#[test]
fn short_flag_as_path() {
    assert_invalid_arg(&["-x", "y"]);
}

#[test]
fn missing_linkpath() {
    assert_invalid_arg(&["target"]);
}

#[test]
fn extra_positional() {
    assert_invalid_arg(&["a", "b", "c"]);
}

#[test]
fn empty_target() {
    assert_invalid_arg(&["", "link"]);
}

#[test]
fn empty_linkpath() {
    assert_invalid_arg(&["target", ""]);
}

#[test]
fn missing_value() {
    assert_invalid_arg(&["--dirfd", "AT_FDCWD"]);
}

#[test]
fn dirfd_ateq() {
    assert_ok(&["--dirfd=AT_FDCWD", "t", "l"], |cfg| {
        assert!(cfg.dirfd.is_none());
    });
}

#[test]
fn dirfd_numeric() {
    let (rd, wr) = sys::pipe::pipe2(0).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let dupfd = rd.export().unwrap();
    dupfd.verify().unwrap();
    let s = format!("{}", dupfd.as_raw());
    assert_ok(&["--dirfd", &s, "t", "l"], |cfg| {
        assert_eq!(cfg.dirfd.as_ref().map(|d| d.as_raw()), Some(dupfd.as_raw()));
    });
}

// --- exec (filesystem) ------------------------------------------------------

fn run_exec(args: &[&str]) -> Result<(), Report<BuiltinError>> {
    let owned: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    let cfg = builtins::symlinkat::parse::symlinkat_parse(&refs).unwrap();
    builtins::symlinkat::symlinkat_exec(&cfg)
}

/// The `SyscallError`'s errno, wherever it sits in the report's chain.
fn errno_of(e: &Report<BuiltinError>) -> i32 {
    let se = e
        .downcast_ref::<sys::SyscallError>()
        .unwrap_or_else(|| panic!("no SyscallError in chain: {e}"));
    se.errno()
}

// Linux errno values, pinned from the plan's C-verified table. `libc` is not a
// `builtins` dependency, so the names are local (the shell is Linux x86_64 only).
const EEXIST: i32 = 17;
const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn scratch() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-symlinkat-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A directory fd for `path`, exported to a number the parse can consume.
fn dirfd_num(path: &std::path::Path) -> String {
    let c = CString::new(path.to_str().unwrap()).unwrap();
    let fd = sys::openat2::openat2(
        sys::AtFd::cwd(),
        c.as_c_str(),
        &sys::openat2::OpenHow::new(sys::fcntl::O_DIRECTORY as u64, 0),
    )
    .unwrap();
    format!("{}", fd.export().unwrap().as_raw())
}

#[test]
fn exec_creates_link() {
    let dir = scratch();
    let link = dir.join("link");
    run_exec(&["target", link.to_str().unwrap()]).unwrap();
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        std::path::Path::new("target")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_stores_content_verbatim() {
    let dir = scratch();
    let link = dir.join("link");
    // A path that does not exist is stored as-is (symlinks are never resolved).
    run_exec(&["no/such/target", link.to_str().unwrap()]).unwrap();
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        std::path::Path::new("no/such/target")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_resolves_against_dirfd() {
    let dir = scratch();
    // The same link name `l` would exist in the scratch root and in `sub`;
    // creating with `sub`'s dirfd puts the link only in the subdir.
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let s = dirfd_num(&sub);
    run_exec(&["--dirfd", &s, "t", "l"]).unwrap();
    // `l` is a symlink to the (absent) `t`, so probe the link itself, not its target.
    assert!(
        std::fs::symlink_metadata(sub.join("l")).is_ok(),
        "sub/l should exist"
    );
    assert!(
        std::fs::symlink_metadata(dir.join("l")).is_err(),
        "root l must not exist"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_existing_link_is_eexist() {
    let dir = scratch();
    let link = dir.join("link");
    run_exec(&["t", link.to_str().unwrap()]).unwrap();
    let e = run_exec(&["t", link.to_str().unwrap()]).unwrap_err();
    assert_eq!(errno_of(&e), EEXIST);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_missing_parent_is_enoent() {
    let dir = scratch();
    let missing = dir.join("nope").join("link");
    let e = run_exec(&["t", missing.to_str().unwrap()]).unwrap_err();
    assert_eq!(errno_of(&e), ENOENT);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_nondirectory_dirfd_is_enotdir() {
    let dir = scratch();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    let c = CString::new(file.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(c.as_c_str(), sys::fcntl::O_RDWR).unwrap();
    let s = format!("{}", fd.export().unwrap().as_raw());
    let e = run_exec(&["--dirfd", &s, "t", "l"]).unwrap_err();
    assert_eq!(errno_of(&e), ENOTDIR);
    drop(fd);
    let _ = std::fs::remove_dir_all(&dir);
}
