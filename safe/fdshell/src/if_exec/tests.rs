#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::ParsedLine;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// Parse an `if` statement into its `IfBlock` (panics if it is not one).
fn if_block(line: &[u8]) -> IfBlock {
    match crate::parse::parse(&stext(line)).unwrap() {
        ParsedLine::If(block) => block,
        _ => panic!("not an if statement: {line:?}"),
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn in_loop(cell: &ForkCell<ShellState>, depth: u32) {
    cell.borrow_mut().unwrap().loop_depth = depth;
}

fn status(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow().unwrap().last_status.exit_code()
}

fn var(cell: &ForkCell<ShellState>, name: &str) -> Option<sys::ShortCStr> {
    let key = sys::ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap();
    cell.borrow()
        .unwrap()
        .strings
        .get::<sys::ShortCStr>(&key)
        .map(|s| s.value.clone())
}

/// A `break` written in the condition leaves the `if` whole: an `if` is not a
/// loop level, so the count is the caller's and nothing is consumed here.
#[test]
fn break_in_the_condition_propagates_out_of_the_if() {
    let cell = make_cell();
    in_loop(&cell, 1);
    let block = if_block(b"if break; then result=1; fi");
    assert_eq!(run_if(&block, &cell).unwrap(), Some(LoopControl::Break(1)));
    assert!(var(&cell, "result").is_none());
}

/// The `continue` twin: the signal reaches the caller's loop.
#[test]
fn continue_in_the_condition_propagates_out_of_the_if() {
    let cell = make_cell();
    in_loop(&cell, 1);
    let block = if_block(b"if continue; then result=1; fi");
    assert_eq!(
        run_if(&block, &cell).unwrap(),
        Some(LoopControl::Continue(1))
    );
    assert!(var(&cell, "result").is_none());
}

/// An `elif` condition is a cond-list run like the `if` condition, so its control
/// propagates out of the `if` too (bash: `elif break` ends the enclosing loop).
#[test]
fn break_in_an_elif_condition_propagates() {
    let cell = make_cell();
    in_loop(&cell, 1);
    let block = if_block(b"if (( 0 )); then :; elif break; then result=1; fi");
    assert_eq!(run_if(&block, &cell).unwrap(), Some(LoopControl::Break(1)));
    assert!(var(&cell, "result").is_none());
}

/// `return` in a condition returns out of the `if` with the status the `return`
/// set (bash: `f() { if return 3; then :; fi; echo after; }; f` → rc 3).
#[test]
fn return_in_an_if_condition_propagates() {
    let cell = make_cell();
    let block = if_block(b"if return 3; then result=1; fi");
    assert_eq!(run_if(&block, &cell).unwrap(), Some(LoopControl::Return));
    assert!(var(&cell, "result").is_none());
    assert_eq!(status(&cell), 3);
}

/// The `return` twin in an `elif` condition.
#[test]
fn return_in_an_elif_condition_propagates() {
    let cell = make_cell();
    let block = if_block(b"if (( 0 )); then :; elif return 3; then result=1; fi");
    assert_eq!(run_if(&block, &cell).unwrap(), Some(LoopControl::Return));
    assert!(var(&cell, "result").is_none());
    assert_eq!(status(&cell), 3);
}

/// A condition that raises no control still takes the `then` body: only a `Some`
/// control propagates, a clean status never does.
#[test]
fn clean_condition_runs_the_then_body() {
    let cell = make_cell();
    let block = if_block(b"if (( 1 )); then result=1; fi");
    assert_eq!(run_if(&block, &cell).unwrap(), None);
    assert!(var(&cell, "result").is_some_and(|v| v.eq_bytes(b"1")));
}

/// A failing condition takes the `else` body (kills a "propagate on every cond
/// result" reading of the condition site).
#[test]
fn failing_condition_runs_the_else_body() {
    let cell = make_cell();
    let block = if_block(b"if (( 0 )); then result=1; else other=1; fi");
    assert_eq!(run_if(&block, &cell).unwrap(), None);
    assert!(var(&cell, "result").is_none());
    assert!(var(&cell, "other").is_some_and(|v| v.eq_bytes(b"1")));
}

/// A true `elif` condition runs its arm and hands back `None` (the second
/// cond-list site must propagate control only, never the arm's status).
#[test]
fn true_elif_condition_runs_the_elif_arm() {
    let cell = make_cell();
    let block = if_block(b"if (( 0 )); then :; elif (( 1 )); then other=1; fi");
    assert_eq!(run_if(&block, &cell).unwrap(), None);
    assert!(var(&cell, "other").is_some_and(|v| v.eq_bytes(b"1")));
}

/// The error path is a statement-level parse abort: `break 0` never reaches
/// `run_if`, the whole statement fails to parse (rc 1, README "Loop control").
#[test]
fn bad_break_count_in_a_condition_is_a_parse_abort() {
    let cell = make_cell();
    let report = crate::repl::handle(&stext(b"if break 0; then :; fi"), &cell).unwrap_err();
    assert!(
        alloc::format!("{report:?}").contains("break/continue count must be a positive integer"),
        "{report:?}"
    );
}

/// The `&`-terminated condition's job fact reaches `run_if`'s two cond-list
/// sites as the parsed block's `cond_background` field (task #356): the plain
/// `&` sets it, a newline `;` token steps back to it, and the `& ;` form (a real
/// `;` byte, task #355) never does. Pinned on the **field**, so both sites are
/// covered without a fork: the flag is the fact's single owner, and the
/// run-side job is pinned in `tests/run.rs`.
#[test]
fn ampersand_terminated_condition_flags_the_block() {
    assert!(if_block(b"if true & then builtin true; fi").cond_background);
    assert!(
        if_block(b"if true &\nthen builtin true; fi").cond_background,
        "bash reads `&` newline `then` as `& then`"
    );
    assert!(
        !if_block(b"if true & ; then builtin true; fi").cond_background,
        "the `& ;` slice stays #355's parse error, with no job"
    );
    assert!(!if_block(b"if true; then builtin true; fi").cond_background);
    assert!(
        !if_block(b"if true; then builtin true & fi").cond_background,
        "a body `&` belongs to the body's own scan"
    );
    let block = if_block(b"if (( 0 )); then :; elif true & then builtin true; fi");
    assert!(
        block.elifs[0].cond_background,
        "the `elif` arm flags itself"
    );
    assert!(!block.cond_background, "and never the `if` condition");
    let block = if_block(b"if (( 0 )); then :; elif true & ; then builtin true; fi");
    assert!(!block.elifs[0].cond_background);
}

// --- the redirection written after the block's `fi` (task #298) ------------

use alloc::format;
use alloc::string::{String, ToString};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop: pids are recycled,
/// so a dir that outlives its process is a later run's inherited dir (LESSONS,
/// task #255). Removal is `.ok()`-tolerant: teardown never panics.
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

/// A pid + counter scratch dir named after `tag`, as a file path inside it plus
/// the guard — bind both (`let (a, _sa) = …`).
fn temp_path(tag: &str) -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell_if_{tag}_{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("f").to_str().unwrap().to_string(), Scratch(dir))
}

/// What fd `n` points at: `/proc/self/fd/N` is a symlink to the open file, so
/// the scope's save/restore is asserted at the fd level (LESSONS: a test that
/// hand-copies the implementation pins the copy).
fn link(n: i32) -> PathBuf {
    std::fs::read_link(format!("/proc/self/fd/{n}")).unwrap()
}

/// Read the redirect target and remove it.
fn body(path: &String) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let _ = std::fs::remove_file(path);
    text
}

/// The `if`'s own redirection: the taken arm's output lands in the file, and
/// fd 1 is back on its original target when the `if` ends.
#[test]
fn a_redirect_after_fi_applies_to_the_if_and_is_restored() {
    let (a, _sa) = temp_path("apply");
    let before = link(1);
    let cell = make_cell();
    let line = format!("if :; then echo t; fi > {a}");
    let block = if_block(line.as_bytes());
    assert_eq!(run_if(&block, &cell).unwrap(), None);
    assert_eq!(link(1), before, "fd 1 is back on its original target");
    assert_eq!(body(&a), "t\n");
}

/// The scope covers every arm: the `elif` chain and the `else` body write into
/// the same file, and the fds are restored after the `if`.
#[test]
fn the_scope_covers_the_elif_and_else_arms() {
    let (a, _sa) = temp_path("arms");
    let before = link(1);
    let cell = make_cell();
    let line = format!("if false; then echo n; elif false; then echo m; else echo e; fi > {a}");
    let block = if_block(line.as_bytes());
    assert_eq!(run_if(&block, &cell).unwrap(), None);
    assert_eq!(link(1), before, "fd 1 is back on its original target");
    assert_eq!(body(&a), "e\n");
}

/// A `break` raised inside the `if` propagates to the caller (`run_if` hands
/// the control whole): the scope restores on that path.
#[test]
fn the_scope_restores_when_the_control_propagates() {
    let (a, _sa) = temp_path("control");
    let before = link(1);
    let cell = make_cell();
    in_loop(&cell, 1);
    let line = format!("if :; then break; fi > {a}");
    let block = if_block(line.as_bytes());
    assert!(run_if(&block, &cell).unwrap().is_some());
    assert_eq!(link(1), before, "the restore runs on the propagating path");
}

/// The scope opens **before** the condition runs, so a failing open aborts the
/// command: the condition and the body never run (bash's order, measured).
#[test]
fn a_failing_open_skips_the_condition_and_the_body() {
    let cell = make_cell();
    let block = if_block(b"if :; then x=1; fi > /nope/dir/f");
    let e = run_if(&block, &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Redirect));
    assert!(var(&cell, "x").is_none(), "the then-body never ran");
}
