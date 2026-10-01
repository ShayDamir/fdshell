#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use builtins::utimensat::parse::TimeSpec;
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
        |a| match builtins::utimensat::parse::utimensat_parse(a) {
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

fn assert_ok<F: FnOnce(&builtins::utimensat::parse::UtimensatConfig)>(args: &[&str], f: F) {
    with_args(
        args,
        |a| match builtins::utimensat::parse::utimensat_parse(a) {
            Ok(cfg) => f(&cfg),
            Err(e) => panic!("expected Ok, got Err({e})"),
        },
    );
}

#[test]
fn basic_defaults_now() {
    assert_ok(&["f"], |cfg| {
        assert!(cfg.dirfd.is_none());
        assert_eq!(cfg.atime, TimeSpec::Now);
        assert_eq!(cfg.mtime, TimeSpec::Now);
        assert_eq!(cfg.flags, 0);
        assert_eq!(cfg.path.to_bytes(), b"f");
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
    assert_invalid_arg(&["--bad", "f"]);
}

#[test]
fn short_flag_as_path() {
    assert_invalid_arg(&["-x"]);
}

#[test]
fn atime_now() {
    assert_ok(&["--atime", "now", "f"], |cfg| {
        assert_eq!(cfg.atime, TimeSpec::Now);
    });
}

#[test]
fn atime_omit() {
    assert_ok(&["--atime", "omit", "f"], |cfg| {
        assert_eq!(cfg.atime, TimeSpec::Omit);
    });
}

#[test]
fn atime_epoch() {
    assert_ok(&["--atime", "1234567890", "f"], |cfg| {
        assert_eq!(cfg.atime, TimeSpec::Epoch(1234567890));
    });
}

#[test]
fn mtime_negative_epoch() {
    assert_ok(&["--mtime", "-5", "f"], |cfg| {
        assert_eq!(cfg.mtime, TimeSpec::Epoch(-5));
    });
}

#[test]
fn mtime_eq_syntax() {
    assert_ok(&["--mtime=omit", "f"], |cfg| {
        assert_eq!(cfg.mtime, TimeSpec::Omit);
    });
}

/// Pins the `TimeSpec -> timespec` encoding: the kernel requires `tv_sec == 0`
/// for the special values, and epoch specs carry zero nanoseconds.
#[test]
fn to_timespec_encoding() {
    let now = TimeSpec::Now.to_timespec();
    assert_eq!(now.tv_sec, 0);
    assert_eq!(now.tv_nsec, sys::fileat::UTIME_NOW);

    let omit = TimeSpec::Omit.to_timespec();
    assert_eq!(omit.tv_sec, 0);
    assert_eq!(omit.tv_nsec, sys::fileat::UTIME_OMIT);

    let epoch = TimeSpec::Epoch(1234567890).to_timespec();
    assert_eq!(epoch.tv_sec, 1234567890);
    assert_eq!(epoch.tv_nsec, 0);

    let neg = TimeSpec::Epoch(-5).to_timespec();
    assert_eq!(neg.tv_sec, -5);
    assert_eq!(neg.tv_nsec, 0);
}

#[test]
fn atime_bad_spec() {
    assert_invalid_arg(&["--atime", "banana", "f"]);
}

#[test]
fn mtime_bad_spec() {
    assert_invalid_arg(&["--mtime", "12x34", "f"]);
}

#[test]
fn flags_nofollow() {
    assert_ok(&["--flags", "AT_SYMLINK_NOFOLLOW", "f"], |cfg| {
        assert_eq!(cfg.flags, sys::fileat::AT_SYMLINK_NOFOLLOW);
    });
}

#[test]
fn flags_hex() {
    assert_ok(&["--flags", "0x100", "f"], |cfg| {
        assert_eq!(cfg.flags, sys::fileat::AT_SYMLINK_NOFOLLOW);
    });
}

#[test]
fn flags_unknown() {
    assert_invalid_arg(&["--flags", "AT_REMOVEDIR", "f"]);
}

#[test]
fn missing_path() {
    assert_invalid_arg(&["--atime", "now"]);
}

#[test]
fn extra_positional() {
    assert_invalid_arg(&["a", "b"]);
}

#[test]
fn empty_path() {
    assert_invalid_arg(&[""]);
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

// --- exec (filesystem) ------------------------------------------------------

fn run_exec(args: &[&str]) -> Result<(), Report<BuiltinError>> {
    let owned: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    let cfg = builtins::utimensat::parse::utimensat_parse(&refs).unwrap();
    builtins::utimensat::utimensat_exec(&cfg)
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
const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn scratch() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-utimensat-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// mtime in epoch seconds, following symlinks (stat).
fn mtime_secs(path: &std::path::Path) -> i64 {
    let st = std::fs::metadata(path).unwrap().modified().unwrap();
    st.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

/// atime in epoch seconds, following symlinks (stat).
fn atime_secs(path: &std::path::Path) -> i64 {
    let st = std::fs::metadata(path).unwrap().accessed().unwrap();
    st.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

/// mtime in epoch seconds of the link itself (lstat).
fn lmtime_secs(path: &std::path::Path) -> i64 {
    let st = std::fs::symlink_metadata(path).unwrap().modified().unwrap();
    st.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

#[test]
fn exec_sets_mtime() {
    let dir = scratch();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    run_exec(&["--mtime", "1234567890", file.to_str().unwrap()]).unwrap();
    assert_eq!(mtime_secs(&file), 1234567890);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_now_is_current() {
    let dir = scratch();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    run_exec(&["--mtime", "now", file.to_str().unwrap()]).unwrap();
    let m = mtime_secs(&file);
    let real = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(
        (m - real).abs() <= 5,
        "mtime {m} not within 5s of now {real}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_omit_leaves_atime() {
    let dir = scratch();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    // Pin both to a known epoch, then move only mtime (atime omitted).
    run_exec(&[
        "--atime",
        "1111111111",
        "--mtime",
        "1111111111",
        file.to_str().unwrap(),
    ])
    .unwrap();
    assert_eq!(atime_secs(&file), 1111111111);
    run_exec(&[
        "--atime",
        "omit",
        "--mtime",
        "2222222222",
        file.to_str().unwrap(),
    ])
    .unwrap();
    assert_eq!(
        atime_secs(&file),
        1111111111,
        "atime must be untouched by omit"
    );
    assert_eq!(mtime_secs(&file), 2222222222, "mtime must be set");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_nofollow_updates_link_not_target() {
    let dir = scratch();
    let target = dir.join("target");
    std::fs::write(&target, b"x").unwrap();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    // Pin the target's mtime to a distinct value so the two can be told apart.
    run_exec(&[
        "--atime",
        "2222222222",
        "--mtime",
        "2222222222",
        target.to_str().unwrap(),
    ])
    .unwrap();
    // Update the link's own mtime (not the target's).
    run_exec(&[
        "--flags",
        "AT_SYMLINK_NOFOLLOW",
        "--mtime",
        "1234567890",
        link.to_str().unwrap(),
    ])
    .unwrap();
    assert_eq!(lmtime_secs(&link), 1234567890, "link mtime must be set");
    assert_eq!(
        mtime_secs(&target),
        2222222222,
        "target mtime must be untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_missing_path_is_enoent() {
    let dir = scratch();
    let missing = dir.join("nope");
    let e = run_exec(&["--mtime", "now", missing.to_str().unwrap()]).unwrap_err();
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
    let e = run_exec(&["--dirfd", &s, "--mtime", "now", "f"]).unwrap_err();
    assert_eq!(errno_of(&e), ENOTDIR);
    drop(fd);
    let _ = std::fs::remove_dir_all(&dir);
}
