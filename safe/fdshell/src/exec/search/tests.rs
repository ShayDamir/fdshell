#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::format;
use alloc::string::{String, ToString};
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::AtomicU64;

use hashbrown::HashMap;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fcntl::O_DIRECTORY;
use sys::fcntl::O_PATH;

use crate::error::child_process::ChildProcessError;
use crate::exec::search::{open_checked, search_path_str};
use crate::exec::{resolve_path, resolve_path_str};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch dir holding an executable `mybin`. The PATH *loop* itself (its
/// entries, their order) is read from the process environment and is pinned by
/// the integration tests in `tests/strict.rs`; these unit tests take the search
/// base fd directly, so they need no environment manipulation (the safe crates
/// forbid the `unsafe` `std::env::set_var`).
fn scratch() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-search-{}-{}", std::process::id(), c));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let helper = dir.join("mybin");
    std::fs::write(&helper, b"#!/bin/sh\necho SEARCH-OK\n").unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn cstr(path: &str) -> CString {
    CString::new(path).unwrap()
}

fn dir_fd(dir: &std::path::Path) -> LocalFd {
    sys::openat2::open(cstr(&dir.to_string_lossy()), O_DIRECTORY).unwrap()
}

/// An existing absolute pathname of a regular file, for the non-strict arms.
fn manifest_file() -> String {
    format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))
}

fn word(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn table(key: &str, pinned: &str) -> HashMap<ShortCStr, ShortCStr> {
    [(word(key), word(pinned))].into_iter().collect()
}

/// Strict bans an absolute program word containing `/`.
#[test]
fn strict_bans_absolute_program_word() {
    let err = resolve_path_str(&word("/bin/sh"), &HashMap::new(), true, None)
        .err()
        .unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::StrictPath(_)
    ));
    // The same word resolves untouched when strict is off.
    let name = resolve_path_str(&word("/bin/sh"), &HashMap::new(), false, None).unwrap();
    assert_eq!(name.as_bytes().unwrap(), b"/bin/sh");
}

/// Strict bans an absolute pinned path *before* the PATH search, so a strict
/// run never resolves a pinned program from the filesystem root.
#[test]
fn strict_bans_absolute_pin() {
    let t = table("prog", "/bin/sh");
    let err = resolve_path_str(&word("prog"), &t, true, None)
        .err()
        .unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::StrictPath(_)
    ));
}

/// Strict bans an absolute PATH entry (the ban rides inside the search loop,
/// naming the entry); the entry name is what the error carries.
#[test]
fn strict_bans_absolute_path_entry() {
    let err = search_path_str(&word("/usr/bin/ls"), true, None)
        .err()
        .unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::StrictPath(_)
    ));
}

/// A relative lookup under strict resolves against the base fd, and the
/// returned pin is the joined relative pathname (a `.` entry contributes no
/// `./` prefix — that string is what `hash` pins and `type` prints).
#[test]
fn strict_relative_join_needs_no_dot_prefix() {
    let dir = scratch();
    let base = dir_fd(&dir);
    let t = table("prog", "mybin");
    let name = resolve_path_str(&word("prog"), &t, true, Some(&base)).unwrap();
    assert_eq!(name.as_bytes().unwrap(), b"mybin");
    let t = table("prog", "sub/mybin");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::copy(dir.join("mybin"), dir.join("sub").join("mybin")).unwrap();
    let name = resolve_path_str(&word("prog"), &t, true, Some(&base)).unwrap();
    assert_eq!(name.as_bytes().unwrap(), b"sub/mybin");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The strict base: a relative pathname opens through the pinned dirfd; with
/// no `%CWD` fd (defensive) it falls back to the process CWD.
#[test]
fn strict_base_falls_back_to_process_cwd() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let base = dir_fd(&dir);
    assert!(open_checked(&word("Cargo.toml"), true, Some(&base)).is_ok());
    assert!(open_checked(&word("Cargo.toml"), true, None).is_ok());
}

/// Strict bans an absolute pathname at the open site too; non-strict opens it
/// verbatim (the pre-strict behavior, byte-for-byte).
#[test]
fn open_checked_bans_absolute_only_under_strict() {
    let abs = word(&manifest_file());
    let err = open_checked(&abs, true, None).err().unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::StrictPath(_)
    ));
    assert!(open_checked(&abs, false, None).is_ok());
}

/// Non-strict `resolve_path` opens the resolved path (the ban is inert).
#[test]
fn non_strict_resolve_path_opens_absolute() {
    let abs = word(&manifest_file());
    let fd = resolve_path(&abs, &HashMap::new(), false, None);
    assert!(fd.is_ok());
    let err = resolve_path(&abs, &HashMap::new(), true, None)
        .err()
        .unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::StrictPath(_)
    ));
}

/// A pin that no longer opens falls back to the PATH search (self-heal), which
/// reports `NotFound` for a name the search cannot resolve.
#[test]
fn stale_pin_falls_back_to_path_search() {
    let t = table("fdshell-absent-task136", "/nonexistent-fdshell-task136/x");
    let err = resolve_path_str(&word("fdshell-absent-task136"), &t, false, None)
        .err()
        .unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::NotFound(_)
    ));
}

/// An empty name is never "found" through a directory entry (`dir/` opens).
#[test]
fn empty_name_is_not_found() {
    let err = search_path_str(&ShortCStr::new(), false, None)
        .err()
        .unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::NotFound(_)
    ));
    let err = search_path_str(&ShortCStr::new(), true, None)
        .err()
        .unwrap();
    assert!(matches!(
        err.current_context(),
        ChildProcessError::NotFound(_)
    ));
}

/// Non-strict resolves a relative program path against the process CWD, in a
/// forked child so only the child's CWD moves (LESSONS: test children exit
/// with `sys::exit`, and a forked harness swallows the child's panics unless
/// caught). Strict mode has no process-CWD route: the same word opens through
/// the pinned base fd, from any CWD.
#[test]
fn relative_program_path_follows_the_process_cwd() {
    let dir = scratch();
    let dir = dir.to_string_lossy().to_string();
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            let check = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sys::openat2::open(cstr(&dir), O_DIRECTORY)
                    .unwrap()
                    .fchdir()
                    .unwrap();
                let rel = word("./mybin");
                assert_eq!(
                    resolve_path_str(&rel, &HashMap::new(), false, None)
                        .unwrap()
                        .as_bytes()
                        .unwrap(),
                    b"./mybin"
                );
                assert!(resolve_path(&rel, &HashMap::new(), false, None).is_ok());
                // The same relative form opens through a base fd when strict.
                let base = dir_fd(std::path::Path::new(&dir));
                assert!(open_checked(&rel, true, Some(&base)).is_ok());
            }));
            sys::exit(i32::from(check.is_err()));
        }
        Some(pidfd) => {
            let status = pidfd.wait_pidfd().unwrap();
            assert_eq!(
                status.exit_code(),
                0,
                "child failed, exit={}",
                status.exit_code()
            );
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// The pinned `%CWD` fd is strict mode's resolution base; `ShellState::new()`
/// has none (startup seeds it), and `strict` is off by default.
#[test]
fn shell_state_exposes_the_strict_base() {
    use crate::state::ShellState;
    let state = ShellState::new();
    assert!(state.cwd_fd().is_none());
    assert!(!state.strict());
    let dir = scratch();
    let mut state = ShellState::new();
    state.insert_cwd(dir_fd(&dir));
    assert!(state.cwd_fd().is_some());
    state.options |= crate::options::STRICT;
    assert!(state.strict());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The strict base is a `LocalFd` borrowed as an `AtFd` view, and the path
/// argument rides as an exported (NUL-terminated) string.
#[test]
fn base_fd_opens_the_target_through_openat2() {
    let dir = scratch();
    let base = dir_fd(&dir);
    let name = word("mybin").export();
    let fd = sys::openat2::openat2(
        sys::AtFd::from(&base),
        &name,
        &sys::openat2::OpenHow::new(O_PATH as u64, 0),
    );
    assert!(fd.is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
