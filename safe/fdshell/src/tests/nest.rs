#![allow(clippy::unwrap_used)]
use alloc::vec::Vec;

use core::ffi::CStr;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

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
            let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn borrow_state<'a>(cell: &'a ForkCell<ShellState>) -> sys::fork_cell::Ref<'a, ShellState> {
    cell.borrow().unwrap()
}

fn var<'a>(state: &'a ShellState, name: &'static CStr) -> Option<&'a ShortCStr> {
    state.strings.get::<ShortCStr>(&name.into()).map(|v| &**v)
}

fn stext(bytes: &[u8]) -> sys::ScriptText {
    sys::ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        sys::Position::new(1, 1),
        sys::Origin::Shell,
    )
}

/// Wrap `inner` in `levels` nested `if true; then … ; fi` blocks.
fn nested_ifs(levels: usize, inner: &[u8]) -> Vec<u8> {
    let mut s = inner.to_vec();
    for _ in 0..levels {
        let mut next = Vec::with_capacity(s.len() + 16);
        next.extend_from_slice(b"if true; then ");
        next.extend_from_slice(&s);
        next.extend_from_slice(b"; fi");
        s = next;
    }
    s
}

#[test]
fn nested_ifs_at_limit_run() {
    child_test(|| {
        let cell = make_cell();
        let script = nested_ifs(100, b"deep=yes");
        crate::repl::run_script(&stext(&script), &cell).unwrap();
        assert_eq!(var(&borrow_state(&cell), c"deep"), Some(&c"yes".into()));
    });
}

#[test]
fn nested_ifs_over_limit_fail() {
    child_test(|| {
        let cell = make_cell();
        let script = nested_ifs(101, b"deep=yes");
        let e = crate::repl::run_script(&stext(&script), &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::NestingTooDeep));
    });
}

#[test]
fn nesting_restored_after_failure() {
    child_test(|| {
        let cell = make_cell();
        let script = nested_ifs(101, b"deep=yes");
        let e = crate::repl::run_script(&stext(&script), &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::NestingTooDeep));
        assert_eq!(borrow_state(&cell).nesting, 0);
        crate::repl::run_script(&stext(b"ok=1"), &cell).unwrap();
        assert_eq!(var(&borrow_state(&cell), c"ok"), Some(&c"1".into()));
    });
}

#[test]
fn while_body_counts_toward_limit() {
    child_test(|| {
        let cell = make_cell();
        let body = b"while true; do deep=yes; break; done";
        crate::repl::run_script(&stext(&nested_ifs(99, body)), &cell).unwrap();
        assert_eq!(var(&borrow_state(&cell), c"deep"), Some(&c"yes".into()));

        let cell = make_cell();
        let e = crate::repl::run_script(&stext(&nested_ifs(100, body)), &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::NestingTooDeep));
    });
}

#[test]
fn for_body_counts_toward_limit() {
    child_test(|| {
        let cell = make_cell();
        let body = b"for x in a; do deep=yes; done";
        crate::repl::run_script(&stext(&nested_ifs(99, body)), &cell).unwrap();
        assert_eq!(var(&borrow_state(&cell), c"deep"), Some(&c"yes".into()));

        let cell = make_cell();
        let e = crate::repl::run_script(&stext(&nested_ifs(100, body)), &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::NestingTooDeep));
    });
}

#[test]
fn case_body_counts_toward_limit() {
    child_test(|| {
        let cell = make_cell();
        let body = b"case x in x) deep=yes;; esac";
        crate::repl::run_script(&stext(&nested_ifs(99, body)), &cell).unwrap();
        assert_eq!(var(&borrow_state(&cell), c"deep"), Some(&c"yes".into()));

        let cell = make_cell();
        let e = crate::repl::run_script(&stext(&nested_ifs(100, body)), &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::NestingTooDeep));
    });
}

#[test]
fn cmd_subst_inherits_nesting() {
    child_test(|| {
        let cell = make_cell();
        let inner = nested_ifs(99, b"builtin echo deep");
        let mut script = Vec::with_capacity(inner.len() + 4);
        script.extend_from_slice(b"x=$(");
        script.extend_from_slice(&inner);
        script.extend_from_slice(b")");
        crate::repl::run_script(&stext(&script), &cell).unwrap();
        assert_eq!(var(&borrow_state(&cell), c"x"), Some(&c"deep".into()));
    });
}

#[test]
fn cmd_subst_over_limit_in_child() {
    child_test(|| {
        let cell = make_cell();
        let inner = nested_ifs(100, b"builtin echo deep");
        let mut script = Vec::with_capacity(inner.len() + 4);
        script.extend_from_slice(b"x=$(");
        script.extend_from_slice(&inner);
        script.extend_from_slice(b")");
        crate::repl::run_script(&stext(&script), &cell).unwrap();
        assert_eq!(var(&borrow_state(&cell), c"x"), Some(&c"".into()));
    });
}
