#![allow(clippy::unwrap_used)]
use super::*;
use crate::error::redirect::OpenRedirectError;
use alloc::format;
use alloc::string::{String, ToString};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). The pid-named temp files live inside this dir now. Removal is
/// `.ok()`-tolerant: teardown never panics, and it runs on a failing test's
/// unwind, which is exactly the test that used to leak.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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

/// A pid + counter scratch dir named after `tag` (the old
/// `fdshell_parent_<tag>_<pid>` files live inside it now); returns the path
/// of a file in it plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell_parent_{tag}_{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("f").to_str().unwrap().to_string(), Scratch(dir))
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

/// POSIX #2.4 on the parent-side handler: closing the scope's saved copy in a
/// function body fails the parent's `scope.restore()?` (`run/parent.rs`). This is the
/// proof of the **mechanism**, so the copy fd is measured (`saved_copy_fd`), never
/// hardcoded: a script cannot name it any more — every fd the shell allocates for
/// itself sits at or above `MIN_SHELL_FD`, out of the script-visible range, and the
/// script-level pin of that floor is
/// `tests/redirect_scope.rs::body_that_closes_the_saved_copy_above_the_floor_fails_the_restore`
/// — but a test can reach it and pin the report shape. The control
/// `f(){ echo ok; }; f 9> target` is rc 0.
#[test]
fn body_that_closes_the_saved_copy_fails_the_command() {
    let (orig, _so) = temp_path("orig");
    let (target, _st) = temp_path("target");
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
    // The frame shape is the distinction a plain rc-1 sweep cannot see: the body
    // closes the saved copy, a fd that **is** open, so the close succeeds and gains
    // no `OpenRedirectError::CloseFd` frame ("failed to close redirection target fd
    // N", the `Close` arm of `Redirect::export`) — that frame belongs to a close of a
    // live fd that failed another errno, never to a not-open fd, which is silent.
    // Here the EBADF comes from the restore's `dup2` and the report has exactly the
    // two contexts.
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
    let _ = sys::dup::close(9);
    assert_eq!(body, "", "the redirect created the target file");
}
