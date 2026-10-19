#![allow(clippy::unwrap_used)]
use alloc::format;
use alloc::string::{String, ToString};
use core::ffi::CStr;

use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText, ShortCStr};

use crate::loop_control::LoopControl;
use crate::state::ShellState;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::run_group;

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

fn text(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn group_of(b: &[u8]) -> crate::parse::group_block::GroupBlock {
    match crate::parse::parse(&text(b)).unwrap() {
        crate::parse::ParsedLine::Group(g) => g,
        _ => panic!("expected Group"),
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn var<'a>(state: &'a ShellState, name: &'static CStr) -> Option<&'a ShortCStr> {
    state.strings.get::<ShortCStr>(&name.into()).map(|v| &**v)
}

/// A pid + counter scratch dir (the old `fdshell_group_<tag>_<pid>` files
/// live inside it now); returns the path plus the guard — bind both.
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell_group_{tag}_{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("f").to_str().unwrap().to_string(), Scratch(dir))
}

/// Read the redirect target and remove it.
fn body(path: &String) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let _ = std::fs::remove_file(path);
    text
}

/// What fd `n` points at: `/proc/self/fd/N` is a symlink to the open file.
fn link(n: i32) -> std::path::PathBuf {
    std::fs::read_link(format!("/proc/self/fd/{n}")).unwrap()
}

/// POSIX #1.2: the body runs in the shell, so its assignment survives the group.
#[test]
fn body_assignment_persists_after_the_group() {
    let cell = make_cell();
    run_group(&group_of(b"{ x=hi; }"), &cell).unwrap();
    assert_eq!(var(&cell.borrow().unwrap(), c"x"), Some(&c"hi".into()));
}

/// The body runs as a cond list, so `&&` / `||` keep their semantics.
#[test]
fn cond_list_semantics_inside_the_body() {
    let cell = make_cell();
    run_group(&group_of(b"{ x=1 && y=2; }"), &cell).unwrap();
    let state = cell.borrow().unwrap();
    assert_eq!(var(&state, c"x"), Some(&c"1".into()));
    assert_eq!(var(&state, c"y"), Some(&c"2".into()));
    drop(state);

    let cell = make_cell();
    run_group(&group_of(b"{ false && y=2; }"), &cell).unwrap();
    assert_eq!(var(&cell.borrow().unwrap(), c"y"), None);
}

/// `break` / `return` in the body propagate to the caller like in a block body.
#[test]
fn loop_control_propagates_out_of_the_body() {
    let cell = make_cell();
    // One loop level so `break` is inside a loop and propagates the clamped count.
    cell.borrow_mut().unwrap().loop_depth = 1;
    let control = run_group(&group_of(b"{ break; }"), &cell).unwrap();
    assert!(matches!(control, Some(LoopControl::Break(1))));

    let cell = make_cell();
    let control = run_group(&group_of(b"{ return 3; }"), &cell).unwrap();
    assert!(matches!(control, Some(LoopControl::Return)));
}

/// The group's redirection is applied for the body and fd 1 is back after it.
#[test]
fn redirect_applies_for_the_group_and_is_restored() {
    let (a, _sa) = temp_path("apply_a");
    let (b, _sb) = temp_path("apply_b");
    let before = link(1);
    let cell = make_cell();
    run_group(
        &group_of(&format!("{{ echo a; }} > {a}").into_bytes()),
        &cell,
    )
    .unwrap();
    assert_eq!(link(1), before, "fd 1 is back on its original target");
    assert_eq!(body(&a), "a\n");

    run_group(
        &group_of(&format!("{{ echo b; }} > {b}").into_bytes()),
        &cell,
    )
    .unwrap();
    assert_eq!(link(1), before, "the restore is repeatable");
    assert_eq!(body(&b), "b\n");
}

/// The body runs one nesting level deeper, and the depth is put back.
#[test]
fn nesting_depth_is_restored() {
    let cell = make_cell();
    run_group(&group_of(b"{ x=1; }"), &cell).unwrap();
    assert_eq!(cell.borrow().unwrap().nesting, 0);

    let mut state = cell.borrow_mut().unwrap();
    state.nesting = crate::nest::MAX_NESTING - 1;
    drop(state);
    run_group(&group_of(b"{ x=2; }"), &cell).unwrap();
    assert_eq!(cell.borrow().unwrap().nesting, crate::nest::MAX_NESTING - 1);

    // At the cap the body never runs, and the depth is not incremented.
    let cell = make_cell();
    let mut state = cell.borrow_mut().unwrap();
    state.nesting = crate::nest::MAX_NESTING;
    drop(state);
    let e = run_group(&group_of(b"{ x=3; }"), &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        crate::error::cmd::CmdError::NestingTooDeep
    ));
    let state = cell.borrow().unwrap();
    assert_eq!(state.nesting, crate::nest::MAX_NESTING);
    assert_eq!(var(&state, c"x"), None);
}

/// A failing redirection open aborts the group before the body runs; a failing
/// command inside the body is reported by the body and the fds are still
/// restored when the group ends.
#[test]
fn failing_open_skips_the_body_and_errors_are_restored() {
    let cell = make_cell();
    let e = run_group(&group_of(b"{ x=1; } > /nope/dir/f"), &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        crate::error::cmd::CmdError::Redirect
    ));
    assert_eq!(var(&cell.borrow().unwrap(), c"x"), None);

    let before = link(1);
    let (target, _scratch) = temp_path("body_err");
    run_group(
        &group_of(&format!("{{ nosuchcmd; }} > {target}").into_bytes()),
        &cell,
    )
    .unwrap();
    assert_eq!(link(1), before, "the restore runs on the body's error path");
}
