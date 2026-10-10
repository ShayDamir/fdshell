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

/// Read the redirect target and remove it.
fn body(path: &String) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let _ = std::fs::remove_file(path);
    text
}

#[test]
fn open_applies_the_redirection_and_restore_restores_stdout() {
    // Two successive file targets, one per scope: the second scope saves the fd
    // the first restore put back, so the save/restore cycle is repeatable.
    let a = temp_path("stdout_a");
    let b = temp_path("stdout_b");
    let before = link(1);
    let cell = cell();
    let scope = Scope::open(&[RedirectDef::write_path(1, cstr(&a))], &cell).unwrap();
    write_fd(1, b"x");
    assert_eq!(
        link(1),
        std::path::Path::new(&a),
        "the redirect is applied to fd 1"
    );
    scope.restore().unwrap();
    assert_eq!(
        link(1),
        before,
        "fd 1 is back on the shell's original target"
    );
    let scope = Scope::open(&[RedirectDef::write_path(1, cstr(&b))], &cell).unwrap();
    write_fd(1, b"y");
    assert_eq!(
        link(1),
        std::path::Path::new(&b),
        "the second target is applied"
    );
    scope.restore().unwrap();
    assert_eq!(link(1), before, "and the second restore puts fd 1 back");
    assert_eq!(body(&a), "x");
    assert_eq!(body(&b), "y");
}

#[test]
fn restore_of_a_closed_fd_leaves_it_closed() {
    // A number the test binary owns and frees, so the target is closed at restore.
    let opened = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let fd = opened.as_raw();
    assert!(sys::dup::dup_cloexec(fd).is_ok(), "the target is open");
    drop(opened);
    // `None` = the target was closed when the scope opened, so the restore closes
    // it best-effort: an already-closed fd is not an actionable error (§4.12).
    let scope = Scope {
        saved: vec![(fd, None)],
    };
    scope.restore().unwrap();
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
    assert_eq!(
        link(9),
        std::path::Path::new(&b),
        "the redirect replaces fd 9"
    );
    scope.restore().unwrap();
    assert_eq!(
        link(9),
        std::path::Path::new(&a),
        "fd 9 is back on the file it saved"
    );
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
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
    assert_eq!(
        link(1),
        std::path::Path::new(&b),
        "the last redirection to fd 1 wins"
    );
    // Reverse restore undoes the winner first, so fd 1 comes back on the shell's
    // original target; a forward restore would stop on the loser `a`.
    scope.restore().unwrap();
    assert_eq!(
        link(1),
        before,
        "fd 1 is restored past the loser, not on it"
    );
    assert_eq!(body(&b), "x", "the winner keeps the write");
    assert_eq!(
        body(&a),
        "",
        "the loser was opened and truncated, and got nothing"
    );
}

#[test]
fn reverse_restore_leaves_the_shell_fd_on_its_original_target() {
    // The discriminating pin for the reverse apply order. fd 1 is the shell's
    // stdout, so the post-restore write is observable by absence: a forward
    // restore ends on the loser `a`, so this write lands in `a` and the
    // `a`-is-empty assertion fails.
    let a = temp_path("rev_a");
    let b = temp_path("rev_b");
    let before = link(1);
    let cell = cell();
    let scope = Scope::open(
        &[
            RedirectDef::write_path(1, cstr(&a)),
            RedirectDef::write_path(1, cstr(&b)),
        ],
        &cell,
    )
    .unwrap();
    write_fd(1, b"x");
    scope.restore().unwrap();
    write_fd(1, b"after");
    assert_eq!(
        link(1),
        before,
        "fd 1 is back on the shell's original target"
    );
    assert_eq!(
        body(&a),
        "",
        "the post-restore write did not land on the loser `a`"
    );
    assert_eq!(body(&b), "x");
}

#[test]
fn post_restore_write_lands_on_the_restored_target() {
    // The same rule with a file as the shell's original target, so the
    // post-restore write is observable in its body. The save copies `orig`'s
    // file description (offset included), so the restore resumes it where the
    // shell left off, and `after` appends to `orig`.
    let orig = temp_path("rw_orig");
    let a = temp_path("rw_a");
    let b = temp_path("rw_b");
    open_at(&orig, 9);
    write_fd(9, b"orig\n");
    let cell = cell();
    let scope = Scope::open(
        &[
            RedirectDef::write_path(9, cstr(&a)),
            RedirectDef::write_path(9, cstr(&b)),
        ],
        &cell,
    )
    .unwrap();
    write_fd(9, b"body");
    assert_eq!(
        link(9),
        std::path::Path::new(&b),
        "the last redirection wins"
    );
    scope.restore().unwrap();
    assert_eq!(
        link(9),
        std::path::Path::new(&orig),
        "fd 9 is back on `orig`"
    );
    write_fd(9, b"after");
    assert_eq!(
        body(&orig),
        "orig\nafter",
        "the post-restore write lands on the restored target"
    );
    assert_eq!(body(&a), "");
    assert_eq!(body(&b), "body");
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
fn resolve_failure_is_a_redirect_error() {
    // A `%var` source that is not in the fd table: `f &> %log` / `cd /tmp &>
    // %log` with `%log` unset. The failure must surface as a redirect error,
    // not as the raw `VarNotFound` frame.
    let cell = cell();
    let report = Scope::open(&[RedirectDef::var(1, c"unset")], &cell)
        .err()
        .unwrap();
    assert!(matches!(report.current_context(), CmdError::Redirect));
}

#[test]
fn restore_of_a_lost_saved_copy_is_a_redirect_error() {
    // The restore's `dup2` needs its source: once the saved copy is gone the
    // export fails, and the failure surfaces as a redirect error. The same arm
    // is reached from a script (`f(){ echo ok; exec 6>&-; }; f >q` closes the
    // copy at fd 6, see `run/parent/tests.rs` and
    // `tests/redirect_scope.rs::body_that_closes_the_saved_copy_fails_the_call`);
    // this unit pin is the deterministic version, independent of where the copy
    // lands.
    let a = temp_path("lost_copy_a");
    let cell = cell();
    let scope = Scope::open(&[RedirectDef::write_path(1, cstr(&a))], &cell).unwrap();
    let copy = scope.saved.first().unwrap().1.as_ref().unwrap().as_raw();
    sys::dup::close(copy).unwrap();
    let report = scope.restore().err().unwrap();
    assert!(matches!(report.current_context(), CmdError::Redirect));
    assert_eq!(body(&a), "");
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
    assert_eq!(body(&a), "x");
}
