#![allow(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::indexing_slicing))]

use super::*;
use crate::capture::Capture;
use crate::redirect::RedirectDef;
use sys::errno::EINVAL;

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
            command: c"mkdirat".into(),
            args: vec![
                c"--mode".into(),
                c"755".into(),
                c"--dirfd".into(),
                c"%CWD".into(),
                c"foo".into(),
            ],
            captures: vec![Capture {
                var: c"foo".into(),
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
    assert_eq!(cmd.command, c"openat2".into());
    assert_eq!(
        cmd.args,
        vec![
            c"--dirfd".into(),
            c"%foo".into(),
            c"--flags".into(),
            c"O_CREAT".into(),
            c"--flags".into(),
            c"O_EXCL".into(),
            c"--mode".into(),
            c"0644".into(),
            c"baz".into(),
        ]
    );
    assert_eq!(
        cmd.captures,
        vec![Capture {
            var: c"baz".into(),
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
    assert_eq!(cmd.command, c"echo".into());
    assert_eq!(cmd.args, vec![c"test".into()]);
    assert!(cmd.captures.is_empty());
    assert_eq!(cmd.redirects, vec![RedirectDef::var(1, c"baz")]);
    assert!(!cmd.background);
}

#[test]
fn test_pipe_tagged_captures() {
    let ParsedLine::Cmd(cmd) = parse("builtin pipe %rd>%server %wr>%client").unwrap() else {
        panic!("expected Cmd")
    };

    assert!(cmd.builtin);
    assert_eq!(cmd.command, c"pipe".into());
    assert!(cmd.args.is_empty());
    assert_eq!(
        cmd.captures,
        vec![
            Capture {
                var: c"server".into(),
                tag: Some(c"rd".into()),
                force: false,
            },
            Capture {
                var: c"client".into(),
                tag: Some(c"wr".into()),
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
    assert_eq!(cmd.command, c"run_server".into());
    assert_eq!(cmd.args, vec![c"params".into()]);
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
            var: c"foo".into(),
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

    assert_eq!(cmd.redirects, vec![RedirectDef::var(2, c"log")]);
}

#[test]
fn test_stdin_redirect() {
    let ParsedLine::Cmd(cmd) = parse("cat <%input").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::var(0, c"input")]);
}

#[test]
fn test_path_redirect() {
    let ParsedLine::Cmd(cmd) = parse("echo test >out.txt").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::write_path(1, c"out.txt")]);
}

#[test]
fn test_path_redirect_stdin() {
    let ParsedLine::Cmd(cmd) = parse("cat <input.txt").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::read_path(0, c"input.txt")]);
}

#[test]
fn test_stderr_path_redirect() {
    let ParsedLine::Cmd(cmd) = parse("cmd 2>err.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::write_path(2, c"err.log")]);
}

#[test]
fn test_path_redirect_append() {
    let ParsedLine::Cmd(cmd) = parse("echo test >>out.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::append_path(1, c"out.log")]);
}

#[test]
fn test_path_redirect_append_named_fd() {
    let ParsedLine::Cmd(cmd) = parse("cmd 2>>err.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::append_path(2, c"err.log")]);
}

#[test]
fn test_append_followed_by_percent_is_error() {
    assert!(matches!(parse("echo >>%var"), Err(EINVAL)));
    assert!(matches!(parse("echo 2>>%var"), Err(EINVAL)));
}

#[test]
fn test_renameat2() {
    let ParsedLine::Cmd(cmd) =
        parse("builtin renameat2 --olddirfd %foo --newdirfd %bar baz qux").unwrap()
    else {
        panic!("expected Cmd")
    };

    assert!(cmd.builtin);
    assert_eq!(cmd.command, c"renameat2".into());
    assert_eq!(
        cmd.args,
        vec![
            c"--olddirfd".into(),
            c"%foo".into(),
            c"--newdirfd".into(),
            c"%bar".into(),
            c"baz".into(),
            c"qux".into(),
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

    assert_eq!(var, c"server_pid".into());
    assert_eq!(value, c"!".into());
}

#[test]
fn test_unset() {
    let ParsedLine::Unset(var) = parse("unset %client").unwrap() else {
        panic!("expected Unset")
    };

    assert_eq!(var, c"client".into());
}

#[test]
fn test_unset_missing_is_ok() {
    let ParsedLine::Unset(var) = parse("unset %nonexistent").unwrap() else {
        panic!("expected Unset")
    };

    assert_eq!(var, c"nonexistent".into());
}

#[test]
fn test_execveat2_builtin() {
    let ParsedLine::Cmd(cmd) = parse("builtin execveat2 %MYBIN myprog arg1").unwrap() else {
        panic!("expected Cmd")
    };

    assert!(cmd.builtin);
    assert_eq!(cmd.command, c"execveat2".into());
    assert_eq!(
        cmd.args,
        vec![c"%MYBIN".into(), c"myprog".into(), c"arg1".into(),]
    );
}

#[test]
fn test_pipeline_two_commands() {
    let ParsedLine::Pipeline(pipe) = parse("echo hello | wc -l").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.commands.len(), 2);
    assert_eq!(pipe.commands[0].command, c"echo".into());
    assert_eq!(pipe.commands[0].args, vec![c"hello".into()]);
    assert_eq!(pipe.commands[1].command, c"wc".into());
    assert_eq!(pipe.commands[1].args, vec![c"-l".into()]);
}

#[test]
fn test_pipeline_three_commands() {
    let ParsedLine::Pipeline(pipe) = parse("a | b | c").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.commands.len(), 3);
    assert_eq!(pipe.commands[0].command, c"a".into());
    assert_eq!(pipe.commands[1].command, c"b".into());
    assert_eq!(pipe.commands[2].command, c"c".into());
}

#[test]
fn test_pipeline_with_captures() {
    let ParsedLine::Pipeline(pipe) = parse("cmd1 %>%a | cmd2 %>%b").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.commands.len(), 2);
    assert_eq!(pipe.commands[0].captures.len(), 1);
    assert_eq!(pipe.commands[1].captures.len(), 1);
}

#[test]
fn test_pipeline_with_redirect() {
    let ParsedLine::Pipeline(pipe) = parse("cmd1 2>%log | cmd2").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.commands[0].redirects.len(), 1);
}

#[test]
fn test_pipeline_empty_segment() {
    assert!(matches!(parse("cmd1 |"), Err(EINVAL)));
    assert!(matches!(parse("| cmd2"), Err(EINVAL)));
    assert!(matches!(parse("cmd1 || cmd2"), Err(EINVAL)));
}

#[test]
fn test_pipe_builtin_not_pipeline() {
    assert!(matches!(
        parse("builtin pipe %rd>%a %wr>%b"),
        Ok(ParsedLine::Cmd(_))
    ));
}
