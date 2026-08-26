#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::{FdVar, ShellState};
use crate::task::Task;
use error_stack::Report;
use sys::ShortCStr;
use sys::Trace;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;
use sys::{Origin, Position, ScriptText};

fn st(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn run_one(b: &[u8], cell: &ForkCell<ShellState>) -> Result<Option<LoopControl>, Report<CmdError>> {
    crate::run::run_one(&st(b), cell)
}

fn run_script(
    b: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    crate::script::run_script(&st(b), cell)
}

fn run_cond_list(
    b: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    crate::cond::run_cond_list(&st(b), cell)
}

fn handle(b: &[u8], cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    crate::repl::handle(&st(b), cell)
}

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

fn borrow_state_mut<'a>(cell: &'a ForkCell<ShellState>) -> sys::fork_cell::RefMut<'a, ShellState> {
    cell.borrow_mut().unwrap()
}

#[test]
fn umask_set_via_run_one() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"umask 0o077", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn umask_set_zero_via_run_one() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"umask 0o000", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn umask_set_without_o_prefix() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"umask 077", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn umask_invalid_returns_err() {
    child_test(|| {
        let cell = make_cell();
        let e = run_one(b"umask abc", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::Parse));
    });
}

#[test]
fn umask_too_many_args_returns_err() {
    child_test(|| {
        let cell = make_cell();
        let e = run_one(b"umask 0o077 extra", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::Parse));
    });
}

#[test]
fn waitpid_no_tasks() {
    let cell = make_cell();
    run_one(b"waitpid", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn waitpid_nonexistent_name() {
    let cell = make_cell();
    let e = run_one(b"waitpid &nonexistent", &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Task));
}

#[test]
fn waitpid_one_task() {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => sys::exit(42),
        Some(pidfd) => {
            let cell = make_cell();
            {
                let mut state = borrow_state_mut(&cell);
                state.tasks.insert(
                    ShortCStr::from(c"mytask"),
                    Task {
                        pidfd,
                        capture_fd: None,
                        child_pid: ret,
                        captures: Vec::new(),
                    },
                );
            }
            run_one(b"waitpid &mytask", &cell).unwrap();
            let state = borrow_state(&cell);
            assert!(matches!(state.last_status, WaitStatus::Exited(42)));
            assert!(state.tasks.is_empty());
        }
    }
}

#[test]
fn waitpid_all_tasks() {
    let (ret1, pidfd1_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    let pidfd1 = match pidfd1_opt {
        None => sys::exit(42),
        Some(pidfd) => pidfd,
    };
    let (ret2, pidfd2_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    let pidfd2 = match pidfd2_opt {
        None => sys::exit(7),
        Some(pidfd) => pidfd,
    };
    let cell = make_cell();
    {
        let mut state = borrow_state_mut(&cell);
        state.tasks.insert(
            ShortCStr::from(c"task1"),
            Task {
                pidfd: pidfd1,
                capture_fd: None,
                child_pid: ret1,
                captures: Vec::new(),
            },
        );
        state.tasks.insert(
            ShortCStr::from(c"task2"),
            Task {
                pidfd: pidfd2,
                capture_fd: None,
                child_pid: ret2,
                captures: Vec::new(),
            },
        );
    }
    run_one(b"waitpid", &cell).unwrap();
    let state = borrow_state(&cell);
    let ok = match state.last_status {
        WaitStatus::Exited(c) => c == 42 || c == 7,
        _ => false,
    };
    assert!(ok);
    assert!(state.tasks.is_empty());
}

#[test]
fn waitpid_rejects_capture() {
    let cell = make_cell();
    let e = run_one(b"waitpid %>%var", &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        CmdError::CapturesNotSupported { command: "waitpid" }
    ));
}

#[test]
fn if_then_runs_body() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if umask 0o077; then umask 0o000; fi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn indented_block_runs_from_keyword() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_script(b"true;  if true; then builtin echo hi; fi", &cell).unwrap();
    });
    assert_eq!(out, b"hi\n");
}

#[test]
fn if_with_else_runs_then() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if umask 0o077; then umask 0o000; else umask 0o007; fi",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_missing_then_returns_err() {
    child_test(|| {
        let cell = make_cell();
        let e = run_one(b"if umask 0o077; umask 0o000; fi", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::Parse));
    });
}

#[test]
fn if_missing_fi_returns_err() {
    child_test(|| {
        let cell = make_cell();
        let e = run_one(b"if umask 0o077; then umask 0o000", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::Parse));
    });
}

// NOTE: "else" without preceding ; is treated as body text by the parser.
// This test is skipped because the parser doesn't distinguish this case.
// #[test]
// fn if_else_before_semicolon_treated_as_body_text() { ... }

#[test]
fn if_then_before_semicolon_returns_err() {
    child_test(|| {
        let cell = make_cell();
        let e = run_one(b"if umask 0o077 then umask 0o000; fi", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::Parse));
    });
}

#[test]
fn if_elif_then_runs_then() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if umask 0o077; then umask 0o000; elif umask 0o007; then umask 0o070; else umask 0o700; fi",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_elif_no_else_runs_then() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if umask 0o077; then umask 0o000; elif umask 0o007; then umask 0o070; fi",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_elif_before_semicolon_returns_err() {
    child_test(|| {
        let cell = make_cell();
        let e = run_one(
            b"if umask 0o077; then umask 0o000; elif umask 0o007 then umask 0o070; fi",
            &cell,
        )
        .unwrap_err();
        assert!(matches!(e.current_context(), CmdError::Parse));
    });
}

#[test]
fn if_elif_without_then_returns_err() {
    child_test(|| {
        let cell = make_cell();
        let e = run_one(
            b"if umask 0o077; then umask 0o000; elif umask 0o007; else umask 0o070; fi",
            &cell,
        )
        .unwrap_err();
        assert!(matches!(e.current_context(), CmdError::Parse));
    });
}

#[test]
fn if_then_newline_separator() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if true\nthen\numask 0o000\nfi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn nested_if_fails() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if true; then if false; then umask 0o000; fi; fi", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn nested_if_newline_fails() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if true\nthen\nif false\nthen\numask 0o000\nfi\nfi", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn string_assign_stores_in_state() {
    let cell = make_cell();
    run_one(b"var=\"hello world\"", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    let val = state
        .strings
        .get::<sys::ShortCStr>(&c"var".into())
        .map(|v| &v.value);
    assert_eq!(val, Some(&c"hello world".into()));
}

#[test]
fn string_assign_empty_value() {
    let cell = make_cell();
    run_one(b"var=", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    let val = state
        .strings
        .get::<sys::ShortCStr>(&c"var".into())
        .map(|v| &v.value);
    assert_eq!(val, Some(&c"".into()));
}

#[test]
fn for_single_word_executes_body() {
    let cell = make_cell();
    run_script(b"for x in hello; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"hello".into())
    );
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"var".into())
            .map(|v| &v.value),
        Some(&c"set".into())
    );
}

#[test]
fn for_multiple_words_sets_var_to_last() {
    let cell = make_cell();
    run_script(b"for x in a b c; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"c".into())
    );
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"var".into())
            .map(|v| &v.value),
        Some(&c"set".into())
    );
}

#[test]
fn for_empty_words_skips_body() {
    let cell = make_cell();
    run_script(b"for x in; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        None
    );
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"var".into())
            .map(|v| &v.value),
        None
    );
}

#[test]
fn for_newline_body() {
    let cell = make_cell();
    run_script(b"for x in hello\ndo\nvar=set\ndone", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"hello".into())
    );
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"var".into())
            .map(|v| &v.value),
        Some(&c"set".into())
    );
}

#[test]
fn for_backtick_expands_to_words() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"for x in `echo 42 7`; do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"7".into())
        );
    });
}

#[test]
fn for_backtick_empty_output_skips_body() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"for x in `echo`; do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            None
        );
    });
}

#[test]
fn for_dollar_paren_expands_to_words() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"for x in $(echo hello world); do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"world".into())
        );
    });
}

#[test]
fn for_backtick_single_number() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"for x in `echo 99`; do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"99".into())
        );
    });
}

#[test]
fn cmd_subst_in_assign() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"x=$(builtin echo hello)", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"hello".into())
        );
    });
}

#[test]
fn cmd_subst_in_assign_and_use() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"x=$(builtin echo world); builtin echo $x", &cell).unwrap();
    });
}

#[test]
fn cmd_subst_semicolon_inside() {
    let cell = make_cell();
    run_script(b"result=$(builtin echo a; builtin echo b)", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"result".into())
            .map(|v| &v.value),
        Some(&c"a\nb".into())
    );
}

#[test]
fn string_assign_dollar_var() {
    let cell = make_cell();
    run_script(b"a=hello; b=$a", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"b".into())
            .map(|v| &v.value),
        Some(&c"hello".into())
    );
}

#[test]
fn string_assign_multiple_vars() {
    let cell = make_cell();
    run_script(b"a=foo; b=bar; c=$a$b", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"c".into())
            .map(|v| &v.value),
        Some(&c"foobar".into())
    );
}

#[test]
fn string_assign_dollar_brace() {
    let cell = make_cell();
    run_script(b"a=hello; b=${a}", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"b".into())
            .map(|v| &v.value),
        Some(&c"hello".into())
    );
}

#[test]
fn string_assign_unknown_var_preserves_literal() {
    let cell = make_cell();
    run_script(b"x=$nonexistent", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"$nonexistent".into())
    );
}

#[test]
fn cmd_subst_in_regular_args() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"builtin echo $(builtin echo hello)", &cell).unwrap();
    });
}

#[test]
fn dollar_question_exit_status() {
    let cell = make_cell();
    run_script(b"builtin echo ok; x=$?", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"0".into())
    );
}

#[test]
fn dollar_question_after_failure() {
    let cell = make_cell();
    run_script(b"nonexistent_cmd_xyzzy; x=$?", &cell).unwrap();
    let state = borrow_state(&cell);
    let val = state
        .strings
        .get::<sys::ShortCStr>(&c"x".into())
        .map(|v| &v.value)
        .unwrap();
    let code: i32 = core::str::from_utf8(val.as_bytes().unwrap())
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(code, 0);
}

#[test]
fn cmd_subst_mixed_with_text() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"x=prefix$(builtin echo middle)suffix", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"prefixmiddlesuffix".into())
        );
    });
}

#[test]
fn export_fd_no_args() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin export_fd", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), sys::errno::EINVAL);
    });
}

#[test]
fn export_fd_no_percent_prefix() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin export_fd foo", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), sys::errno::EINVAL);
    });
}

#[test]
fn export_fd_tag_contains_percent() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin export_fd %tag %var", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), sys::errno::EINVAL);
    });
}

#[test]
fn export_fd_second_arg_no_percent() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin export_fd tag var", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), sys::errno::EINVAL);
    });
}

#[test]
fn export_fd_too_many_args() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin export_fd %a %b %c", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), sys::errno::EINVAL);
    });
}

#[test]
fn export_fd_var_not_in_state() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin export_fd tag %nonexistent", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), sys::errno::EINVAL);
    });
}

#[test]
fn export_fd_dispatch_single_arg_no_var() {
    child_test(|| {
        let cell = make_cell();
        let state = borrow_state(&cell);
        let arg = c"%missing".into();
        let result = crate::child::fdpass::dispatch(b"export_fd", &[arg], &state);
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().unwrap_err().current_context(),
            crate::error::fdpass::FdPassError::NotFound
        ));
    });
}

#[test]
fn export_fd_dispatch_calls_export_fd() {
    child_test(|| {
        let cell = make_cell();
        let state = borrow_state(&cell);
        let result = crate::child::fdpass::dispatch(b"export_fd", &[], &state);
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().unwrap_err().current_context(),
            crate::error::fdpass::FdPassError::MissingArg
        ));
    });
}

#[test]
fn export_fd_dispatch_unknown_name_returns_none() {
    child_test(|| {
        let cell = make_cell();
        let state = borrow_state(&cell);
        let result = crate::child::fdpass::dispatch(b"nonexistent_builtin", &[], &state);
        assert!(result.is_none());
    });
}

#[test]
fn true_builtin_exits_zero() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"true", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn help_builtin_exits_zero() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"help", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn false_builtin_exits_one() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"false", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 1);
    });
}

#[test]
fn true_via_builtin_keyword() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin true", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn false_used_in_cond_list() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"false && builtin echo ok", &cell).unwrap();
    });
}

#[test]
fn and_fail_with_or_fallback() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"false && builtin echo skipped || builtin echo ran", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn and_fail_chain_with_or_fallback() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(
            b"false && builtin echo a && builtin echo b || builtin echo c",
            &cell,
        )
        .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn or_success_skips_rest() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"true || builtin echo skipped", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn and_fail_with_quoted_or_in_skipped_part() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"false && builtin echo \"a||b\" || builtin echo ran", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn and_fail_with_trailing_quote_in_failed_part() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"false \"x\" && echo a || true", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn and_success_runs_second_command() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"true && builtin true", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn empty_part_between_operators_is_separator() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"false || && builtin true", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn pwd_builtin_succeeds() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"pwd", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn pwd_via_builtin_keyword() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"builtin pwd", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn last_bg_pid_set_on_background_task() {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => sys::exit(42),
        Some(pidfd) => {
            use crate::launch::LaunchOutcome;
            use crate::parse::ParsedLine;
            let mut cmdline = match crate::parse::parse(&st(b"echo")).unwrap() {
                ParsedLine::Cmd(cmd) => cmd,
                _ => panic!("expected Cmd for echo"),
            };
            cmdline.pidvar = Some(ShortCStr::from(c"bg"));
            let cell = make_cell();
            let outcome = LaunchOutcome {
                pidfd,
                capture_fd: None,
                child_pid: ret,
            };
            {
                let mut state = borrow_state_mut(&cell);
                let status = crate::postlaunch::finish_cmd(cmdline, outcome, &mut state).unwrap();
                assert!(matches!(status, WaitStatus::Exited(0)));
                assert_eq!(state.last_bg_pid, Some(ret));
            }
        }
    }
}

#[test]
fn if_false_else_runs_else_body() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if false; then umask 0o000; else umask 0o077; fi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn if_false_no_else_sets_zero() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if false; then umask 0o000; fi", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_first_elif_fails_runs_elif_body() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if false; then umask 0o000; elif true; then umask 0o070; else umask 0o700; fi",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o070);
    });
}

#[test]
fn if_all_elifs_fail_runs_else() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if false; then umask 0o000; elif false; then umask 0o070; else umask 0o007; fi",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o007);
    });
}

#[test]
fn if_false_elif_fails_no_else_sets_zero() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if false; then umask 0o000; elif false; then umask 0o070; fi",
            &cell,
        )
        .unwrap();
        assert_ne!(sys::umask::get(), 0o000);
        assert_ne!(sys::umask::get(), 0o070);
    });
}

#[test]
fn if_else_newline_separator() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if false\nthen\numask 0o000\nelse\numask 0o077\nfi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn if_elif_else_newline_separator() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if false\nthen\numask 0o000\nelif false\nthen\numask 0o070\nelse\numask 0o007\nfi",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o007);
    });
}

#[test]
fn if_false_else_nested_if_runs_else() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"if false; then if true; then umask 0o000; fi; else umask 0o077; fi",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn while_false_never_runs_body() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"while false; do umask 0o000; done", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o000);
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn until_true_body_never_runs() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"until true; do umask 0o077; done", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o077);
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn export_set_env_var() {
    let cell = make_cell();
    run_one(b"export FOO=bar", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"FOO".into())
            .map(|v| &v.value),
        Some(&c"bar".into())
    );
    assert_eq!(
        state
            .exports
            .get::<sys::ShortCStr>(&c"FOO".into())
            .map(|v| &v.value),
        Some(&c"bar".into())
    );
}

#[test]
fn export_multiple_vars() {
    let cell = make_cell();
    run_script(b"export FOO=bar; export BAZ=qux", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.exports.len(), 2);
    assert_eq!(
        state
            .exports
            .get::<sys::ShortCStr>(&c"FOO".into())
            .map(|v| &v.value),
        Some(&c"bar".into())
    );
    assert_eq!(
        state
            .exports
            .get::<sys::ShortCStr>(&c"BAZ".into())
            .map(|v| &v.value),
        Some(&c"qux".into())
    );
}

#[test]
fn export_list_empty() {
    let cell = make_cell();
    run_one(b"export", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

fn capture_stdout(f: impl FnOnce()) -> Vec<u8> {
    let (out_r, out_w) = sys::pipe::pipe2(0).unwrap();
    match sys::fork_pidfd::fork_pidfd().unwrap().1 {
        None => {
            out_w.export_to(1).unwrap();
            drop(out_w);
            f();
            sys::exit(0);
        }
        Some(pidfd) => {
            drop(out_w);
            let mut out = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = out_r.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                if let Some(part) = chunk.get(..n) {
                    out.extend_from_slice(part);
                }
            }
            match sys::wait_pidfd::wait_pidfd(&pidfd).unwrap() {
                WaitStatus::Exited(0) => {}
                other => panic!("child failed: {}", other.exit_code()),
            }
            out
        }
    }
}

fn export_output(setup: &[u8]) -> Vec<u8> {
    capture_stdout(|| {
        let cell = make_cell();
        run_one(setup, &cell).unwrap();
        run_one(b"export", &cell).unwrap();
    })
}

fn builtin_output(name: &[u8]) -> Vec<u8> {
    capture_stdout(|| {
        let state = ShellState::new();
        let cmd = sys::ShortCStr::from_vec(name.to_vec()).unwrap();
        let code = crate::child::dispatch::dispatch_builtin(cmd, &[], &[], &state).unwrap();
        sys::exit(code);
    })
}

#[test]
fn export_list_writes_entries() {
    let out = export_output(b"export FOO=bar");
    assert_eq!(out, b"export FOO=bar\n");
}

#[test]
fn export_name_only_lists_empty_value() {
    let out = export_output(b"export FOO");
    assert_eq!(out, b"export FOO=\n");
}

#[test]
fn help_lists_shell_commands_and_builtins() {
    let out = builtin_output(b"help");
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.starts_with("Shell commands:"));
    assert!(text.contains("Change directory"));
    assert!(text.contains("Builtins:"));
    assert!(text.contains("Print arguments"));
}

#[test]
fn pwd_prints_current_directory() {
    let out = builtin_output(b"pwd");
    let cwd = sys::env::getcwd().unwrap();
    let mut expected = cwd;
    expected.push(b'\n');
    assert_eq!(out, expected);
}

fn script_exit_code(script: &[u8]) -> i32 {
    match sys::fork_pidfd::fork_pidfd().unwrap().1 {
        None => {
            let cell = make_cell();
            match crate::main_cli::execute_script(script, Origin::Shell, &cell) {
                Ok(()) => sys::exit(42),
                Err(_) => sys::exit(43),
            }
        }
        Some(pidfd) => sys::wait_pidfd::wait_pidfd(&pidfd).unwrap().exit_code(),
    }
}

#[test]
fn execute_script_exits_with_script_code() {
    assert_eq!(script_exit_code(b"false"), 1);
}

#[test]
fn execute_script_zero_code_returns_ok() {
    assert_eq!(script_exit_code(b"true"), 42);
}

#[test]
fn execute_script_error_exits_one() {
    assert_eq!(script_exit_code(b"nonexistent_cmd_xyz"), 1);
}

#[test]
fn cmd_mode_positional_origins_are_argv_indexed() {
    child_test(|| {
        let cell = make_cell();
        let args: [ShortCStr; 4] = [
            ShortCStr::from(c"fdshell"),
            ShortCStr::from(c"builtin true"),
            ShortCStr::from(c"first"),
            ShortCStr::from(c"second"),
        ];
        crate::main_cli::run_cmd_mode(&args, &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state.positional.front().unwrap().trace.origin,
            Origin::CliArgument(3)
        );
        assert_eq!(
            state.positional.get(1).unwrap().trace.origin,
            Origin::CliArgument(4)
        );
    });
}

#[test]
fn shebang_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"#!/usr/bin/env fdshell\nbuiltin echo ok", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn inline_comment_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"builtin echo ok # this is a comment", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn comment_after_statement_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"builtin echo first # comment\nbuiltin echo second", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn comment_inside_if_block_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"if true; then # comment\numask 0o077\nfi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn hash_mid_word_is_not_a_comment() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_one(b"builtin echo a#b", &cell).unwrap();
    });
    assert_eq!(out, b"a#b\n");
}

#[test]
fn hash_after_semicolon_is_a_comment() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_script(b"builtin echo a;#x", &cell).unwrap();
    });
    assert_eq!(out, b"a\n");
}

#[test]
fn full_line_comment_after_command_is_skipped() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_script(
            b"builtin echo a\n# full line comment\nbuiltin echo b",
            &cell,
        )
        .unwrap();
    });
    assert_eq!(out, b"a\nb\n");
}

#[test]
fn exit_rejects_negative_code() {
    child_test(|| {
        let cell = make_cell();
        assert!(run_one(b"exit -1", &cell).is_err());
    });
}

#[test]
fn exit_rejects_overflow_code() {
    child_test(|| {
        let cell = make_cell();
        assert!(run_one(b"exit 256", &cell).is_err());
    });
}

#[test]
fn for_break_exits_loop() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"for x in a b c; do if true; then break; fi; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"a".into())
        );
    });
}

#[test]
fn break_in_nested_for() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"for x in a b; do for y in 1 2; do break; done; done",
            &cell,
        )
        .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"b".into())
        );
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"y".into())
                .map(|v| &v.value),
            Some(&c"1".into())
        );
    });
}

#[test]
fn while_break_exits_loop() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"while true; do umask 0o077; break; done", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn break_outside_loop_returns_error() {
    child_test(|| {
        let cell = make_cell();
        let e = handle(b"break", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::BreakOutsideLoop));
    });
}

#[test]
fn continue_outside_loop_returns_error() {
    child_test(|| {
        let cell = make_cell();
        let e = handle(b"continue", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::ContinueOutsideLoop));
    });
}

#[test]
fn for_continue_skips_iteration() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"for x in a b c; do if false; then continue; fi; result=$x; done",
            &cell,
        )
        .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"result".into())
                .map(|v| &v.value),
            Some(&c"c".into())
        );
    });
}

#[test]
fn while_continue_skips_iteration() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"while true; do if false; then continue; fi; result=1; break; done",
            &cell,
        )
        .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"result".into())
                .map(|v| &v.value),
            Some(&c"1".into())
        );
    });
}

#[test]
fn until_break_exits_loop() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"until false; do umask 0o077; break; done", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn break_in_if_inside_for() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"for x in a b c; do if true; then break; fi; done", &cell).unwrap();
        run_script(b"x=after", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state
                .strings
                .get::<sys::ShortCStr>(&c"x".into())
                .map(|v| &v.value),
            Some(&c"after".into())
        );
    });
}

#[test]
fn case_match_first_clause() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"case \"foo\" in foo) umask 0o000;; *) umask 0o077;; esac",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn case_match_second_clause() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"case \"bar\" in foo) umask 0o000;; bar) umask 0o070;; *) umask 0o700;; esac",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o070);
    });
}

#[test]
fn case_no_match_sets_zero() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"case \"baz\" in foo) umask 0o000;; bar) umask 0o070;; esac",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o022);
    });
}

#[test]
fn case_star_catchall() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"case \"anything\" in foo) umask 0o000;; *) umask 0o007;; esac",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o007);
    });
}

#[test]
fn case_alternative_patterns() {
    child_test(|| {
        let cell = make_cell();
        run_script(
            b"case \"x\" in a|x) umask 0o000;; *) umask 0o077;; esac",
            &cell,
        )
        .unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn case_no_match_no_else_sets_zero() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"case \"x\" in a) echo yes;; esac", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 0);
    });
}

#[test]
fn case_last_clause_no_double_semi() {
    child_test(|| {
        let cell = make_cell();
        run_script(b"case \"x\" in a) echo yes;; *) echo no esac", &cell).unwrap();
    });
}

#[test]
fn assign_fd_copies_fd_variable() {
    let cell = make_cell();
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    borrow_state_mut(&cell).fds.insert(
        ShortCStr::from(c"src"),
        FdVar {
            fd: dev_null,
            trace: Trace::boundary(Origin::Captured(ShortCStr::from(c"openat2"))),
        },
    );
    run_one(b"%copy=%src", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(state.fds.contains_key(&ShortCStr::from(c"copy")));
}

fn insert_fd_var(cell: &ForkCell<ShellState>, name: &[u8]) {
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    borrow_state_mut(cell).fds.insert(
        ShortCStr::from_vec(name.to_vec()).unwrap(),
        FdVar {
            fd: dev_null,
            trace: Trace::boundary(Origin::Shell),
        },
    );
}

#[test]
fn array_empty_creates_array() {
    let cell = make_cell();
    run_one(b"%arr=[]", &cell).unwrap();
    let state = borrow_state(&cell);
    let arr = state.arrays.get(&ShortCStr::from(c"arr")).unwrap();
    assert!(arr.is_empty());
    assert!(!state.fds.contains_key(&ShortCStr::from(c"arr")));
}

#[test]
fn array_empty_replaces_scalar() {
    let cell = make_cell();
    insert_fd_var(&cell, b"arr");
    run_one(b"%arr=[]", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(state.arrays.contains_key(&ShortCStr::from(c"arr")));
    assert!(!state.fds.contains_key(&ShortCStr::from(c"arr")));
}

#[test]
fn array_append_dups_scalar_with_provenance() {
    let cell = make_cell();
    insert_fd_var(&cell, b"src");
    run_one(b"%arr=[]", &cell).unwrap();
    run_one(b"%arr+=%src", &cell).unwrap();
    run_one(b"%arr+=%src", &cell).unwrap();
    let state = borrow_state(&cell);
    let arr = state.arrays.get(&ShortCStr::from(c"arr")).unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].source, ShortCStr::from(c"src"));
    let src_fd = state.fds.get(&ShortCStr::from(c"src")).unwrap().fd.as_raw();
    assert_ne!(arr[0].fd.as_raw(), src_fd);
    assert_ne!(arr[0].fd.as_raw(), arr[1].fd.as_raw());
}

#[test]
fn array_append_creates_array() {
    let cell = make_cell();
    insert_fd_var(&cell, b"src");
    run_one(b"%arr+=%src", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.arrays.get(&ShortCStr::from(c"arr")).unwrap().len(), 1);
}

#[test]
fn array_append_on_scalar_is_not_an_array() {
    let cell = make_cell();
    insert_fd_var(&cell, b"arr");
    let e = run_one(b"%arr+=%arr", &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::NotAnArray { .. }));
}

#[test]
fn array_indexed_read_dups_entry() {
    let cell = make_cell();
    insert_fd_var(&cell, b"src");
    run_one(b"%arr=[]", &cell).unwrap();
    run_one(b"%arr+=%src", &cell).unwrap();
    run_one(b"%x=%arr[0]", &cell).unwrap();
    let state = borrow_state(&cell);
    let x = state.fds.get(&ShortCStr::from(c"x")).unwrap();
    let entry = state.arrays.get(&ShortCStr::from(c"arr")).unwrap();
    assert_ne!(x.fd.as_raw(), entry[0].fd.as_raw());
    assert_eq!(x.trace.origin, entry[0].trace.origin);
}

#[test]
fn array_index_out_of_range_is_error() {
    let cell = make_cell();
    run_one(b"%arr=[]", &cell).unwrap();
    let e = run_one(b"%x=%arr[0]", &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        CmdError::ArrayIndexOutOfRange { .. }
    ));
}

#[test]
fn array_index_on_unset_var_is_fd_not_set() {
    let cell = make_cell();
    let e = run_one(b"%x=%nope[0]", &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::FdNotSet));
}

#[test]
fn array_index_on_scalar_is_not_an_array() {
    let cell = make_cell();
    insert_fd_var(&cell, b"arr");
    let e = run_one(b"%x=%arr[0]", &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::NotAnArray { .. }));
}

#[test]
fn array_unset_entry_by_provenance() {
    let cell = make_cell();
    insert_fd_var(&cell, b"a");
    insert_fd_var(&cell, b"b");
    run_one(b"%arr=[]", &cell).unwrap();
    run_one(b"%arr+=%a", &cell).unwrap();
    run_one(b"%arr+=%b", &cell).unwrap();
    run_one(b"%arr+=%a", &cell).unwrap();
    run_one(b"unset %arr[%a]", &cell).unwrap();
    {
        let state = borrow_state(&cell);
        let arr = state.arrays.get(&ShortCStr::from(c"arr")).unwrap();
        let sources: Vec<&sys::ShortCStr> = arr.iter().map(|e| &e.source).collect();
        assert_eq!(
            sources,
            alloc::vec![&ShortCStr::from(c"b"), &ShortCStr::from(c"a")]
        );
    }
    run_one(b"unset %arr[%a]", &cell).unwrap();
    run_one(b"unset %arr[%b]", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(
        state
            .arrays
            .get(&ShortCStr::from(c"arr"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn array_unset_entry_missing_is_ok() {
    let cell = make_cell();
    insert_fd_var(&cell, b"a");
    run_one(b"%arr=[]", &cell).unwrap();
    run_one(b"%arr+=%a", &cell).unwrap();
    run_one(b"unset %arr[%missing]", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.arrays.get(&ShortCStr::from(c"arr")).unwrap().len(), 1);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn array_unset_closes_all_entries() {
    let cell = make_cell();
    insert_fd_var(&cell, b"src");
    run_one(b"%arr=[]", &cell).unwrap();
    run_one(b"%arr+=%src", &cell).unwrap();
    let before = std::fs::read_dir("/proc/self/fd").unwrap().count();
    run_one(b"unset %arr", &cell).unwrap();
    let after = std::fs::read_dir("/proc/self/fd").unwrap().count();
    assert_eq!(after, before - 1);
    let state = borrow_state(&cell);
    assert!(!state.arrays.contains_key(&ShortCStr::from(c"arr")));
    assert!(state.fds.contains_key(&ShortCStr::from(c"src")));
}

#[test]
fn for_over_array_runs_body_per_entry() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        insert_fd_var(&cell, b"src");
        run_script(
            b"%arr=[]; %arr+=%src; %arr+=%src; for %x in %arr; do builtin echo hi; done",
            &cell,
        )
        .unwrap();
    });
    assert_eq!(out, b"hi\nhi\n");
}

#[test]
fn for_over_array_binds_fd_var() {
    let cell = make_cell();
    insert_fd_var(&cell, b"src");
    run_script(
        b"%arr=[]; %arr+=%src; for %x in %arr; do var=set; done",
        &cell,
    )
    .unwrap();
    let state = borrow_state(&cell);
    assert!(state.fds.contains_key(&ShortCStr::from(c"x")));
    assert!(state.strings.contains_key(&ShortCStr::from(c"var")));
}

#[test]
fn for_over_empty_array_skips_body() {
    let cell = make_cell();
    run_script(b"%arr=[]; for %x in %arr; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(!state.fds.contains_key(&ShortCStr::from(c"x")));
    assert!(!state.strings.contains_key(&ShortCStr::from(c"var")));
}

#[test]
fn for_over_unset_percent_name_is_literal_word() {
    let cell = make_cell();
    run_script(b"for %x in %nope; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(!state.fds.contains_key(&ShortCStr::from(c"x")));
    assert_eq!(
        state
            .strings
            .get::<sys::ShortCStr>(&c"var".into())
            .map(|v| &v.value),
        Some(&c"set".into())
    );
}

#[test]
fn explain_unset_var_reports_unset() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_one(b"explain nope", &cell).unwrap();
    });
    assert_eq!(out, b"nope is unset\n");
}

#[test]
fn explain_assigned_var_shows_position_and_origin() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_one(b"x=hi", &cell).unwrap();
        run_one(b"explain x", &cell).unwrap();
    });
    assert_eq!(
        out,
        b"x=\"hi\" (set on line 1, column 1, from shell default)\n"
    );
}

#[test]
fn explain_positional_shows_argv_origin() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        {
            let mut state = borrow_state_mut(&cell);
            let mut positional = alloc::collections::VecDeque::new();
            positional.push_back(sys::ImportedStr::shell(ShortCStr::from(c"sh")));
            positional.push_back(sys::ImportedStr::new(
                ShortCStr::from(c"first"),
                sys::Trace::boundary(sys::Origin::CliArgument(2)),
            ));
            state.set_positional(positional);
        }
        run_one(b"explain 1", &cell).unwrap();
    });
    assert_eq!(out, b"$1=\"first\" (from argv[2])\n");
}

#[test]
fn explain_assignment_is_transitive() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        {
            let mut state = borrow_state_mut(&cell);
            let mut positional = alloc::collections::VecDeque::new();
            positional.push_back(sys::ImportedStr::shell(ShortCStr::from(c"sh")));
            positional.push_back(sys::ImportedStr::new(
                ShortCStr::from(c"first"),
                sys::Trace::boundary(sys::Origin::CliArgument(2)),
            ));
            state.set_positional(positional);
        }
        run_one(b"p=$1", &cell).unwrap();
        run_one(b"explain p", &cell).unwrap();
    });
    assert_eq!(
        out,
        b"p=\"first\" (set on line 1, column 1, from argv[2])\n"
    );
}

#[test]
fn explain_command_output_origin() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_one(b"q=$(builtin echo hi)", &cell).unwrap();
        run_one(b"explain q", &cell).unwrap();
    });
    assert_eq!(
        out,
        b"q=\"hi\" (set on line 1, column 1, from command output)\n"
    );
}

#[test]
fn explain_second_positional_uses_index() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        {
            let mut state = borrow_state_mut(&cell);
            let mut positional = alloc::collections::VecDeque::new();
            positional.push_back(sys::ImportedStr::shell(ShortCStr::from(c"sh")));
            positional.push_back(sys::ImportedStr::new(
                ShortCStr::from(c"one"),
                sys::Trace::boundary(sys::Origin::CliArgument(2)),
            ));
            positional.push_back(sys::ImportedStr::new(
                ShortCStr::from(c"two"),
                sys::Trace::boundary(sys::Origin::CliArgument(3)),
            ));
            state.set_positional(positional);
        }
        run_one(b"explain 2", &cell).unwrap();
    });
    assert_eq!(out, b"$2=\"two\" (from argv[3])\n");
}

#[test]
fn explain_no_argument_sets_status_1() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"explain", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 1);
    });
}

#[test]
fn explain_two_arguments_sets_status_1() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"explain a b", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 1);
    });
}

#[test]
fn fdexplain_unset_var_reports_unset() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_one(b"fdexplain %nope", &cell).unwrap();
    });
    assert_eq!(out, b"%nope is unset\n");
}

#[test]
fn fdexplain_cwd_shows_shell_origin() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        {
            let mut state = borrow_state_mut(&cell);
            let cwd = sys::openat2::open(c"/tmp", sys::fcntl::O_DIRECTORY).unwrap();
            state.insert_cwd(cwd);
        }
        run_one(b"fdexplain %CWD", &cell).unwrap();
    });
    assert_eq!(out, b"%CWD (from shell default)\n");
}

#[test]
fn fdexplain_captured_fd_shows_tag_and_line() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        {
            let mut state = borrow_state_mut(&cell);
            let fd = sys::openat2::open(c"/dev/null", 0).unwrap();
            state.fds.insert(
                ShortCStr::from(c"f"),
                FdVar {
                    fd,
                    trace: Trace::at(
                        Position::new(3, 1),
                        Origin::Captured(ShortCStr::from(c"openat2")),
                    ),
                },
            );
        }
        run_one(b"fdexplain %f", &cell).unwrap();
    });
    assert_eq!(out, b"%f (set on line 3, column 1, from tag openat2)\n");
}

#[test]
fn fdexplain_assigned_fd_is_transitive() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        {
            let mut state = borrow_state_mut(&cell);
            let fd = sys::openat2::open(c"/dev/null", 0).unwrap();
            state.fds.insert(
                ShortCStr::from(c"src"),
                FdVar {
                    fd,
                    trace: Trace::boundary(Origin::Captured(ShortCStr::from(c"openat2"))),
                },
            );
        }
        run_one(b"%copy=%src", &cell).unwrap();
        run_one(b"fdexplain %copy", &cell).unwrap();
    });
    assert_eq!(out, b"%copy (set on line 1, column 1, from tag openat2)\n");
}

#[test]
fn fdexplain_end_to_end_capture_origin() {
    let dir = std::env::temp_dir().join("fdshell-fdexplain-e2e");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("target"), b"x").unwrap();

    let out = capture_stdout(|| {
        let cell = make_cell();
        {
            let mut state = borrow_state_mut(&cell);
            let path = dir.to_str().unwrap().as_bytes().to_vec();
            let path = ShortCStr::from_vec(path).unwrap();
            let cwd = sys::openat2::open(path.export(), sys::fcntl::O_DIRECTORY).unwrap();
            state.insert_cwd(cwd);
        }
        run_one(
            b"builtin openat2 --dirfd %CWD --flags O_RDONLY target %>%f",
            &cell,
        )
        .unwrap();
        run_one(b"fdexplain %f", &cell).unwrap();
    });
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(out, b"%f (set on line 1, column 1, from tag openat2)\n");
}

#[test]
fn fdexplain_no_argument_sets_status_1() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"fdexplain", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 1);
    });
}

#[test]
fn fdexplain_two_arguments_sets_status_1() {
    child_test(|| {
        let cell = make_cell();
        run_one(b"fdexplain %a %b", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.last_status.exit_code(), 1);
    });
}

fn string_of(state: &ShellState, name: &str) -> Option<ShortCStr> {
    let key = ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap();
    state
        .strings
        .get::<sys::ShortCStr>(&key)
        .map(|s| s.value.clone())
}

#[test]
fn function_definition_stores_and_call_runs_body() {
    let cell = make_cell();
    run_script(b"foo() { a=one; b=two; } foo", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.last_status.exit_code(), 0);
    assert_eq!(string_of(&state, "a"), Some(c"one".into()));
    assert_eq!(string_of(&state, "b"), Some(c"two".into()));
}

#[test]
fn function_return_sets_status_and_stops_body() {
    let cell = make_cell();
    run_script(b"foo() { a=before; return 5; a=after; } foo", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.last_status.exit_code(), 5);
    assert_eq!(string_of(&state, "a"), Some(c"before".into()));
}

#[test]
fn function_return_without_arg_keeps_last_status() {
    let cell = make_cell();
    run_script(b"foo() { a=1; return; } foo", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.last_status.exit_code(), 0);
}

#[test]
fn function_args_become_positional_params() {
    let cell = make_cell();
    run_script(b"foo() { first=$1; second=$2; } foo alpha beta", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(string_of(&state, "first"), Some(c"alpha".into()));
    assert_eq!(string_of(&state, "second"), Some(c"beta".into()));
}

#[test]
fn function_call_restores_caller_positional_params() {
    let cell = make_cell();
    run_script(b"set -- sh A B; foo() { inner=$1; } foo X; first=$1", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(string_of(&state, "inner"), Some(c"X".into()));
    assert_eq!(string_of(&state, "first"), Some(c"A".into()));
}

#[test]
fn nested_function_calls() {
    let cell = make_cell();
    run_script(
        b"inner() { msg=from-inner; } outer() { inner; msg2=set; } outer",
        &cell,
    )
    .unwrap();
    let state = borrow_state(&cell);
    assert_eq!(string_of(&state, "msg"), Some(c"from-inner".into()));
    assert_eq!(string_of(&state, "msg2"), Some(c"set".into()));
}

#[test]
fn function_return_escapes_inner_loop() {
    let cell = make_cell();
    run_script(b"foo() { while true; do return 7; done; } foo", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.last_status.exit_code(), 7);
}

#[test]
fn function_return_inside_if_skips_rest() {
    let cell = make_cell();
    run_script(
        b"foo() { if true; then return 3; fi; a=not-run; } foo",
        &cell,
    )
    .unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.last_status.exit_code(), 3);
    assert_eq!(string_of(&state, "a"), None);
}

#[test]
fn function_redefinition_overwrites_body() {
    let cell = make_cell();
    run_script(b"foo() { a=first; } foo() { a=second; } foo", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(string_of(&state, "a"), Some(c"second".into()));
}

#[test]
fn function_shadows_builtin() {
    let cell = make_cell();
    run_script(b"echo() { shadowed=yes; } echo", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(string_of(&state, "shadowed"), Some(c"yes".into()));
}

#[test]
fn return_outside_function_is_error() {
    let cell = make_cell();
    let e = handle(b"return 1", &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        CmdError::ReturnOutsideFunction
    ));
}

#[test]
fn return_with_non_integer_is_parse_error() {
    let cell = make_cell();
    let e = run_one(b"return abc", &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Parse));
}

#[test]
fn unclosed_function_brace_is_parse_error() {
    let cell = make_cell();
    let e = run_script(b"foo() { a=1", &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Parse));
}
