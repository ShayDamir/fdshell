#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Run pins of the arithmetic `for ((init;cond;step)) ; do … ; done`
//! (task #293): the clause order, the control flow, the status rule. The
//! harness is `loop_/tests.rs`'s, since both runners are loops.

use super::arith::*;
use crate::loop_control::LoopControl;
use crate::parse::ParsedLine;
use crate::parse::for_block::arith::ArithForBlock;
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

/// Parse a `for ((…))` statement into its `ArithForBlock` (panics if it is not one).
fn arith_block(line: &[u8]) -> ArithForBlock {
    match crate::parse::parse(&stext(line)).unwrap() {
        ParsedLine::ArithFor(block) => block,
        _ => panic!("not an arithmetic for: {line:?}"),
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

/// Seed the state with in-process statements (`i=0`, `false`, …) before the loop.
fn seed(cell: &ForkCell<ShellState>, setup: &[u8]) {
    crate::script::run_script(&stext(setup), cell).unwrap();
}

fn status(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow().unwrap().last_status.exit_code()
}

fn depth(cell: &ForkCell<ShellState>) -> u32 {
    cell.borrow().unwrap().loop_depth
}

fn var(cell: &ForkCell<ShellState>, name: &str) -> Option<sys::ShortCStr> {
    let key = sys::ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap();
    cell.borrow()
        .unwrap()
        .strings
        .get::<sys::ShortCStr>(&key)
        .map(|s| s.value.clone())
}

fn is_set(cell: &ForkCell<ShellState>, name: &str) -> bool {
    var(cell, name).is_some()
}

/// The step drives the condition, and the loop var is an in-process shell
/// variable that survives the loop (bash: `for ((i=3;i>0;i--)); do …; done; echo
/// i=$i` → `i=0`).
#[test]
fn the_step_drives_the_condition() {
    let cell = make_cell();
    seed(&cell, b"n=0");
    let block = arith_block(b"for ((i=0;i<3;i++)); do n=$((n+1)); done");
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert!(var(&cell, "n").is_some_and(|v| v.eq_bytes(b"3")));
    assert!(var(&cell, "i").is_some_and(|v| v.eq_bytes(b"3")));
}

/// An empty condition is true: the loop runs until its body breaks out of it.
#[test]
fn empty_condition_loops_until_break() {
    let cell = make_cell();
    let block = arith_block(b"for ((;;)); do once=1; break; done");
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert!(is_set(&cell, "once"));
    assert_eq!(status(&cell), 0);
}

/// An empty init and an empty step are no-ops; the body carries the loop.
#[test]
fn empty_init_and_step_run() {
    let cell = make_cell();
    seed(&cell, b"n=0; i=0");
    let block = arith_block(b"for ((;i<2;)); do n=$((n+1)); i=$((i+1)); done");
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert!(var(&cell, "n").is_some_and(|v| v.eq_bytes(b"2")));
}

/// A body that never ran leaves status 0, not the last condition's value (the
/// rule of task #164, shared with `while`/`until` and the word `for`).
#[test]
fn body_never_runs_reports_status_zero() {
    let cell = make_cell();
    seed(&cell, b"false");
    assert_eq!(status(&cell), 1);
    let block = arith_block(b"for ((i=0;i<0;i++)); do false; done");
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert_eq!(status(&cell), 0);
}

/// The loop's status is the body's last command (bash: `do false; done; echo
/// $?` → 1).
#[test]
fn status_is_the_body_last_command() {
    let cell = make_cell();
    let block = arith_block(b"for ((i=0;i<3;i++)); do false; done");
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert_eq!(status(&cell), 1);
}

/// `break` leaves the loop **before** the step, so the loop var keeps the value
/// the iteration started with.
#[test]
fn break_skips_the_step() {
    let cell = make_cell();
    let block = arith_block(b"for ((i=0;i<3;i++)); do break; done");
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert!(var(&cell, "i").is_some_and(|v| v.eq_bytes(b"0")));
    assert_eq!(status(&cell), 0);
}

/// `continue` runs the step, then the condition is re-tested: the guarded word
/// after it is never set, and the step reaches the last value.
#[test]
fn continue_runs_the_step() {
    let cell = make_cell();
    let block = arith_block(b"for ((i=0;i<2;i++)); do continue; never=1; done");
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert!(!is_set(&cell, "never"));
    assert!(var(&cell, "i").is_some_and(|v| v.eq_bytes(b"2")));
}

/// `break 2` at depth 1 (an enclosing loop) is this loop's own break plus the
/// remainder for the enclosing level, and `in_loop` restores the depth.
#[test]
fn break_2_at_depth_one_hands_the_remainder_over() {
    let cell = make_cell();
    cell.borrow_mut().unwrap().loop_depth = 1;
    let block = arith_block(b"for ((i=0;i<3;i++)); do break 2; done");
    assert_eq!(run(&block, &cell).unwrap(), Some(LoopControl::Break(1)));
    assert_eq!(depth(&cell), 1);
    assert!(var(&cell, "i").is_some_and(|v| v.eq_bytes(b"0")));
}

/// `return` in the body leaves the loop whole, status included, and hands
/// [`LoopControl::Return`] to the function frame.
#[test]
fn return_in_the_body_leaves_the_loop() {
    let cell = make_cell();
    let block = arith_block(b"for ((i=0;i<3;i++)); do builtin true; return 3; done");
    assert_eq!(run(&block, &cell).unwrap(), Some(LoopControl::Return));
    assert_eq!(status(&cell), 3);
    assert_eq!(depth(&cell), 0);
}

/// Every exit path runs inside [`in_loop`], so the loop depth is restored.
#[test]
fn in_loop_restores_the_depth() {
    for line in [
        b"for ((i=0;i<3;i++)); do :; done".as_slice(),
        b"for ((;;)); do break; done".as_slice(),
        b"for ((i=0;i<0;i++)); do :; done".as_slice(),
        b"for ((i=0;i<2;i++)); do continue; done".as_slice(),
    ] {
        let cell = make_cell();
        assert_eq!(run(&arith_block(line), &cell).unwrap(), None, "{line:?}");
        assert_eq!(depth(&cell), 0, "{line:?}");
    }
}

/// An arithmetically broken condition is a resolve error that stops the loop:
/// the statement aborts (the `((expr))` command's rule), it never runs forever.
#[test]
fn condition_arith_error_is_a_resolve_error() {
    let cell = make_cell();
    let block = arith_block(b"for ((i=0;i<;i++)); do echo a; done");
    assert!(run(&block, &cell).is_err());
}

/// A broken **step** is an error too, and it is raised only once the loop got
/// past the condition (bash's step error also ends the loop with rc 1).
#[test]
fn step_arith_error_surfaces_after_the_body() {
    let cell = make_cell();
    let block = arith_block(b"for ((i=0;i<2;i++ +)); do builtin true; done");
    assert!(run(&block, &cell).is_err());
    // The step's expression never parsed, so no increment happened at all.
    assert!(var(&cell, "i").is_some_and(|v| v.eq_bytes(b"0")));
}

// --- the redirection written after the block's `done` (task #298) ----------

use super::run_for;
use crate::error::cmd::CmdError;
use crate::parse::for_block::ForBlock;
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
    let dir = std::env::temp_dir().join(format!("fdshell_for_{tag}_{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("f").to_str().unwrap().to_string(), Scratch(dir))
}

/// What fd `n` points at: `/proc/self/fd/N` is a symlink to the open file, so
/// the scope's save/restore is asserted at the fd level (LESSONS).
fn link(n: i32) -> PathBuf {
    std::fs::read_link(format!("/proc/self/fd/{n}")).unwrap()
}

/// Read the redirect target and remove it.
fn body(path: &String) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let _ = std::fs::remove_file(path);
    text
}

/// Parse a word-form `for` statement into its `ForBlock`.
fn for_block(line: &[u8]) -> ForBlock {
    match crate::parse::parse(&stext(line)).unwrap() {
        ParsedLine::For(block) => block,
        _ => panic!("not a for statement: {line:?}"),
    }
}

/// The word `for`'s own redirection: every iteration's output lands in the
/// file, and fd 1 is back on its original target when the loop ends.
#[test]
fn a_redirect_after_done_applies_to_the_for_and_is_restored() {
    let (a, _sa) = temp_path("apply");
    let text = stext(format!("for x in a b; do echo $x; done > {a}").as_bytes());
    let before = link(1);
    let cell = make_cell();
    let block = for_block(text.as_bytes().unwrap());
    assert_eq!(run_for(&block, &text, &cell).unwrap(), None);
    assert_eq!(link(1), before, "fd 1 is back on its original target");
    assert_eq!(body(&a), "a\nb\n");
}

/// An **empty word list** still opens the target: the file is created, empty.
#[test]
fn an_empty_word_list_still_creates_the_file() {
    let (a, _sa) = temp_path("empty_words");
    let text = stext(format!("for x in; do echo A; done > {a}").as_bytes());
    let cell = make_cell();
    let block = for_block(text.as_bytes().unwrap());
    assert_eq!(run_for(&block, &text, &cell).unwrap(), None);
    assert_eq!(body(&a), "", "bash creates the file, empty");
}

/// The scope opens **before** the word list is expanded, so a failing open
/// leaves a side-effecting word list unexpanded (bash's order, measured: no
/// `m` file).
#[test]
fn a_failing_open_leaves_the_word_list_unexpanded() {
    let (_dir, scratch) = temp_path("words");
    let cell = make_cell();
    // The state's cwd is where a relative target and a side-effect file land.
    seed(&cell, format!("cd {}", scratch.display()).as_bytes());
    let text = stext(b"for x in $(touch m; echo a); do :; done > /nope/dir/f");
    let block = for_block(text.as_bytes().unwrap());
    let e = run_for(&block, &text, &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Redirect));
    assert!(
        !scratch.join("m").exists(),
        "the word list is not expanded when the open fails"
    );
}

/// The arithmetic `for`: the same rule, with the scope opening before the init
/// clause runs.
#[test]
fn a_redirect_after_done_applies_to_the_arithmetic_for() {
    let (a, _sa) = temp_path("arith");
    let text = stext(format!("for ((i=0;i<2;i++)); do echo $i; done > {a}").as_bytes());
    let before = link(1);
    let cell = make_cell();
    let block = arith_block(text.as_bytes().unwrap());
    assert_eq!(run(&block, &cell).unwrap(), None);
    assert_eq!(link(1), before, "fd 1 is back on its original target");
    assert_eq!(body(&a), "0\n1\n");
}

/// A failing open on the arithmetic `for` leaves the clauses unrun: no `m`.
#[test]
fn a_failing_open_leaves_the_arithmetic_clauses_unrun() {
    let (_dir, scratch) = temp_path("arith_words");
    let cell = make_cell();
    seed(&cell, format!("cd {}", scratch.display()).as_bytes());
    let text = stext(b"for ((i=$(touch m; echo 1);i<2;i++)); do :; done > /nope/dir/f");
    let block = arith_block(text.as_bytes().unwrap());
    let e = run(&block, &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Redirect));
    assert!(
        !scratch.join("m").exists(),
        "no clause is evaluated when the open fails"
    );
}
