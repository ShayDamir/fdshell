#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]

use super::*;
use crate::capture::Capture;
use crate::redirect::RedirectDef;
use sys::errno::EINVAL;

#[test]
fn test_mkdirat_capture() {
    let ParsedLine::Cmd(cmd) =
        parse(b"builtin mkdirat --mode 755 --dirfd %CWD foo %>%foo").unwrap()
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
            pidvar: None,
            bg_force: false,
        }
    );
}

#[test]
fn test_openat2_capture() {
    let ParsedLine::Cmd(cmd) = parse(
        b"builtin openat2 --dirfd %foo --flags O_CREAT --flags O_EXCL --mode 0644 baz %>%baz",
    )
    .unwrap() else {
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
    assert!(cmd.pidvar.is_none());
}

#[test]
fn test_pipe_tagged_captures() {
    let ParsedLine::Cmd(cmd) = parse(b"builtin pipe %rd>%server %wr>%client").unwrap() else {
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
    assert!(cmd.pidvar.is_none());
}

#[test]
fn test_background() {
    let ParsedLine::Cmd(cmd) = parse(b"run_server params &>&myserver").unwrap() else {
        panic!("expected Cmd")
    };

    assert!(!cmd.builtin);
    assert_eq!(cmd.command, c"run_server".into());
    assert_eq!(cmd.args, vec![c"params".into()]);
    assert!(cmd.captures.is_empty());
    assert!(cmd.redirects.is_empty());
    assert_eq!(cmd.pidvar, Some(c"myserver".into()));
    assert!(!cmd.bg_force);
}

#[test]
fn test_background_force() {
    let ParsedLine::Cmd(cmd) = parse(b"run_server &>|&myserver").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.pidvar, Some(c"myserver".into()));
    assert!(cmd.bg_force);
}

#[test]
fn test_bare_background_is_err() {
    assert!(parse(b"cmd &").is_err());
}

#[test]
fn test_force_capture() {
    let ParsedLine::Cmd(cmd) = parse(b"builtin mkdirat foo %>|%foo").unwrap() else {
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
    let ParsedLine::Cmd(cmd) = parse(b"echo err 2>%log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::var(2, c"log")]);
}

#[test]
fn test_stdin_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <%input").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::var(0, c"input")]);
}

#[test]
fn test_path_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"echo test >out.txt").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::write_path(1, c"out.txt")]);
}

#[test]
fn test_path_redirect_stdin() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <input.txt").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::read_path(0, c"input.txt")]);
}

#[test]
fn test_stderr_path_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>err.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::write_path(2, c"err.log")]);
}

#[test]
fn test_path_redirect_append() {
    let ParsedLine::Cmd(cmd) = parse(b"echo test >>out.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::append_path(1, c"out.log")]);
}

#[test]
fn test_path_redirect_append_named_fd() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>>err.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.redirects, vec![RedirectDef::append_path(2, c"err.log")]);
}

#[test]
fn test_append_followed_by_percent_is_error() {
    assert!(parse(b"echo >>%var").is_err());
    assert!(parse(b"echo 2>>%var").is_err());
}

#[test]
fn test_renameat2() {
    let ParsedLine::Cmd(cmd) =
        parse(b"builtin renameat2 --olddirfd %foo --newdirfd %bar baz qux").unwrap()
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
fn test_assign_fd() {
    let ParsedLine::AssignFd { var, value } = parse(b"%server_pid=%!").unwrap() else {
        panic!("expected AssignFd")
    };

    assert_eq!(var, c"server_pid".into());
    assert_eq!(value, c"!".into());
}

#[test]
fn test_assign_str_simple() {
    let ParsedLine::AssignStr { var, value } = parse(b"var=hello").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(var, c"var".into());
    assert_eq!(value, c"hello".into());
}

#[test]
fn test_assign_str_empty_value() {
    let ParsedLine::AssignStr { var, value } = parse(b"var=").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(var, c"var".into());
    assert_eq!(value, c"".into());
}

#[test]
fn test_assign_str_quoted_spaces() {
    let ParsedLine::AssignStr { var, value } = parse(b"var=\"foo bar\"").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(var, c"var".into());
    assert_eq!(value, c"foo bar".into());
}

#[test]
fn test_assign_str_no_lhs_is_not_assign() {
    let result = parse(b"=value").unwrap();
    assert!(!matches!(result, ParsedLine::AssignStr { .. }));
}

#[test]
fn test_assign_str_fd_assign_takes_priority() {
    let result = parse(b"%x=%y").unwrap();
    assert!(matches!(result, ParsedLine::AssignFd { .. }));
}

#[test]
fn test_assign_str_percent_value_is_literal() {
    let ParsedLine::AssignStr { var, value } = parse(b"var=%othervar").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(var, c"var".into());
    assert_eq!(value, c"%othervar".into());
}

#[test]
fn test_unset() {
    let ParsedLine::Unset(var) = parse(b"unset %client").unwrap() else {
        panic!("expected Unset")
    };

    assert_eq!(var, c"client".into());
}

#[test]
fn test_unset_missing_is_ok() {
    let ParsedLine::Unset(var) = parse(b"unset %nonexistent").unwrap() else {
        panic!("expected Unset")
    };

    assert_eq!(var, c"nonexistent".into());
}

#[test]
fn test_execveat2_builtin() {
    let ParsedLine::Cmd(cmd) = parse(b"builtin execveat2 %MYBIN myprog arg1").unwrap() else {
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
    let ParsedLine::Pipeline(pipe) = parse(b"echo hello | wc -l").unwrap() else {
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
    let ParsedLine::Pipeline(pipe) = parse(b"a | b | c").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.commands.len(), 3);
    assert_eq!(pipe.commands[0].command, c"a".into());
    assert_eq!(pipe.commands[1].command, c"b".into());
    assert_eq!(pipe.commands[2].command, c"c".into());
}

#[test]
fn test_pipeline_with_captures() {
    let ParsedLine::Pipeline(pipe) = parse(b"cmd1 %>%a | cmd2 %>%b").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.commands.len(), 2);
    assert_eq!(pipe.commands[0].captures.len(), 1);
    assert_eq!(pipe.commands[1].captures.len(), 1);
}

#[test]
fn test_pipeline_with_redirect() {
    let ParsedLine::Pipeline(pipe) = parse(b"cmd1 2>%log | cmd2").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.commands[0].redirects.len(), 1);
}

#[test]
fn test_pipeline_empty_segment() {
    assert!(parse(b"cmd1 |").is_err());
    assert!(parse(b"| cmd2").is_err());
    assert!(parse(b"cmd1 || cmd2").is_err());
}

#[test]
fn test_pipe_builtin_not_pipeline() {
    assert!(matches!(
        parse(b"builtin pipe %rd>%a %wr>%b"),
        Ok(ParsedLine::Cmd(_))
    ));
}

#[test]
fn test_umask_no_args() {
    let ParsedLine::Umask(mask) = parse(b"umask").unwrap() else {
        panic!("expected Umask")
    };
    assert_eq!(mask, None);
}

#[test]
fn test_umask_zero_o() {
    let ParsedLine::Umask(mask) = parse(b"umask 0o077").unwrap() else {
        panic!("expected Umask")
    };
    assert_eq!(mask, Some(0o077));
}

#[test]
fn test_umask_numeric() {
    let ParsedLine::Umask(mask) = parse(b"umask 077").unwrap() else {
        panic!("expected Umask")
    };
    assert_eq!(mask, Some(0o077));
}

#[test]
fn test_umask_zero() {
    let ParsedLine::Umask(mask) = parse(b"umask 0o000").unwrap() else {
        panic!("expected Umask")
    };
    assert_eq!(mask, Some(0o000));
}

#[test]
fn test_umask_max() {
    let ParsedLine::Umask(mask) = parse(b"umask 0o777").unwrap() else {
        panic!("expected Umask")
    };
    assert_eq!(mask, Some(0o777));
}

#[test]
fn test_umask_too_many_args() {
    assert!(parse(b"umask 0o077 extra").is_err());
}

#[test]
fn test_umask_invalid_digit() {
    assert!(parse(b"umask abc").is_err());
}

#[test]
fn test_umask_invalid_non_octal() {
    assert!(parse(b"umask 0o078").is_err());
}

#[test]
fn tokenize_if_newline_separators() {
    let tokens = token::tokenize(b"if true\nthen\numask 0o000\nfi").unwrap();
    assert_eq!(
        tokens,
        vec![
            c"if".into(),
            c"true".into(),
            c";".into(),
            c"then".into(),
            c";".into(),
            c"umask".into(),
            c"0o000".into(),
            c";".into(),
            c"fi".into(),
        ],
    );
}

#[test]
fn parse_if_newline_separators() {
    let ParsedLine::If(ib) = parse(b"if true\nthen\numask 0o000\nfi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition, c"true".into());
    assert_eq!(ib.then_body, c"umask 0o000".into());
    assert!(ib.elifs.is_empty());
    assert!(ib.else_body.is_none());
}

#[test]
fn for_simple() {
    let tokens = token::tokenize(b"for var in a b c; do echo $var; done").unwrap();
    let fb = for_block::tokens_to_for(&tokens).unwrap();
    assert_eq!(fb.var, c"var".into());
    assert_eq!(fb.words, vec![c"a".into(), c"b".into(), c"c".into()]);
    assert_eq!(fb.body, c"echo $var".into());
}

#[test]
fn for_single_word() {
    let tokens = token::tokenize(b"for x in foo; do cmd; done").unwrap();
    let fb = for_block::tokens_to_for(&tokens).unwrap();
    assert_eq!(fb.var, c"x".into());
    assert_eq!(fb.words, vec![c"foo".into()]);
    assert_eq!(fb.body, c"cmd".into());
}

#[test]
fn for_newline_separators() {
    let tokens = token::tokenize(b"for x in a b c\ndo\ncmd1; cmd2\ndone").unwrap();
    let fb = for_block::tokens_to_for(&tokens).unwrap();
    assert_eq!(fb.var, c"x".into());
    assert_eq!(fb.words, vec![c"a".into(), c"b".into(), c"c".into()]);
    assert_eq!(fb.body, c"cmd1 ; cmd2".into());
}

#[test]
fn for_empty_words() {
    let tokens = token::tokenize(b"for x in ; do cmd; done").unwrap();
    let fb = for_block::tokens_to_for(&tokens).unwrap();
    assert_eq!(fb.var, c"x".into());
    assert!(fb.words.is_empty());
    assert_eq!(fb.body, c"cmd".into());
}

#[test]
fn for_missing_do_returns_err() {
    let tokens = token::tokenize(b"for x in a; done").unwrap();
    assert_eq!(for_block::tokens_to_for(&tokens).unwrap_err(), EINVAL);
}

#[test]
fn for_missing_done_returns_err() {
    let tokens = token::tokenize(b"for x in a; do cmd").unwrap();
    assert_eq!(for_block::tokens_to_for(&tokens).unwrap_err(), EINVAL);
}

#[test]
fn for_no_in_returns_err() {
    let tokens = token::tokenize(b"for x a; do cmd; done").unwrap();
    assert_eq!(for_block::tokens_to_for(&tokens).unwrap_err(), EINVAL);
}

#[test]
fn for_not_starting_with_for_is_not_a_for() {
    let tokens = token::tokenize(b"while x; do cmd; done").unwrap();
    assert!(for_block::tokens_to_for(&tokens).is_err());
}

#[test]
fn for_parse_dispatch() {
    let ParsedLine::For(fb) = parse(b"for var in a b c; do echo $var; done").unwrap() else {
        panic!("expected For")
    };
    assert_eq!(fb.var, c"var".into());
    assert_eq!(fb.words, vec![c"a".into(), c"b".into(), c"c".into()]);
    assert_eq!(fb.body, c"echo $var".into());
}

#[test]
fn tokenize_backtick_command() {
    let result = token::tokenize(b"echo `seq 1 10`").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], c"echo".into());
    assert_eq!(result[1], c"`seq 1 10`".into());
}

#[test]
fn tokenize_dollar_paren_command() {
    let result = token::tokenize(b"echo $(seq 1 10)").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], c"echo".into());
    assert_eq!(result[1], c"$(seq 1 10)".into());
}

#[test]
fn tokenize_backtick_empty() {
    let result = token::tokenize(b"for x in ``; do body; done").unwrap();
    assert_eq!(result[3], c"``".into());
}

#[test]
fn tokenize_dollar_paren_nested() {
    let result = token::tokenize(b"$(echo $(seq 3))").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], c"$(echo $(seq 3))".into());
}

#[test]
fn for_backtick_word_is_single_token() {
    let ParsedLine::For(fb) = parse(b"for x in `echo 1 2 3`; do body; done").unwrap() else {
        panic!("expected For")
    };
    assert_eq!(fb.words.len(), 1);
    assert_eq!(fb.words[0], c"`echo 1 2 3`".into());
}

#[test]
fn for_dollar_paren_word_is_single_token() {
    let ParsedLine::For(fb) = parse(b"for x in $(echo hello); do body; done").unwrap() else {
        panic!("expected For")
    };
    assert_eq!(fb.words.len(), 1);
    assert_eq!(fb.words[0], c"$(echo hello)".into());
}

#[test]
fn while_parse_dispatch() {
    let ParsedLine::While(wb) = parse(b"while true; do echo x; done").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.condition, c"true".into());
    assert_eq!(wb.body, c"echo x".into());
}

#[test]
fn while_parse_with_semicolon_body() {
    let ParsedLine::While(wb) = parse(b"while false; do echo a; echo b; done").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.condition, c"false".into());
    let body_bytes = wb.body.as_bytes().unwrap();
    // Body is all tokens between "do" and "done", joined with spaces
    assert!(body_bytes.starts_with(b"echo"));
}

#[test]
fn while_parse_pipe_in_body() {
    let ParsedLine::While(wb) = parse(b"while true; do echo hello | cat; done").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.condition, c"true".into());
    assert_eq!(wb.body, c"echo hello | cat".into());
}

#[test]
fn while_parse_newline_separator() {
    let ParsedLine::While(wb) = parse(b"while umask 0o077\ndo\numask 0o000\ndone").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.condition, c"umask 0o077".into());
    assert_eq!(wb.body, c"umask 0o000".into());
}

#[test]
fn while_not_starting_with_while_is_a_cmd() {
    let tokens = token::tokenize(b"while_true; do body; done").unwrap();
    assert!(!tokens.first().is_some_and(|t| t.eq_bytes(b"while")));
}

#[test]
fn while_without_do_returns_err() {
    let tokens = token::tokenize(b"while true; echo x; done").unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"while").is_err());
}

#[test]
fn while_missing_done_at_end_returns_err() {
    let result = parse(b"while true; do echo x");
    assert!(result.is_err());
}

#[test]
fn while_do_not_preceded_by_semi_returns_err() {
    let tokens = token::tokenize(b"while true do echo x; done").unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"while").is_err());
}

#[test]
fn until_parse_dispatch() {
    let ParsedLine::Until(wb) = parse(b"until false; do echo x; done").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition, c"false".into());
    assert_eq!(wb.body, c"echo x".into());
}

#[test]
fn until_parse_with_semicolon_body() {
    let ParsedLine::Until(wb) = parse(b"until false; do echo a; echo b; done").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition, c"false".into());
    let body_bytes = wb.body.as_bytes().unwrap();
    assert!(body_bytes.starts_with(b"echo"));
}

#[test]
fn until_parse_newline_separator() {
    let ParsedLine::Until(wb) = parse(b"until false\ndo\numask 0o000\ndone").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition, c"false".into());
    assert_eq!(wb.body, c"umask 0o000".into());
}

#[test]
fn until_not_starting_with_until_is_a_cmd() {
    let tokens = token::tokenize(b"until_true; do body; done").unwrap();
    assert!(!tokens.first().is_some_and(|t| t.eq_bytes(b"until")));
}

#[test]
fn until_without_do_returns_err() {
    let tokens = token::tokenize(b"until true; echo x; done").unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"until").is_err());
}

#[test]
fn until_missing_done_at_end_returns_err() {
    let result = parse(b"until true; do echo x");
    assert!(result.is_err());
}

#[test]
fn until_do_not_preceded_by_semi_returns_err() {
    let tokens = token::tokenize(b"until true do echo x; done").unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"until").is_err());
}

#[test]
fn until_parse_pipe_in_body() {
    let ParsedLine::Until(wb) = parse(b"until true; do echo hello | cat; done").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition, c"true".into());
    assert_eq!(wb.body, c"echo hello | cat".into());
}
