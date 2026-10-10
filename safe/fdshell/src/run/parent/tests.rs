#![allow(clippy::unwrap_used)]
use super::*;
use crate::error::redirect::OpenRedirectError;
use alloc::format;
use alloc::string::{String, ToString};

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn st(b: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(b.to_vec()).unwrap(),
        sys::Position::new(1, 1),
        sys::Origin::Shell,
    )
}

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("fdshell_parent_{tag}_{}", std::process::id()))
        .to_str()
        .unwrap()
        .to_string()
}

fn cstr(s: &String) -> sys::ShortCStr {
    sys::ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

/// Open `path` onto the shell fd `target` (`dup2` clears `CLOEXEC`, so `target`
/// is a plain shell fd like `exec 9> file` leaves it).
fn open_at(path: &String, target: i32) {
    let name = sys::ExportedCStr::from(cstr(path));
    sys::openat2::open(
        &name,
        sys::fcntl::O_RDWR + sys::fcntl::O_CREAT + sys::fcntl::O_TRUNC,
    )
    .unwrap()
    .export_to(target)
    .unwrap();
}

/// What fd `n` points at (`None` when it is closed): `/proc/self/fd/N` is a
/// symlink to the open file.
fn link(n: i32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/self/fd/{n}")).ok()
}

/// The fd the scope's saved copy lands on. The copy is the only fd at/above
/// `min_fd` that points at the shell's **original** file while the scope is open
/// (the opened redirect file, its re-home and the resolved source all point at
/// the redirect target), so measure it by opening the same scope once. The
/// restore puts the fd table back to the state the script below sees, so that
/// run allocates the same fds in the same order and its copy lands here.
fn saved_copy_fd(orig: &String, target: i32, path: &String, cell: &ForkCell<ShellState>) -> i32 {
    let scope = crate::redirect::Scope::open(
        &[crate::redirect::RedirectDef::write_path(target, cstr(path))],
        cell,
    )
    .unwrap();
    let original = std::path::PathBuf::from(orig);
    let copy = (target + 1..64)
        .find(|n| link(*n) == Some(original.clone()))
        .unwrap();
    scope.restore().unwrap();
    copy
}

/// POSIX #2.4 on the parent-side handler: the scope's saved copy **is** a
/// script-nameable fd, so a function body can close it and the parent's
/// `scope.restore()?` (`run/parent.rs:40`) fails the command. Measured in the
/// shell binary with strace for the `f(){ :; }; f >copy_probe` case: the copy of
/// fd 1 lands at fd 6 (`fcntl(1, F_DUPFD_CLOEXEC, 2) = 6`), and
/// `fdshell -c 'f(){ echo ok; exec 6>&-; }; f >q'` reports rc 1 with the
/// two-frame `redirection failed` + `EBADF` at `redirect/scope.rs:60`, while the
/// control `f(){ echo ok; }; f >q` is rc 0. bash 5.3.9 gives rc 0 for the same
/// script: its `-c` fd table is 0-2,3 and its save sits at fd >= 10, outside the
/// script-visible range. The floor that makes the copy unreachable is task #184.
#[test]
fn body_that_closes_the_saved_copy_fails_the_command() {
    let orig = temp_path("orig");
    let target = temp_path("target");
    open_at(&orig, 9);
    let copy = saved_copy_fd(&orig, 9, &target, &cell());
    let script = format!("f(){{ exec {copy}>&-; }}; f 9> {target}");
    let cell = cell();
    let report = crate::script::run_script(&st(script.as_bytes()), &cell)
        .err()
        .unwrap();
    assert_eq!(
        report.current_context().to_string(),
        "redirection failed",
        "report: {report}"
    );
    // The frame shape is the distinction a plain rc-1 sweep cannot see: closing a
    // *closed* fd in the body fails at the `exec`'s own close and gains the middle
    // `OpenRedirectError::CloseFd` frame ("failed to close redirection target fd N",
    // `redirect.rs:48`). Here there is no such frame, so the EBADF comes from the
    // restore's `dup2` and the report has exactly the two contexts.
    assert_eq!(
        report.frames().filter(|f| f.is::<CmdError>()).count(),
        1,
        "the redirect error is the command's context: {report}"
    );
    assert_eq!(
        report
            .frames()
            .filter(|f| f.is::<OpenRedirectError>())
            .count(),
        0,
        "no `failed to close redirection target fd N` middle frame: {report}"
    );
    assert_eq!(
        report
            .frames()
            .filter(|f| f.is::<sys::SyscallError>())
            .count(),
        1,
        "the single source is the restore's EBADF: {report}"
    );
    assert_eq!(
        link(9),
        Some(std::path::PathBuf::from(&target)),
        "the redirect stays applied: the restore never put fd 9 back on `orig`"
    );
    assert_eq!(
        link(copy),
        None,
        "the body closed the saved copy, so the restore had no source (fd {copy})"
    );
    let body = std::fs::read_to_string(&target).unwrap();
    let _ = std::fs::remove_file(&target);
    let _ = std::fs::remove_file(&orig);
    let _ = sys::dup::close(9);
    assert_eq!(body, "", "the redirect created the target file");
}
