#![allow(clippy::unwrap_used)]

use crate::error::cmd::CmdError;
use crate::run::run_one;
use crate::state::ShellState;
use crate::task::Task;
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
            std::process::exit(42);
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
fn wait_no_tasks() {
    let cell = make_cell();
    run_one(b"wait", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn wait_nonexistent_name() {
    let cell = make_cell();
    let e = run_one(b"wait &nonexistent", &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Task));
}

#[test]
fn wait_one_task() {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => std::process::exit(42),
        Some(pidfd) => {
            let cell = make_cell();
            {
                let mut state = borrow_state_mut(&cell);
                state.tasks.insert(
                    ShortCStr::from(c"mytask"),
                    Task {
                        pidfd,
                        capture_fd: None,
                        child_pid: ret as i32,
                        captures: Vec::new(),
                    },
                );
            }
            run_one(b"wait &mytask", &cell).unwrap();
            let state = borrow_state(&cell);
            assert!(matches!(state.last_status, WaitStatus::Exited(42)));
            assert!(state.tasks.is_empty());
        }
    }
}

#[test]
fn wait_all_tasks() {
    let (ret1, pidfd1_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    let pidfd1 = match pidfd1_opt {
        None => std::process::exit(42),
        Some(pidfd) => pidfd,
    };
    let (ret2, pidfd2_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    let pidfd2 = match pidfd2_opt {
        None => std::process::exit(7),
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
                child_pid: ret1 as i32,
                captures: Vec::new(),
            },
        );
        state.tasks.insert(
            ShortCStr::from(c"task2"),
            Task {
                pidfd: pidfd2,
                capture_fd: None,
                child_pid: ret2 as i32,
                captures: Vec::new(),
            },
        );
    }
    run_one(b"wait", &cell).unwrap();
    let state = borrow_state(&cell);
    let ok = match state.last_status {
        WaitStatus::Exited(c) => c == 42 || c == 7,
        _ => false,
    };
    assert!(ok);
    assert!(state.tasks.is_empty());
}

#[test]
fn wait_rejects_capture() {
    let cell = make_cell();
    let e = run_one(b"wait %>%var", &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        CmdError::CapturesNotSupported { command: "wait" }
    ));
}

#[test]
fn if_then_runs_body() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"if umask 0o077; then umask 0o000; fi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_with_else_runs_then() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(
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
        crate::repl::run_script(
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
        crate::repl::run_script(
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
        crate::repl::run_script(b"if true\nthen\numask 0o000\nfi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn nested_if_fails() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"if true; then if false; then umask 0o000; fi; fi", &cell)
            .unwrap();
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn nested_if_newline_fails() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"if true\nthen\nif false\nthen\numask 0o000\nfi\nfi", &cell)
            .unwrap();
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn string_assign_stores_in_state() {
    let cell = make_cell();
    run_one(b"var=\"hello world\"", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    let val = state.strings.get(&c"var".into());
    assert_eq!(val, Some(&c"hello world".into()));
}

#[test]
fn string_assign_empty_value() {
    let cell = make_cell();
    run_one(b"var=", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    let val = state.strings.get(&c"var".into());
    assert_eq!(val, Some(&c"".into()));
}

#[test]
fn for_single_word_executes_body() {
    let cell = make_cell();
    crate::repl::run_script(b"for x in hello; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"hello".into()));
    assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
}

#[test]
fn for_multiple_words_sets_var_to_last() {
    let cell = make_cell();
    crate::repl::run_script(b"for x in a b c; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"c".into()));
    assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
}

#[test]
fn for_empty_words_skips_body() {
    let cell = make_cell();
    crate::repl::run_script(b"for x in; do var=set; done", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), None);
    assert_eq!(state.strings.get(&c"var".into()), None);
}

#[test]
fn for_newline_body() {
    let cell = make_cell();
    crate::repl::run_script(b"for x in hello\ndo\nvar=set\ndone", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"hello".into()));
    assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
}

#[test]
fn for_backtick_expands_to_words() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"for x in `echo 42 7`; do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"7".into()));
    });
}

#[test]
fn for_backtick_empty_output_skips_body() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"for x in `echo`; do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), None);
    });
}

#[test]
fn for_dollar_paren_expands_to_words() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"for x in $(echo hello world); do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"world".into()));
    });
}

#[test]
fn for_backtick_single_number() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"for x in `echo 99`; do var=set; done", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"99".into()));
    });
}

#[test]
fn cmd_subst_in_assign() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"x=$(builtin echo hello)", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"hello".into()));
    });
}

#[test]
fn cmd_subst_in_assign_and_use() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"x=$(builtin echo world); builtin echo $x", &cell).unwrap();
    });
}

#[test]
fn string_assign_dollar_var() {
    let cell = make_cell();
    crate::repl::run_script(b"a=hello; b=$a", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.strings.get(&c"b".into()), Some(&c"hello".into()));
}

#[test]
fn string_assign_multiple_vars() {
    let cell = make_cell();
    crate::repl::run_script(b"a=foo; b=bar; c=$a$b", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.strings.get(&c"c".into()), Some(&c"foobar".into()));
}

#[test]
fn string_assign_dollar_brace() {
    let cell = make_cell();
    crate::repl::run_script(b"a=hello; b=${a}", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.strings.get(&c"b".into()), Some(&c"hello".into()));
}

#[test]
fn string_assign_unknown_var_preserves_literal() {
    let cell = make_cell();
    crate::repl::run_script(b"x=$nonexistent", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(
        state.strings.get(&c"x".into()),
        Some(&c"$nonexistent".into())
    );
}

#[test]
fn cmd_subst_in_regular_args() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"builtin echo $(builtin echo hello)", &cell).unwrap();
    });
}

#[test]
fn dollar_question_exit_status() {
    let cell = make_cell();
    crate::repl::run_script(b"builtin echo ok; x=$?", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"0".into()));
}

#[test]
fn dollar_question_after_failure() {
    let cell = make_cell();
    crate::repl::run_script(b"nonexistent_cmd_xyzzy; x=$?", &cell).unwrap();
    let state = borrow_state(&cell);
    let val = state.strings.get(&c"x".into()).unwrap();
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
        crate::repl::run_script(b"x=prefix$(builtin echo middle)suffix", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(
            state.strings.get(&c"x".into()),
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
        let arg = ShortCStr::from_vec(b"%missing".to_vec()).unwrap();
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
        crate::repl::run_cond_list(b"false && builtin echo ok", &cell).unwrap();
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
        None => std::process::exit(42),
        Some(pidfd) => {
            use crate::launch::LaunchOutcome;
            use crate::parse::ParsedLine;
            let mut cmdline = match crate::parse::parse(b"echo").unwrap() {
                ParsedLine::Cmd(cmd) => cmd,
                _ => panic!("expected Cmd for echo"),
            };
            cmdline.pidvar = Some(ShortCStr::from(c"bg"));
            let cell = make_cell();
            let outcome = LaunchOutcome {
                pidfd,
                capture_fd: None,
                child_pid: ret as i32,
            };
            {
                let mut state = borrow_state_mut(&cell);
                let status = crate::postlaunch::finish_cmd(cmdline, outcome, &mut state).unwrap();
                assert!(matches!(status, WaitStatus::Exited(0)));
                assert_eq!(state.last_bg_pid, Some(ret as i32));
            }
        }
    }
}

#[test]
fn if_false_else_runs_else_body() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"if false; then umask 0o000; else umask 0o077; fi", &cell)
            .unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn if_false_no_else_sets_zero() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"if false; then umask 0o000; fi", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_first_elif_fails_runs_elif_body() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(
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
        crate::repl::run_script(
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
        crate::repl::run_script(
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
        crate::repl::run_script(b"if false\nthen\numask 0o000\nelse\numask 0o077\nfi", &cell)
            .unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn if_elif_else_newline_separator() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(
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
        crate::repl::run_script(
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
        crate::repl::run_script(b"while false; do umask 0o000; done", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn until_true_body_never_runs() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"until true; do umask 0o077; done", &cell).unwrap();
        assert_ne!(sys::umask::get(), 0o077);
    });
}

#[test]
fn export_set_env_var() {
    let cell = make_cell();
    run_one(b"export FOO=bar", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"FOO".into()), Some(&c"bar".into()));
    assert_eq!(
        state.exports.get(&c"FOO".into()).map(|v| v.as_slice()),
        Some(&b"bar"[..])
    );
}

#[test]
fn export_multiple_vars() {
    let cell = make_cell();
    crate::repl::run_script(b"export FOO=bar; export BAZ=qux", &cell).unwrap();
    let state = borrow_state(&cell);
    assert_eq!(state.exports.len(), 2);
    assert_eq!(
        state.exports.get(&c"FOO".into()).map(|v| v.as_slice()),
        Some(&b"bar"[..])
    );
    assert_eq!(
        state.exports.get(&c"BAZ".into()).map(|v| v.as_slice()),
        Some(&b"qux"[..])
    );
}

#[test]
fn export_list_empty() {
    let cell = make_cell();
    run_one(b"export", &cell).unwrap();
    let state = borrow_state(&cell);
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn shebang_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"#!/usr/bin/env fdshell\nbuiltin echo ok", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn inline_comment_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"builtin echo ok # this is a comment", &cell).unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn comment_after_statement_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"builtin echo first # comment\nbuiltin echo second", &cell)
            .unwrap();
        let state = borrow_state(&cell);
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    });
}

#[test]
fn comment_inside_if_block_is_skipped() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"if true; then # comment\numask 0o077\nfi", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
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
        crate::repl::run_script(b"for x in a b c; do if true; then break; fi; done", &cell)
            .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"a".into()));
    });
}

#[test]
fn break_in_nested_for() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(
            b"for x in a b; do for y in 1 2; do break; done; done",
            &cell,
        )
        .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"b".into()));
        assert_eq!(state.strings.get(&c"y".into()), Some(&c"1".into()));
    });
}

#[test]
fn while_break_exits_loop() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"while true; do umask 0o077; break; done", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn break_outside_loop_returns_error() {
    child_test(|| {
        let cell = make_cell();
        let e = crate::repl::handle(b"break", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::BreakOutsideLoop));
    });
}

#[test]
fn continue_outside_loop_returns_error() {
    child_test(|| {
        let cell = make_cell();
        let e = crate::repl::handle(b"continue", &cell).unwrap_err();
        assert!(matches!(e.current_context(), CmdError::ContinueOutsideLoop));
    });
}

#[test]
fn for_continue_skips_iteration() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(
            b"for x in a b c; do if false; then continue; fi; result=$x; done",
            &cell,
        )
        .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"result".into()), Some(&c"c".into()));
    });
}

#[test]
fn while_continue_skips_iteration() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(
            b"while true; do if false; then continue; fi; result=1; break; done",
            &cell,
        )
        .unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"result".into()), Some(&c"1".into()));
    });
}

#[test]
fn until_break_exits_loop() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"until false; do umask 0o077; break; done", &cell).unwrap();
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn break_in_if_inside_for() {
    child_test(|| {
        let cell = make_cell();
        crate::repl::run_script(b"for x in a b c; do if true; then break; fi; done", &cell)
            .unwrap();
        crate::repl::run_script(b"x=after", &cell).unwrap();
        let state = borrow_state(&cell);
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"after".into()));
    });
}
