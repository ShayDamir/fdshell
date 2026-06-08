#![allow(clippy::unwrap_used)]

use crate::run::run_one;
use crate::state::ShellState;
use crate::task::Task;
use sys::ShortCStr;
use sys::errno::EINVAL;
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

#[test]
fn umask_set_via_run_one() {
    child_test(|| {
        let mut state = ShellState::new();
        run_one(b"umask 0o077", &mut state).unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn umask_set_zero_via_run_one() {
    child_test(|| {
        let mut state = ShellState::new();
        run_one(b"umask 0o000", &mut state).unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn umask_set_without_o_prefix() {
    child_test(|| {
        let mut state = ShellState::new();
        run_one(b"umask 077", &mut state).unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn umask_invalid_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(b"umask abc", &mut state).unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn umask_too_many_args_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(b"umask 0o077 extra", &mut state).unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn wait_no_tasks() {
    let mut state = ShellState::new();
    run_one(b"wait", &mut state).unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn wait_nonexistent_name() {
    let mut state = ShellState::new();
    let e = run_one(b"wait &nonexistent", &mut state).unwrap_err();
    assert_eq!(e, EINVAL);
}

#[test]
fn wait_one_task() {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => std::process::exit(42),
        Some(pidfd) => {
            let mut state = ShellState::new();
            state.tasks.insert(
                ShortCStr::from(c"mytask"),
                Task {
                    pidfd,
                    capture_fd: None,
                    child_pid: ret as i32,
                    captures: Vec::new(),
                },
            );
            run_one(b"wait &mytask", &mut state).unwrap();
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
    let mut state = ShellState::new();
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
    run_one(b"wait", &mut state).unwrap();
    let ok = match state.last_status {
        WaitStatus::Exited(c) => c == 42 || c == 7,
        _ => false,
    };
    assert!(ok);
    assert!(state.tasks.is_empty());
}

#[test]
fn wait_rejects_capture() {
    let mut state = ShellState::new();
    let e = run_one(b"wait %>%var", &mut state).unwrap_err();
    assert_eq!(e, EINVAL);
}

#[test]
fn if_then_runs_body() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"if umask 0o077; then umask 0o000; fi", &mut state).unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_with_else_runs_then() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(
            b"if umask 0o077; then umask 0o000; else umask 0o007; fi",
            &mut state,
        )
        .unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_missing_then_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(b"if umask 0o077; umask 0o000; fi", &mut state).unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn if_missing_fi_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(b"if umask 0o077; then umask 0o000", &mut state).unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn if_else_before_semicolon_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(
            b"if umask 0o077; then umask 0o000 else umask 0o007; fi",
            &mut state,
        )
        .unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn if_then_before_semicolon_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(b"if umask 0o077 then umask 0o000; fi", &mut state).unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn if_elif_then_runs_then() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(
            b"if umask 0o077; then umask 0o000; elif umask 0o007; then umask 0o070; else umask 0o700; fi",
            &mut state,
        )
        .unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_elif_no_else_runs_then() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(
            b"if umask 0o077; then umask 0o000; elif umask 0o007; then umask 0o070; fi",
            &mut state,
        )
        .unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn if_elif_before_semicolon_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(
            b"if umask 0o077; then umask 0o000; elif umask 0o007 then umask 0o070; fi",
            &mut state,
        )
        .unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn if_elif_without_then_returns_err() {
    child_test(|| {
        let mut state = ShellState::new();
        let e = run_one(
            b"if umask 0o077; then umask 0o000; elif umask 0o007; else umask 0o070; fi",
            &mut state,
        )
        .unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn if_then_newline_separator() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"if true\nthen\numask 0o000\nfi", &mut state).unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn nested_if_fails() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(
            b"if true; then if false; then umask 0o000; fi; fi",
            &mut state,
        )
        .unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn nested_if_newline_fails() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(
            b"if true\nthen\nif false\nthen\numask 0o000\nfi\nfi",
            &mut state,
        )
        .unwrap();
        assert!(matches!(state.last_status, WaitStatus::Exited(0)));
        assert_ne!(sys::umask::get(), 0o000);
    });
}

#[test]
fn string_assign_stores_in_state() {
    let mut state = ShellState::new();
    run_one(b"var=\"hello world\"", &mut state).unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    let val = state.strings.get(&c"var".into());
    assert_eq!(val, Some(&c"hello world".into()));
}

#[test]
fn string_assign_empty_value() {
    let mut state = ShellState::new();
    run_one(b"var=", &mut state).unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    let val = state.strings.get(&c"var".into());
    assert_eq!(val, Some(&c"".into()));
}

#[test]
fn for_single_word_executes_body() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"for x in hello; do var=set; done", &mut state).unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"hello".into()));
    assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
}

#[test]
fn for_multiple_words_sets_var_to_last() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"for x in a b c; do var=set; done", &mut state).unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"c".into()));
    assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
}

#[test]
fn for_empty_words_skips_body() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"for x in; do var=set; done", &mut state).unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), None);
    assert_eq!(state.strings.get(&c"var".into()), None);
}

#[test]
fn for_newline_body() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"for x in hello\ndo\nvar=set\ndone", &mut state).unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"hello".into()));
    assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
}

#[test]
fn for_backtick_expands_to_words() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"for x in `echo 42 7`; do var=set; done", &mut state).unwrap();
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"7".into()));
        assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
    });
}

#[test]
fn for_backtick_empty_output_skips_body() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"for x in `echo`; do var=set; done", &mut state).unwrap();
        assert_eq!(state.strings.get(&c"x".into()), None);
        assert_eq!(state.strings.get(&c"var".into()), None);
    });
}

#[test]
fn for_dollar_paren_expands_to_words() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(
            b"for x in $(echo hello world); do var=set; done",
            &mut state,
        )
        .unwrap();
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"world".into()));
        assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
    });
}

#[test]
fn for_backtick_single_number() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"for x in `echo 99`; do var=set; done", &mut state).unwrap();
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"99".into()));
        assert_eq!(state.strings.get(&c"var".into()), Some(&c"set".into()));
    });
}

#[test]
fn cmd_subst_in_assign() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"x=$(builtin echo hello)", &mut state).unwrap();
        assert_eq!(state.strings.get(&c"x".into()), Some(&c"hello".into()));
    });
}

#[test]
fn cmd_subst_in_assign_and_use() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"x=$(builtin echo world); builtin echo $x", &mut state).unwrap();
    });
}

#[test]
fn string_assign_dollar_var() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"a=hello; b=$a", &mut state).unwrap();
    assert_eq!(state.strings.get(&c"b".into()), Some(&c"hello".into()));
}

#[test]
fn string_assign_multiple_vars() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"a=foo; b=bar; c=$a$b", &mut state).unwrap();
    assert_eq!(state.strings.get(&c"c".into()), Some(&c"foobar".into()));
}

#[test]
fn string_assign_dollar_brace() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"a=hello; b=${a}", &mut state).unwrap();
    assert_eq!(state.strings.get(&c"b".into()), Some(&c"hello".into()));
}

#[test]
fn string_assign_unknown_var_preserves_literal() {
    let mut state = ShellState::new();
    crate::repl::run_script(b"x=$nonexistent", &mut state).unwrap();
    assert_eq!(
        state.strings.get(&c"x".into()),
        Some(&c"$nonexistent".into())
    );
}

#[test]
fn cmd_subst_in_regular_args() {
    child_test(|| {
        let mut state = ShellState::new();
        crate::repl::run_script(b"builtin echo $(builtin echo hello)", &mut state).unwrap();
    });
}

#[test]
fn dollar_question_exit_status() {
    let mut state = ShellState::new();
    // After a successful command, $? should be 0
    crate::repl::run_script(b"builtin echo ok; x=$?", &mut state).unwrap();
    assert_eq!(state.strings.get(&c"x".into()), Some(&c"0".into()));
}

#[test]
fn dollar_question_after_failure() {
    let mut state = ShellState::new();
    // An unknown command sets non-zero exit; $? captures it
    crate::repl::run_script(b"nonexistent_cmd_xyzzy; x=$?", &mut state).unwrap();
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
        let mut state = ShellState::new();
        crate::repl::run_script(b"x=prefix$(builtin echo middle)suffix", &mut state).unwrap();
        assert_eq!(
            state.strings.get(&c"x".into()),
            Some(&c"prefixmiddlesuffix".into())
        );
    });
}
