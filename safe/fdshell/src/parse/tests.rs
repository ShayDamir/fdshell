#![allow(clippy::unwrap_used)]

use super::*;
use crate::capture::Capture;
use crate::redirect::Redirect;
use std::ffi::CString;

#[test]
fn test_mkdirat_capture() {
    let ParsedLine::Cmd(cmd) = parse("builtin mkdirat --mode 755 --dirfd %CWD foo %>%foo").unwrap()
    else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd,
        CommandLine {
            builtin: true,
            command: CString::from(c"mkdirat"),
            args: vec![
                CString::from(c"--mode"),
                CString::from(c"755"),
                CString::from(c"--dirfd"),
                CString::from(c"%CWD"),
                CString::from(c"foo"),
            ],
            captures: vec![Capture {
                var: CString::from(c"foo"),
                tag: None,
                force: false,
            }],
            redirects: vec![],
            background: false,
        }
    );
}

#[test]
fn test_openat2_capture() {
    let ParsedLine::Cmd(cmd) =
        parse("builtin openat2 --dirfd %foo --flags O_CREAT --flags O_EXCL --mode 0644 baz %>%baz")
            .unwrap()
    else {
        panic!("expected Cmd")
    };

    assert!(cmd.builtin);
    assert_eq!(cmd.command, CString::from(c"openat2"));
    assert_eq!(
        cmd.args,
        vec![
            CString::from(c"--dirfd"),
            CString::from(c"%foo"),
            CString::from(c"--flags"),
            CString::from(c"O_CREAT"),
            CString::from(c"--flags"),
            CString::from(c"O_EXCL"),
            CString::from(c"--mode"),
            CString::from(c"0644"),
            CString::from(c"baz"),
        ]
    );
    assert_eq!(
        cmd.captures,
        vec![Capture {
            var: CString::from(c"baz"),
            tag: None,
            force: false,
        }]
    );
    assert!(cmd.redirects.is_empty());
    assert!(!cmd.background);
}

#[test]
fn test_echo_redirect() {
    let ParsedLine::Cmd(cmd) = parse("echo \"test\" >%baz").unwrap() else {
        panic!("expected Cmd")
    };

    assert!(!cmd.builtin);
    assert_eq!(cmd.command, CString::from(c"echo"));
    assert_eq!(cmd.args, vec![CString::from(c"test")]);
    assert!(cmd.captures.is_empty());
    assert_eq!(
        cmd.redirects,
        vec![Redirect {
            target_fd: 1,
            src_var: CString::from(c"baz"),
        }]
    );
    assert!(!cmd.background);
}

#[test]
fn test_pipe_tagged_captures() {
    let ParsedLine::Cmd(cmd) = parse("builtin pipe %rd>%server %wr>%client").unwrap() else {
        panic!("expected Cmd")
    };

    assert!(cmd.builtin);
    assert_eq!(cmd.command, CString::from(c"pipe"));
    assert!(cmd.args.is_empty());
    assert_eq!(
        cmd.captures,
        vec![
            Capture {
                var: CString::from(c"server"),
                tag: Some(CString::from(c"rd")),
                force: false,
            },
            Capture {
                var: CString::from(c"client"),
                tag: Some(CString::from(c"wr")),
                force: false,
            },
        ]
    );
    assert!(cmd.redirects.is_empty());
    assert!(!cmd.background);
}

#[test]
fn test_background() {
    let ParsedLine::Cmd(cmd) = parse("run_server params &").unwrap() else {
        panic!("expected Cmd")
    };

    assert!(!cmd.builtin);
    assert_eq!(cmd.command, CString::from(c"run_server"));
    assert_eq!(cmd.args, vec![CString::from(c"params")]);
    assert!(cmd.captures.is_empty());
    assert!(cmd.redirects.is_empty());
    assert!(cmd.background);
}

#[test]
fn test_force_capture() {
    let ParsedLine::Cmd(cmd) = parse("builtin mkdirat foo %>|%foo").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.captures,
        vec![Capture {
            var: CString::from(c"foo"),
            tag: None,
            force: true,
        }]
    );
}

#[test]
fn test_stderr_redirect() {
    let ParsedLine::Cmd(cmd) = parse("echo err 2>%log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![Redirect {
            target_fd: 2,
            src_var: CString::from(c"log"),
        }]
    );
}

#[test]
fn test_stdin_redirect() {
    let ParsedLine::Cmd(cmd) = parse("cat <%input").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![Redirect {
            target_fd: 0,
            src_var: CString::from(c"input"),
        }]
    );
}

#[test]
fn test_renameat2() {
    let ParsedLine::Cmd(cmd) =
        parse("builtin renameat2 --olddirfd %foo --newdirfd %bar baz qux").unwrap()
    else {
        panic!("expected Cmd")
    };

    assert!(cmd.builtin);
    assert_eq!(cmd.command, CString::from(c"renameat2"));
    assert_eq!(
        cmd.args,
        vec![
            CString::from(c"--olddirfd"),
            CString::from(c"%foo"),
            CString::from(c"--newdirfd"),
            CString::from(c"%bar"),
            CString::from(c"baz"),
            CString::from(c"qux"),
        ]
    );
    assert!(cmd.captures.is_empty());
    assert!(cmd.redirects.is_empty());
}

#[test]
fn test_assign() {
    let ParsedLine::Assign { var, value } = parse("%server_pid=%!").unwrap() else {
        panic!("expected Assign")
    };

    assert_eq!(var, CString::from(c"server_pid"));
    assert_eq!(value, CString::from(c"!"));
}

#[test]
fn test_unset() {
    let ParsedLine::Unset(var) = parse("unset %client").unwrap() else {
        panic!("expected Unset")
    };

    assert_eq!(var, CString::from(c"client"));
}

#[test]
fn test_unset_missing_is_ok() {
    let ParsedLine::Unset(var) = parse("unset %nonexistent").unwrap() else {
        panic!("expected Unset")
    };

    assert_eq!(var, CString::from(c"nonexistent"));
}
