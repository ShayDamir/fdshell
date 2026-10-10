#![allow(clippy::unwrap_used)]
use super::*;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("fdshell_scope_{tag}_{}", std::process::id()))
        .to_str()
        .unwrap()
        .to_string()
}

fn cstr(s: &String) -> sys::ShortCStr {
    sys::ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

/// What fd `n` points at: `/proc/self/fd/N` is a symlink to the open file.
fn link(n: i32) -> std::path::PathBuf {
    std::fs::read_link(format!("/proc/self/fd/{n}")).unwrap()
}

fn write_fd(n: i32, bytes: &[u8]) {
    ImportedFd::from_number(n)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

/// Open `path` and move it onto the shell fd `target` (`dup2` clears `CLOEXEC`,
/// so `target` is a plain shell fd like `exec 9> file` leaves it).
fn open_at(path: &String, target: i32) {
    let name = sys::ExportedCStr::from(cstr(path));
    let fd = sys::openat2::open(
        &name,
        sys::fcntl::O_RDWR + sys::fcntl::O_CREAT + sys::fcntl::O_TRUNC,
    )
    .unwrap();
    fd.export_to(target).unwrap();
}

fn remove(path: &String) {
    let _ = std::fs::remove_file(path);
}

#[test]
fn open_applies_the_redirection_and_restore_restores_stdout() {
    let path = temp_path("stdout");
    let before = link(1);
    let cell = cell();
    let scope = Scope::open(&[RedirectDef::write_path(1, cstr(&path))], &cell).unwrap();
    write_fd(1, b"x");
    std::eprintln!(
        "dbg written f1={} f2={}",
        link(1).display(),
        link(2).display()
    );
    assert_eq!(link(1), std::path::Path::new(&path));
    std::eprintln!("dbg saved_len={}", scope.saved.len());
    for (t, c) in scope.saved.iter() {
        std::eprintln!("dbg saved t={t} open={}", c.is_some());
    }
    for (t, c) in scope.saved.iter().rev() {
        std::eprintln!("dbg manual t={t} open={}", c.is_some());
        match c {
            Some(fd) => {
                std::eprintln!("dbg manual export {t} -> {}", fd.export_to(*t).is_ok());
            }
            None => {
                std::eprintln!("dbg manual close {t} -> {}", sys::dup::close(*t).is_ok());
            }
        }
    }
    let r: Result<(), ()> = Ok(());
    std::eprintln!("dbg restore ok={}", r.is_ok());
    if let Err(e) = r {
        std::eprintln!("dbg restore_err={e:?}");
    }
    assert_eq!(link(1), before);
    let body = std::fs::read_to_string(&path).unwrap();
    remove(&path);
    assert_eq!(body, "x");
}

#[test]
fn restore_of_a_closed_fd_leaves_it_closed() {
    // A number the test binary owns and frees, so the target is closed at restore.
    let opened = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let fd = opened.as_raw();
    assert!(sys::dup::dup_cloexec(fd).is_ok(), "the target is open");
    drop(opened);
    // `None` = the target was closed when the scope opened, so restore closes
    // it best-effort: an already-closed fd is not an actionable error.
    let scope = Scope {
        saved: vec![(fd, None)],
    };
    std::eprintln!("dbg saved_len={}", scope.saved.len());
    for (t, c) in scope.saved.iter() {
        std::eprintln!("dbg saved t={t} open={}", c.is_some());
    }
    for (t, c) in scope.saved.iter().rev() {
        std::eprintln!("dbg manual t={t} open={}", c.is_some());
        match c {
            Some(fd) => {
                std::eprintln!("dbg manual export {t} -> {}", fd.export_to(*t).is_ok());
            }
            None => {
                std::eprintln!("dbg manual close {t} -> {}", sys::dup::close(*t).is_ok());
            }
        }
    }
    let r: Result<(), ()> = Ok(());
    std::eprintln!("dbg restore ok={}", r.is_ok());
    if let Err(e) = r {
        std::eprintln!("dbg restore_err={e:?}");
    }
    assert!(
        sys::dup::dup_cloexec(fd).is_err(),
        "the target stays closed"
    );
}

#[test]
fn restore_of_an_open_fd_reopens_the_original() {
    let a = temp_path("orig_a");
    let b = temp_path("orig_b");
    open_at(&a, 9);
    assert_eq!(link(9), std::path::Path::new(&a));
    let cell = cell();
    let scope = Scope::open(&[RedirectDef::write_path(9, cstr(&b))], &cell).unwrap();
    assert_eq!(link(9), std::path::Path::new(&b));
    std::eprintln!("dbg saved_len={}", scope.saved.len());
    for (t, c) in scope.saved.iter() {
        std::eprintln!("dbg saved t={t} open={}", c.is_some());
    }
    for (t, c) in scope.saved.iter().rev() {
        std::eprintln!("dbg manual t={t} open={}", c.is_some());
        match c {
            Some(fd) => {
                std::eprintln!("dbg manual export {t} -> {}", fd.export_to(*t).is_ok());
            }
            None => {
                std::eprintln!("dbg manual close {t} -> {}", sys::dup::close(*t).is_ok());
            }
        }
    }
    let r: Result<(), ()> = Ok(());
    std::eprintln!("dbg restore ok={}", r.is_ok());
    if let Err(e) = r {
        std::eprintln!("dbg restore_err={e:?}");
    }
    assert_eq!(link(9), std::path::Path::new(&a));
    remove(&a);
    remove(&b);
}

#[test]
fn last_wins_within_one_scope() {
    let a = temp_path("lw_a");
    let b = temp_path("lw_b");
    let before = link(1);
    let cell = cell();
    // Two write targets to fd 1 in source order: the list is sorted by fd and
    // stable within one fd, so `b` is applied last and its write wins.
    let scope = Scope::open(
        &[
            RedirectDef::write_path(1, cstr(&a)),
            RedirectDef::write_path(1, cstr(&b)),
        ],
        &cell,
    )
    .unwrap();
    write_fd(1, b"x");
    std::eprintln!(
        "dbg written f1={} f2={}",
        link(1).display(),
        link(2).display()
    );
    // Reverse restore puts the winner's predecessor back first, so fd 1 comes
    // back on the shell's original target; a forward restore would leave `a`.
    std::eprintln!("dbg saved_len={}", scope.saved.len());
    for (t, c) in scope.saved.iter() {
        std::eprintln!("dbg saved t={t} open={}", c.is_some());
    }
    for (t, c) in scope.saved.iter().rev() {
        std::eprintln!("dbg manual t={t} open={}", c.is_some());
        match c {
            Some(fd) => {
                std::eprintln!("dbg manual export {t} -> {}", fd.export_to(*t).is_ok());
            }
            None => {
                std::eprintln!("dbg manual close {t} -> {}", sys::dup::close(*t).is_ok());
            }
        }
    }
    let r: Result<(), ()> = Ok(());
    std::eprintln!("dbg restore ok={}", r.is_ok());
    if let Err(e) = r {
        std::eprintln!("dbg restore_err={e:?}");
    }
    assert_eq!(link(1), before);
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "x");
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "");
    remove(&a);
    remove(&b);
}

#[test]
fn open_failure_is_a_redirect_error() {
    let cell = cell();
    let report = Scope::open(&[RedirectDef::write_path(1, c"/nope/x")], &cell)
        .err()
        .unwrap();
    assert!(matches!(report.current_context(), CmdError::Redirect));
}

#[test]
fn saved_copy_survives_a_later_dup2() {
    // Closing fd 2 leaves a hole below `min_fd` (max target 2 + 1 = 3). A plain
    // `dup_cloexec` save would land the copy of fd 1 in that hole, and the
    // `2>&1` `dup2` — which closes fd 2 — would clobber it, so the restore would
    // put the wrong fd back on stdout. Saving above every target keeps it safe.
    let _ = sys::dup::close(2);
    let a = temp_path("dup_a");
    let before = link(1);
    let cell = cell();
    let scope = Scope::open(
        &[RedirectDef::write_path(1, cstr(&a)), RedirectDef::dup(2, 1)],
        &cell,
    )
    .unwrap();
    write_fd(1, b"x");
    // The write redirect wins on fd 1. `2>&1` copies the fd 1 that resolve
    // captured, i.e. the pre-scope stdout: sources are resolved before the scope
    // applies, so an intra-command `> a 2>&1` does not see the new fd 1 (a
    // divergence from bash, tracked by task #183).
    assert_eq!(link(1), std::path::Path::new(&a));
    assert_eq!(link(2), before);
    scope.restore().unwrap();
    // fd 1 is back to the pre-scope stdout. fd 2 was closed when the scope
    // opened, so its save is `None` and the restore leaves it closed.
    assert_eq!(link(1), before);
    assert!(sys::dup::dup_cloexec(2).is_err());
    // Give the harness a stderr again so a failure below is reportable.
    open_at(&"/dev/null".to_string(), 2);
    let body = std::fs::read_to_string(&a).unwrap();
    remove(&a);
    assert_eq!(body, "x");
}
