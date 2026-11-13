#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use sys::siginfo::WaitStatus;
use sys::{Origin, Position, ScriptText};

fn st(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn run_one(script: &[u8], cell: &ForkCell<ShellState>) {
    crate::run::run_one(&st(script), cell, false).unwrap();
}

/// The child half of `src/tests/run.rs`: the umask is the process's, so the rows that
/// really call the syscall run in a fork and restore what they found.
fn child_test(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            sys::umask::init();
            let saved = sys::umask::get();
            f();
            sys::umask::set(saved);
            sys::exit(42);
        }
        Some(pidfd) => {
            let status = pidfd.wait_pidfd().unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

/// What the run prints for each flag combination, bash 5.3.9 measured (the mask in
/// each row is the one bash had when the line was printed — an `applied` row is a
/// mode operand, and only `-S` prints then, without the reusable prefix).
#[test]
fn the_report_line_is_the_one_bash_prints() {
    for ((applied, dash_p, dash_s, mask), want) in [
        ((false, false, false, 0o022), Some("0022\n")),
        ((false, true, false, 0o022), Some("umask 0022\n")),
        ((false, false, true, 0o022), Some("u=rwx,g=rx,o=rx\n")),
        (
            (false, true, true, 0o022),
            Some("umask -S u=rwx,g=rx,o=rx\n"),
        ),
        ((true, false, false, 0o022), None),
        ((true, true, false, 0o022), None),
        ((true, false, true, 0o077), Some("u=rwx,g=,o=\n")),
        ((true, true, true, 0o077), Some("u=rwx,g=,o=\n")),
        ((false, false, false, 0o000), Some("0000\n")),
        ((false, false, true, 0o000), Some("u=rwx,g=rwx,o=rwx\n")),
        ((false, false, true, 0o777), Some("u=,g=,o=\n")),
        ((false, false, true, 0o644), Some("u=x,g=wx,o=wx\n")),
        ((false, false, true, 0o111), Some("u=rw,g=rw,o=rw\n")),
        ((false, false, true, 0o240), Some("u=rx,g=wx,o=rwx\n")),
    ] {
        assert_eq!(
            report(applied, dash_p, dash_s, mask).as_deref(),
            want,
            "{applied} {dash_p} {dash_s} {mask:04o}"
        );
    }
}

#[test]
fn a_symbolic_operand_moves_the_mask_and_leaves_zero() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"umask 022", &cell);
        run_one(b"umask u=g", &cell);
        assert_eq!(sys::umask::get(), 0o222);
        assert!(matches!(
            cell.borrow().unwrap().last_status,
            WaitStatus::Exited(0)
        ));
    });
}

#[test]
fn an_octal_operand_still_sets_the_mask_it_is_given() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"umask 022", &cell);
        run_one(b"umask 0o077", &cell);
        assert_eq!(sys::umask::get(), 0o077);
        assert!(matches!(
            cell.borrow().unwrap().last_status,
            WaitStatus::Exited(0)
        ));
    });
}

#[test]
fn the_no_operand_report_leaves_zero_too() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"umask 022", &cell);
        run_one(b"umask", &cell);
        assert_eq!(sys::umask::get(), 0o022);
        assert!(matches!(
            cell.borrow().unwrap().last_status,
            WaitStatus::Exited(0)
        ));
    });
}
