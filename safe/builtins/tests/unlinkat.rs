#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::Report;
use std::ffi::CString;
use sys::fileat::AT_REMOVEDIR;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| {
        match builtins::unlinkat::parse::unlinkat_parse(a) {
            Err(e) => {
                let ctx = e.current_context();
                match (ctx, expected) {
                    (BuiltinError::Help, BuiltinError::Help) => {}
                    (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                    _ => panic!("unexpected error: {ctx}"),
                }
            }
            _ => panic!("expected Err"),
        }
    });
}

fn assert_invalid_arg(args: &[&str]) {
    assert_err(args, BuiltinError::InvalidArgument("x"));
}

fn assert_ok<F: FnOnce(&builtins::unlinkat::parse::UnlinkatConfig)>(args: &[&str], f: F) {
    with_args(args, |a| {
        match builtins::unlinkat::parse::unlinkat_parse(a) {
            Ok(cfg) => f(&cfg),
            Err(e) => panic!("expected Ok, got Err({e})"),
        }
    });
}

#[test]
fn basic() {
    assert_ok(&["file"], |cfg| {
        assert!(cfg.dirfd.is_none());
        assert_eq!(cfg.path.to_bytes(), b"file");
        assert_eq!(cfg.flags, 0);
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
    assert_invalid_arg(&["--bad", "x"]);
}

#[test]
fn short_flag_as_path() {
    assert_invalid_arg(&["-x", "y"]);
}

#[test]
fn short_flag_no_path() {
    assert_invalid_arg(&["-x"]);
}

#[test]
fn missing_path() {
    assert_invalid_arg(&["--dirfd", "AT_FDCWD"]);
}

#[test]
fn extra_path() {
    assert_invalid_arg(&["a", "b"]);
}

#[test]
fn empty_path() {
    assert_invalid_arg(&[""]);
}

#[test]
fn missing_value() {
    assert_invalid_arg(&["--flags", "--dirfd", "5", "a"]);
}

#[test]
fn dirfd_ateq() {
    assert_ok(&["--dirfd=AT_FDCWD", "f"], |cfg| {
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
    assert_ok(&["--dirfd", &s, "f"], |cfg| {
        assert_eq!(cfg.dirfd.as_ref().map(|d| d.as_raw()), Some(dupfd.as_raw()));
    });
}

#[test]
fn flags_removedir() {
    assert_ok(&["--flags", "AT_REMOVEDIR", "f"], |cfg| {
        assert_eq!(cfg.flags, AT_REMOVEDIR);
    });
}

#[test]
fn flags_hex() {
    assert_ok(&["--flags", "0x200", "f"], |cfg| {
        assert_eq!(cfg.flags, AT_REMOVEDIR);
    });
}

#[test]
fn flags_eq_syntax() {
    assert_ok(&["--flags=AT_REMOVEDIR", "f"], |cfg| {
        assert_eq!(cfg.flags, AT_REMOVEDIR);
    });
}

#[test]
fn flags_unknown() {
    assert_invalid_arg(&["--flags", "RENAME_NOREPLACE", "f"]);
}

// --- exec (filesystem) ------------------------------------------------------

fn run_exec(args: &[&str]) -> Result<(), Report<BuiltinError>> {
    let owned: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    let cfg = builtins::unlinkat::parse::unlinkat_parse(&refs).unwrap();
    builtins::unlinkat::unlinkat_exec(&cfg)
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
const EISDIR: i32 = 21;
const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn scratch() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-unlinkat-{}-{}", std::process::id(), c));
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
fn exec_unlinks_file() {
    let dir = scratch();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    assert!(file.exists());
    run_exec(&[file.to_str().unwrap()]).unwrap();
    assert!(!file.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_unlinks_dir_with_removedir() {
    let dir = scratch();
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    run_exec(&["--flags", "AT_REMOVEDIR", sub.to_str().unwrap()]).unwrap();
    assert!(!sub.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_dir_without_flag_is_eisdir() {
    let dir = scratch();
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let e = run_exec(&[sub.to_str().unwrap()]).unwrap_err();
    assert_eq!(errno_of(&e), EISDIR);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_missing_path_is_enoent() {
    let dir = scratch();
    let missing = dir.join("nope");
    let e = run_exec(&[missing.to_str().unwrap()]).unwrap_err();
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
    let e = run_exec(&["--dirfd", &s, "victim"]).unwrap_err();
    assert_eq!(errno_of(&e), ENOTDIR);
    drop(fd);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_resolves_against_dirfd() {
    let dir = scratch();
    // The same entry name `g` exists in the scratch root and in `sub`;
    // unlinking with `sub`'s dirfd removes only the subdir's copy.
    std::fs::write(dir.join("g"), b"root").unwrap();
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("g"), b"sub").unwrap();

    let s = dirfd_num(&sub);
    run_exec(&["--dirfd", &s, "g"]).unwrap();

    assert!(!sub.join("g").exists(), "sub/g should be gone");
    assert!(dir.join("g").exists(), "root g must survive");
    let _ = std::fs::remove_dir_all(&dir);
}
