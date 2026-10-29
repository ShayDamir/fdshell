#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::capture::Capture;
use crate::error::cmd::CmdError;
use crate::error::parse::ParseError;
use crate::mask::MaskBit::Unquoted;
use crate::mask::tests::runs;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::{Origin, Position, ScriptText};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// A scoped-assignment `Assign` for a value word written fully quoted
/// (`quoted = true`) or fully unquoted (`false`) — the mask the parser must
/// hand on: the value word's own mask, one bit per value byte.
fn assign(name: &str, value: &str, quoted: bool) -> Assign {
    Assign {
        name: sys::ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap(),
        value: sys::ShortCStr::from_vec(value.as_bytes().to_vec()).unwrap(),
        mask: vec![MaskBit::from_quoted(quoted); value.len()],
        spans: Vec::new(),
    }
}

/// A raw token word at byte offset `start` with an all-unquoted mask of its own
/// length — the shape `envassign::prefix_len` inspects (word bytes + the `;`
/// guard). Its range is nominal; `prefix_len` never reads the range.
fn word_token(word: &[u8], start: usize) -> Token {
    (
        sys::ShortCStr::from_vec(word.to_vec()).unwrap(),
        start,
        start + word.len(),
        false,
        vec![Unquoted; word.len()],
        Vec::new(),
    )
}

fn pos() -> Position {
    Position::new(1, 1)
}

/// Wraps a raw line in a nominal `ScriptText` so call sites keep passing `&[u8]`.
fn parse(line: &[u8]) -> Result<ParsedLine, Report<ParseError>> {
    super::parse(&stext(line))
}

/// A `Path` redirect def with the unquoted quote mask (all `false`), matching
/// what the parser produces for an unquoted path word.
fn path_def(export_to: i32, direction: RedirectDirection, path: &str) -> RedirectDef {
    RedirectDef {
        export_to,
        extra_to: None,
        direction,
        source: RedirectSource::path(
            sys::ShortCStr::from_vec(path.as_bytes().to_vec()).unwrap(),
            vec![Unquoted; path.len()],
        ),
    }
}

/// The `&>` family's def shape: one open (`export_to` = fd 1, `extra_to` = fd 2),
/// unquoted quote mask, as the parser produces it for an unquoted path word.
fn combined_def(direction: RedirectDirection, path: &str) -> RedirectDef {
    RedirectDef {
        export_to: 1,
        extra_to: Some(2),
        direction,
        source: RedirectSource::path(
            sys::ShortCStr::from_vec(path.as_bytes().to_vec()).unwrap(),
            vec![Unquoted; path.len()],
        ),
    }
}
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
            prefix: BuiltinPrefix::Builtin,
            command: c"mkdirat".into(),
            command_mask: vec![Unquoted; 7],
            command_span: vec![],
            command_quoted: false,
            env_assigns: vec![],
            args: vec![
                c"--mode".into(),
                c"755".into(),
                c"--dirfd".into(),
                c"%CWD".into(),
                c"foo".into(),
            ],
            args_mask: vec![
                vec![Unquoted; 6],
                vec![Unquoted; 3],
                vec![Unquoted; 7],
                vec![Unquoted; 4],
                vec![Unquoted; 3],
            ],
            args_span: vec![vec![]; 5],
            args_quoted: vec![false; 5],
            captures: vec![Capture {
                var: c"foo".into(),
                tag: None,
                force: false,
                cap: None,
                set_at: pos(),
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

    assert_eq!(cmd.prefix, BuiltinPrefix::Builtin);
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
            cap: None,
            set_at: pos(),
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

    assert_eq!(cmd.prefix, BuiltinPrefix::Builtin);
    assert_eq!(cmd.command, c"pipe".into());
    assert!(cmd.args.is_empty());
    assert_eq!(
        cmd.captures,
        vec![
            Capture {
                var: c"server".into(),
                tag: Some(c"rd".into()),
                force: false,
                cap: None,
                set_at: pos(),
            },
            Capture {
                var: c"client".into(),
                tag: Some(c"wr".into()),
                force: false,
                cap: None,
                set_at: pos(),
            },
        ]
    );
    assert!(cmd.redirects.is_empty());
    assert!(cmd.pidvar.is_none());
}

#[test]
fn test_command_keyword_is_command_prefix() {
    let ParsedLine::Cmd(cmd) = parse(b"command openat2 --dirfd %foo %>%baz").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.prefix, BuiltinPrefix::Command);
    assert_eq!(cmd.command, c"openat2".into());
    let ParsedLine::Cmd(plain) = parse(b"openat2 --dirfd %foo %>%baz").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(plain.prefix, BuiltinPrefix::None);
}

#[test]
fn test_background() {
    let ParsedLine::Cmd(cmd) = parse(b"run_server params &>&myserver").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.prefix, BuiltinPrefix::None);
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
            cap: None,
            set_at: pos(),
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
    // A var source resolves by cloning the table fd, so its `direction` is the
    // operator's (`<` reads); it is inert at resolve time.
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 0,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Var(c"input".into()),
        }]
    );
}

#[test]
fn test_path_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"echo test >out.txt").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![path_def(1, RedirectDirection::Write, "out.txt")]
    );
}

#[test]
fn test_path_redirect_stdin() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <input.txt").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![path_def(0, RedirectDirection::Read, "input.txt")]
    );
}

#[test]
fn test_stderr_path_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>err.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![path_def(2, RedirectDirection::Write, "err.log")]
    );
}

#[test]
fn test_path_redirect_append() {
    let ParsedLine::Cmd(cmd) = parse(b"echo test >>out.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![path_def(1, RedirectDirection::Append, "out.log")]
    );
}

#[test]
fn test_path_redirect_append_named_fd() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>>err.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![path_def(2, RedirectDirection::Append, "err.log")]
    );
}

#[test]
fn test_here_string_prefix_form() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<<hello").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            0,
            c"hello",
            vec![Unquoted; 5],
            Vec::new()
        )]
    );
    assert!(cmd.args.is_empty());
}

#[test]
fn test_here_string_keeps_unquoted_spaces() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<<\"a b\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            0,
            c"a b",
            runs(&[(true, 3)]),
            Vec::new()
        )]
    );
}

#[test]
fn parse_here_string_word_carries_its_quote_mask() {
    // The attached form slices the operator token's mask at `<<<` (offset 3).
    let ParsedLine::Cmd(cmd) = parse(b"cat <<<\"a\\ b\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            0,
            c"a\\ b",
            runs(&[(true, 4)]),
            Vec::new()
        )]
    );
    // An unquoted escape pair stays in the *token text* with both bytes
    // unmasked (`\ ` never breaks the token), so the attached word is the
    // unmasked `a\ b` (4 bytes: `a`, `\`, ` `, `b`) and keeps folding to `a b`,
    // as bash does.
    let ParsedLine::Cmd(cmd) = parse(b"cat <<<a\\ b").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            0,
            c"a\\ b",
            vec![Unquoted; 4],
            Vec::new()
        )]
    );
    // A bare `<<<` takes the next token's mask verbatim.
    let ParsedLine::Cmd(cmd) = parse(b"cat <<< \"a\\ b\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            0,
            c"a\\ b",
            runs(&[(true, 4)]),
            Vec::new()
        )]
    );
}

#[test]
fn parse_here_string_mask_of_a_quoted_word_with_a_second_word() {
    // The anti-off-by-one pair (task #217's rule, here on the here-string
    // surface): the two words have **different** masks, so a slice shifted by
    // one byte misreads one of them and the pair cannot agree.
    let ParsedLine::Cmd(cmd) = parse(b"cat <<<\"a\\ b\" b\\ c").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            0,
            c"a\\ b",
            runs(&[(true, 4)]),
            Vec::new()
        )]
    );
    assert_eq!(cmd.args, vec![c"b\\ c".into()]);
    assert_eq!(cmd.args_mask, vec![vec![Unquoted; 4]]);
}

/// The io-number form: the descriptor comes from the raw-byte prefix and the
/// word is sliced past `prefix + 3` (task #312).
#[test]
fn parse_here_string_io_number_prefix_targets_the_descriptor() {
    let ParsedLine::Cmd(cmd) = parse(b"cat 3<<<hello").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            3,
            c"hello",
            vec![Unquoted; 5],
            Vec::new()
        )]
    );
    assert!(cmd.args.is_empty());
    // The unprefixed form keeps fd 0 (`<<<word` is the stdin here-string).
    let ParsedLine::Cmd(cmd) = parse(b"cat 003<<<hello").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            3,
            c"hello",
            vec![Unquoted; 5],
            Vec::new()
        )]
    );
}

/// The mask shift is `prefix + 3`, and only `word_mask` owns it (LESSONS #269):
/// the pair below has **differing** masks, so an off-by-one and an off-by-two
/// cannot both pass it.
#[test]
fn parse_here_string_prefixed_word_mask_is_shifted_by_the_prefix() {
    let ParsedLine::Cmd(cmd) = parse(b"cat 3<<<\"a\\ b\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            3,
            c"a\\ b",
            runs(&[(true, 4)]),
            Vec::new()
        )]
    );
    let ParsedLine::Cmd(cmd) = parse(b"cat 3<<<\"a\\ b\" b\\ c").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            3,
            c"a\\ b",
            runs(&[(true, 4)]),
            Vec::new()
        )]
    );
    assert_eq!(cmd.args, vec![c"b\\ c".into()]);
    assert_eq!(cmd.args_mask, vec![vec![Unquoted; 4]]);
}

#[test]
fn parse_here_string_prefixed_bare_form_takes_the_next_word() {
    // `3<<< word`: the operator token is `3<<<` (4 bytes), which the unshifted
    // attached test would read as attached on `<`, so the shift is what makes
    // this the bare form.
    let ParsedLine::Cmd(cmd) = parse(b"cat 3<<< word").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            3,
            c"word",
            vec![Unquoted; 4],
            Vec::new()
        )]
    );
    assert!(cmd.args.is_empty());
    // `3<<<<<x` is the pinned `cat <<<<<x` shape fd-ified: word `<<x` on fd 3.
    let ParsedLine::Cmd(cmd) = parse(b"cat 3<<<<<x").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            3,
            c"<<x",
            vec![Unquoted; 3],
            Vec::new()
        )]
    );
}

#[test]
fn test_heredoc_basic() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<EOF\nbody\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.command, c"cat".into());
    assert!(cmd.args.is_empty());
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_doc(0, c"body\n", true)]
    );
}

#[test]
fn test_heredoc_quoted_delimiter_is_literal() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<\"Q\"\n$x\nQ").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_doc(0, c"$x\n", false)]
    );
}

#[test]
fn test_heredoc_dash_form_strips_leading_tabs() {
    // A mixed-indent line is the sharpest pin: leading tabs are dropped, the
    // spaces that follow them survive.
    let ParsedLine::Cmd(cmd) = parse(b"cat <<-EOF\n\tone\n\t\t two\n\tEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_doc(0, c"one\n two\n", true)]
    );
}

#[test]
fn test_heredoc_empty_body() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<EOF\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.redirects, vec![RedirectDef::here_doc(0, c"", true)]);
}

#[test]
fn test_heredoc_args_and_redirect_after_operator() {
    let ParsedLine::Cmd(cmd) = parse(b"echo a <<EOF b >out\nbody\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"a".into(), c"b".into()]);
    assert_eq!(
        cmd.redirects,
        vec![
            RedirectDef::here_doc(0, c"body\n", true),
            path_def(1, RedirectDirection::Write, "out"),
        ]
    );
}

// A heredoc whose delimiter line never arrives is the end-of-file shape (task
// #364): the parse layer accepts it — an empty body when nothing is left, the
// remaining lines as the body when they are — and warns (the warning itself is
// asserted in the integration tests). `InvalidRedirect` variants stay pinned in
// parse/heredoc/tests.rs, where the layout result is test-Debug.
#[test]
fn test_heredoc_at_end_of_file_is_accepted() {
    assert!(parse(b"cat <<EOF").is_ok());
    assert!(parse(b"cat <<NOPE\nbody\nnothing").is_ok());
}

#[test]
fn test_heredoc_bare_operator_is_invalid_redirect() {
    assert!(parse(b"cat <<").is_err());
    assert!(parse(b"cat << ;").is_err());
}

// POSIX #2.5: both bodies are kept, in operator order, and the apply loop makes
// the last one win. The two bodies must be *distinct*: a spec walk that reads
// `specs[0]` for every operator yields ["x\n", "x\n"] and fails here.
#[test]
fn test_heredoc_twice_keeps_both_bodies_in_order() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<A <<B\nx\nA\ny\nB").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            RedirectDef::here_doc(0, c"x\n", true),
            RedirectDef::here_doc(0, c"y\n", true),
        ]
    );
}

#[test]
fn test_heredoc_then_read_redirect_keeps_both() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<EOF <f1\nbody\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            RedirectDef::here_doc(0, c"body\n", true),
            path_def(0, RedirectDirection::Read, "f1"),
        ]
    );
}

#[test]
fn test_heredoc_pipeline_stage_trimmed() {
    let ParsedLine::Pipeline(pl) = parse(b"cat <<EOF | wc -l\nbody\nEOF").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pl.stages.len(), 2);
    // Stage 2 must not see the body tokens.
    assert_eq!(pl.command_at(1).unwrap().command, c"wc".into());
    assert_eq!(pl.command_at(1).unwrap().args, vec![c"-l".into()]);
    assert_eq!(
        pl.command_at(0).unwrap().redirects,
        vec![RedirectDef::here_doc(0, c"body\n", true)]
    );
    assert!(pl.command_at(1).unwrap().redirects.is_empty());
}

/// The per-stage spec window advances by each stage's **count**, so a stage
/// with two operators shifts the next stage's window past both bodies: stage 2
/// gets `ccc\n`, its own body, never `aaaa\n` (a `*spec_at * count` reset
/// hands it the first body and `wc -c` prints 5, measured).
#[test]
fn test_heredoc_pipeline_spec_window_advances_by_operator_count() {
    let ParsedLine::Pipeline(pl) =
        parse(b"cat <<A <<B | wc -c <<C\naaaa\nA\nbb\nB\nccc\nC").unwrap()
    else {
        panic!("expected Pipeline")
    };
    assert_eq!(pl.stages.len(), 2);
    assert_eq!(
        pl.command_at(0).unwrap().redirects,
        vec![
            RedirectDef::here_doc(0, c"aaaa\n", true),
            RedirectDef::here_doc(0, c"bb\n", true),
        ]
    );
    assert_eq!(
        pl.command_at(1).unwrap().redirects,
        vec![RedirectDef::here_doc(0, c"ccc\n", true)]
    );
}

#[test]
fn test_here_string_bare_takes_next_token() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<< word").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(
            0,
            c"word",
            vec![Unquoted; 4],
            Vec::new()
        )]
    );
    assert!(cmd.args.is_empty());
}

#[test]
fn test_here_string_bare_at_eof_is_empty_word() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<<").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(0, c"", Vec::new(), Vec::new())]
    );
}

#[test]
fn test_here_string_bare_before_redirect_is_error() {
    assert!(parse(b"cat <<< >file").is_err());
    assert!(parse(b"cat <<< %var").is_err());
}

#[test]
fn test_fd_dup_redirect_stderr_to_stdout() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hello 2>&1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 2,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Dup(1),
        }]
    );
    assert_eq!(cmd.args, vec![c"hello".into()]);
}

#[test]
fn test_fd_dup_redirect_stdin_from_stdout() {
    let ParsedLine::Cmd(cmd) = parse(b"cat 0<&1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 0,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Dup(1),
        }]
    );
}

#[test]
fn test_fd_dup_redirect_bare_prefix() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd >&1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 1,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Dup(1),
        }]
    );
}

#[test]
fn test_fd_close_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 3>&-").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 3,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Close,
        }]
    );
}

#[test]
fn test_fd_dup_var_redirect_stderr() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>&%name").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 2,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Var(c"name".into()),
        }]
    );
}

#[test]
fn test_fd_dup_var_redirect_stdin() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 0<&%name").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 0,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Var(c"name".into()),
        }]
    );
}

#[test]
fn test_fd_dup_var_redirect_bare_prefix() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd >&%name").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 1,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Var(c"name".into()),
        }]
    );
}

#[test]
fn test_fd_dup_redirect_non_numeric_is_arg() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>&abc").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.redirects.is_empty());
    assert_eq!(cmd.args, vec![c"2>&abc".into()]);
}

#[test]
fn test_fd_dup_redirect_non_numeric_prefix_is_arg() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd a>&1").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.redirects.is_empty());
    assert_eq!(cmd.args, vec![c"a>&1".into()]);
}

/// A `/dev/fd/N` target is a **path** def, in the operator's own direction: bash
/// opens the magic path with the operator's flags as the redirection runs, so the
/// direction is not inert and the def is not a dup (the resolver's magic arm,
/// `redirect/resolve/fd_path.rs`; LESSONS).
#[test]
fn test_fd_path_redirect_is_a_path_def() {
    let ParsedLine::Cmd(cmd) = parse(b"cat </dev/fd/3").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(0, RedirectDirection::Read, "/dev/fd/3")]
    );
}

/// The numbered operator keeps its own direction on the magic path: `5>` writes
/// and truncates the file fd 2 points at (bash's `open` flags).
#[test]
fn test_fd_path_redirect_numbered_write() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 5>/dev/fd/2").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(5, RedirectDirection::Write, "/dev/fd/2")]
    );
}

/// `/proc/self/fd/N` is the other prefix Linux installs, and the same rule.
#[test]
fn test_proc_self_fd_redirect_is_a_path_def() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd </proc/self/fd/7").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(0, RedirectDirection::Read, "/proc/self/fd/7")]
    );
}

#[test]
fn test_fd_path_redirect_like_is_path() {
    let ParsedLine::Cmd(cmd) = parse(b"cat </dev/fd/3x").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(0, RedirectDirection::Read, "/dev/fd/3x")]
    );
}

#[test]
fn test_fd_path_negative_falls_through_to_path() {
    let ParsedLine::Cmd(cmd) = parse(b"cat </dev/fd/-1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(0, RedirectDirection::Read, "/dev/fd/-1")]
    );
}

#[test]
fn test_fd_path_argument_is_not_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cat /dev/fd/3").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.redirects.is_empty());
    assert_eq!(cmd.args, vec![c"/dev/fd/3".into()]);
}

#[test]
fn test_rw_redirect_defaults_to_fd_zero() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <>file").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 0,
            extra_to: None,
            direction: RedirectDirection::Rw,
            source: RedirectSource::path(c"file", vec![Unquoted; 4]),
        }]
    );
}

#[test]
fn test_rw_redirect_numbered() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 3<>file").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 3,
            extra_to: None,
            direction: RedirectDirection::Rw,
            source: RedirectSource::path(c"file", vec![Unquoted; 4]),
        }]
    );
}

#[test]
fn test_rw_redirect_without_path_is_error() {
    assert!(parse(b"cmd <>").is_err());
    assert!(parse(b"cmd 2<>").is_err());
}

#[test]
fn test_space_separated_write_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hi > out.txt").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(1, RedirectDirection::Write, "out.txt")]
    );
    assert_eq!(cmd.args, vec![c"hi".into()]);
}

#[test]
fn test_space_separated_stdin_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cat < in.txt").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(0, RedirectDirection::Read, "in.txt")]
    );
    assert!(cmd.args.is_empty());
}

#[test]
fn test_space_separated_named_fd_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2> err.log").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(2, RedirectDirection::Write, "err.log")]
    );
}

#[test]
fn test_space_separated_append_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hi >> out.log").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(1, RedirectDirection::Append, "out.log")]
    );
}

#[test]
fn test_space_separated_rw_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 3<> rw.log").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(3, RedirectDirection::Rw, "rw.log")]
    );
}

#[test]
fn test_space_separated_redirect_quoted_path() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hi > \"my file\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 1,
            extra_to: None,
            direction: RedirectDirection::Write,
            source: RedirectSource::path_word(c"my file", runs(&[(true, 7)]), Vec::new(), true),
        }]
    );
}

/// The space-separated bare operator takes the magic path as its word.
#[test]
fn test_space_separated_redirect_fd_path_is_a_path() {
    let ParsedLine::Cmd(cmd) = parse(b"cat < /dev/fd/3").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(0, RedirectDirection::Read, "/dev/fd/3")]
    );
}

#[test]
fn test_space_separated_redirect_without_operand_is_error() {
    assert!(parse(b"cmd >").is_err());
    assert!(parse(b"cmd <").is_err());
    assert!(parse(b"cmd > ;").is_err());
    // `>|` is a bare operator, so its operand rules are the same ones.
    assert!(parse(b"cmd >|").is_err());
    assert!(parse(b"cmd >| ;").is_err());
    assert!(parse(b"cmd >| | f").is_err());
    assert!(parse(b"cmd >| >| g").is_err());
    assert!(parse(b"cmd >| %v").is_err());
    assert!(parse(b"cmd >| <<EOF").is_err());
}

#[test]
fn test_space_separated_redirect_before_operator_is_error() {
    assert!(parse(b"cmd > <file").is_err());
    assert!(parse(b"cmd < >file").is_err());
    assert!(parse(b"cmd > &1").is_err());
    assert!(parse(b"cmd > %v").is_err());
    // A clobber operator with no operand: the bare form takes the next word,
    // so these are operand rejections, not operator rejections.
    assert!(parse(b"cmd >| >file").is_err());
    assert!(parse(b"cmd >| &1").is_err());
    assert!(parse(b"cmd &>|f").is_err());
    // Append and read have no clobber form; the `|` after them is a pipeline
    // pipe, so the bare `>>`/`<` operator has no operand and is rejected.
    assert!(parse(b"cmd >>|f").is_err());
    assert!(parse(b"cmd <|f").is_err());
}

#[test]
fn test_space_separated_redirect_non_numeric_prefix_is_arg() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd a> file").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.redirects.is_empty());
    assert_eq!(cmd.args, vec![c"a>".into(), c"file".into()]);
}

#[test]
fn test_space_separated_redirect_in_pipeline_stage() {
    let ParsedLine::Pipeline(pl) = parse(b"echo a > out | cat").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(
        pl.command_at(0).unwrap().redirects,
        vec![path_def(1, RedirectDirection::Write, "out")]
    );
    assert!(pl.command_at(1).unwrap().redirects.is_empty());
}

#[test]
fn test_space_separated_redirect_with_heredoc() {
    let ParsedLine::Cmd(cmd) = parse(b"echo a <<EOF > out\nbody\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            RedirectDef::here_doc(0, c"body\n", true),
            path_def(1, RedirectDirection::Write, "out"),
        ]
    );
}

// POSIX #2.2 `>|` (clobber: bypasses `noclobber`) in every operator position
// the existing operators occupy, measured against bash 5.3.9.

#[test]
fn test_clobber_attached() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hi >|out.txt").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(1, RedirectDirection::Clobber, "out.txt")]
    );
    assert_eq!(cmd.args, vec![c"hi".into()]);
}

#[test]
fn test_clobber_bare() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hi >| out.txt").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(1, RedirectDirection::Clobber, "out.txt")]
    );
    assert_eq!(cmd.args, vec![c"hi".into()]);
}

#[test]
fn test_clobber_stderr() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>|err.log").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(2, RedirectDirection::Clobber, "err.log")]
    );
}

#[test]
fn test_clobber_var_form() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd >|%log").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 1,
            extra_to: None,
            direction: RedirectDirection::Clobber,
            source: RedirectSource::Var(c"log".into()),
        }]
    );
}

/// `>|` keeps its `Clobber` direction on the magic path: the open bypasses
/// `noclobber` and truncates the file fd 3 points at.
#[test]
fn test_clobber_bare_fd_path() {
    let ParsedLine::Cmd(cmd) = parse(b"cat >| /dev/fd/3").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![path_def(1, RedirectDirection::Clobber, "/dev/fd/3")]
    );
}

#[test]
fn test_clobber_bare_quoted_path() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hi >| \"my file\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 1,
            extra_to: None,
            direction: RedirectDirection::Clobber,
            source: RedirectSource::path_word(c"my file", runs(&[(true, 7)]), Vec::new(), true),
        }]
    );
}

#[test]
fn test_clobber_bare_in_pipeline_stage() {
    let ParsedLine::Pipeline(pl) = parse(b"echo a >| out | cat").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(
        pl.command_at(0).unwrap().redirects,
        vec![path_def(1, RedirectDirection::Clobber, "out")]
    );
    assert!(pl.command_at(1).unwrap().redirects.is_empty());
}

// `>&|f`: `>&` is the fd-dup operator, its `&…` tail is empty so no redirect
// is produced, and the `|` (not absorbed: the word ends at `&`, not at `>`) is
// the pipeline pipe. Measured: bash is a syntax error (rc 2); fdshell parses it
// as a pipeline, which exits non-zero on the second stage.
#[test]
fn test_amp_clobber_tail_is_a_pipeline_pipe() {
    let ParsedLine::Pipeline(pl) = parse(b"cmd 2>&|f").unwrap() else {
        panic!("expected Pipeline")
    };
    assert!(pl.command_at(0).unwrap().redirects.is_empty());
    assert_eq!(pl.command_at(0).unwrap().args, vec![c"2>&".into()]);
    assert_eq!(pl.command_at(1).unwrap().command, c"f".into());
}

// The `%`-target arm is evaluated after the operator is interpreted, so every
// write form takes the fd variable: `>%x` and `3<%x` are byte-identical to
// before `>|` existed, and `<>%x` follows the same rule as its operator.
#[test]
fn test_fd_var_target_after_each_operator() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd >%x").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.redirects, vec![RedirectDef::var(1, c"x")]);
    let ParsedLine::Cmd(cmd) = parse(b"cmd 3<%x").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 3,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Var(c"x".into()),
        }]
    );
    let ParsedLine::Cmd(cmd) = parse(b"cmd <>%x").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 0,
            extra_to: None,
            direction: RedirectDirection::Rw,
            source: RedirectSource::Var(c"x".into()),
        }]
    );
    // Append-to-fd-var stays rejected (a `>>` has nothing to append to a fd).
    assert!(parse(b"cmd >>%x").is_err());
    assert!(parse(b"cmd >>|%x").is_err());
}

#[test]
fn test_fd_dup_redirect_negative_tail_is_arg() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 2>&-5").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.redirects.is_empty());
    assert_eq!(cmd.args, vec![c"2>&-5".into()]);
}

#[test]
fn test_here_string_quoted_is_plain_arg() {
    let ParsedLine::Cmd(cmd) = parse(b"cat \"<<<x\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.redirects.is_empty());
    assert_eq!(cmd.args, vec![c"<<<x".into()]);
}

#[test]
fn test_append_followed_by_percent_is_error() {
    assert!(parse(b"echo >>%var").is_err());
    assert!(parse(b"echo 2>>%var").is_err());
}

#[test]
fn test_combined_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &>out.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![combined_def(RedirectDirection::Write, "out.log"),]
    );
}

#[test]
fn test_combined_redirect_append() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &>>out.log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![combined_def(RedirectDirection::Append, "out.log"),]
    );
}

#[test]
fn test_combined_redirect_var() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &>%log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::combined(
            RedirectDirection::Write,
            RedirectSource::Var(c"log".into()),
        )]
    );
}

#[test]
fn test_combined_redirect_then_stderr_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &>file 2>other").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            combined_def(RedirectDirection::Write, "file"),
            path_def(2, RedirectDirection::Write, "other"),
        ]
    );
}

#[test]
fn test_fd1_redirect_then_combined_last_wins() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd 1>other &>file").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            path_def(1, RedirectDirection::Write, "other"),
            combined_def(RedirectDirection::Write, "file"),
        ]
    );
}

#[test]
fn test_duplicate_read_redirects_keep_source_order() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <a <b <c").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            path_def(0, RedirectDirection::Read, "a"),
            path_def(0, RedirectDirection::Read, "b"),
            path_def(0, RedirectDirection::Read, "c"),
        ]
    );
}

#[test]
fn test_duplicate_write_redirects_last_is_last() {
    let ParsedLine::Cmd(cmd) = parse(b"echo hi >a >b").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            path_def(1, RedirectDirection::Write, "a"),
            path_def(1, RedirectDirection::Write, "b"),
        ]
    );
}

// The list is in the order the redirections were **written**: POSIX #2.5
// applies every redirection in that order, and "the last entry for a fd wins"
// follows from it. A duplicate key that arrives after a larger one therefore
// stays after it — sorting by target fd would move `1>b` before `2>e`.
#[test]
fn test_targets_stay_in_written_order() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd >a 2>e 1>b").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            path_def(1, RedirectDirection::Write, "a"),
            path_def(2, RedirectDirection::Write, "e"),
            path_def(1, RedirectDirection::Write, "b"),
        ]
    );
}

/// The `2>&1`/`> f` pair is written in the order the shell must apply it: a
/// fd-sorted table would move `> f` (fd 1) in front of `2>&1` (fd 2) and send
/// fd 2 to `f`, where bash keeps the shell's fd 1 (`tests/redirect.rs::
/// stderr_dup_before_a_write_redirect_keeps_the_shell_fd`).
#[test]
fn test_stderr_dup_and_write_redirect_keep_written_order() {
    for (line, want) in [
        (
            b"cmd 2>&1 > f".as_slice(),
            vec![
                RedirectDef::dup(2, 1),
                path_def(1, RedirectDirection::Write, "f"),
            ],
        ),
        (
            b"cmd > f 2>&1",
            vec![
                path_def(1, RedirectDirection::Write, "f"),
                RedirectDef::dup(2, 1),
            ],
        ),
    ] {
        let ParsedLine::Cmd(cmd) = parse(line).unwrap() else {
            panic!("line={line:?}: expected Cmd")
        };
        assert_eq!(cmd.redirects, want, "line={line:?}");
    }
}

#[test]
fn test_close_and_reopen_keep_order() {
    let ParsedLine::Cmd(cmd) = parse(b"cat 0>&- <f1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            RedirectDef::close(0),
            path_def(0, RedirectDirection::Read, "f1"),
        ]
    );
    let ParsedLine::Cmd(cmd) = parse(b"cat <f1 0>&-").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![
            path_def(0, RedirectDirection::Read, "f1"),
            RedirectDef::close(0),
        ]
    );
}

#[test]
fn test_combined_redirect_append_var() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &>>%log").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::combined(
            RedirectDirection::Append,
            RedirectSource::Var(c"log".into()),
        )]
    );
}

// The bare `&>` / `&>>` forms take the next word as their operand, with the
// plain-word rule the bare `>` family uses (task #145). Measured against bash
// 5.3: `cmd &> f` opens `f` once and shares it between fd 1 and fd 2 (bash's
// `> f 2>&1`), and the operand is never an argument.

#[test]
fn test_bare_combined_redirect_takes_next_token() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &> out.log").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![combined_def(RedirectDirection::Write, "out.log"),]
    );
    assert!(cmd.args.is_empty(), "the operand must not leak into args");
}

#[test]
fn test_bare_combined_append_takes_next_token() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &>> out.log").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![combined_def(RedirectDirection::Append, "out.log"),]
    );
    assert!(cmd.args.is_empty(), "the operand must not leak into args");
}

#[test]
fn test_bare_combined_operand_keeps_its_quote_mask() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd &> \"a b.log\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef {
            export_to: 1,
            extra_to: Some(2),
            direction: RedirectDirection::Write,
            source: RedirectSource::path_word(c"a b.log", runs(&[(true, 7)]), Vec::new(), true),
        }]
    );
    assert!(cmd.args.is_empty());
}

#[test]
fn test_bare_combined_following_words_stay_args() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd a &> f b").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"a".into(), c"b".into()]);
    assert_eq!(
        cmd.redirects,
        vec![combined_def(RedirectDirection::Write, "f"),]
    );
}

#[test]
fn test_bare_combined_in_pipeline_stage() {
    let ParsedLine::Pipeline(pl) = parse(b"echo hi &> f | cat").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(
        pl.command_at(0).unwrap().redirects,
        vec![combined_def(RedirectDirection::Write, "f"),]
    );
    assert_eq!(pl.command_at(0).unwrap().command, c"echo".into());
    assert_eq!(
        pl.command_at(0).unwrap().args,
        vec![c"hi".into()],
        "the operand must not leak into args"
    );
    assert_eq!(pl.command_at(1).unwrap().command, c"cat".into());
    assert!(pl.command_at(1).unwrap().redirects.is_empty());
    assert!(pl.command_at(1).unwrap().args.is_empty());
}

#[test]
fn test_bare_combined_missing_operand_is_error() {
    assert!(parse(b"cmd &>").is_err());
    assert!(parse(b"cmd &>>").is_err());
    assert!(parse(b"cmd &> ; echo x").is_err());
    assert!(parse(b"cmd &> && echo x").is_err());
    // `|` is a separator: a pipeline stage whose `&>` has no operand.
    assert!(parse(b"cmd &> | f").is_err());
    assert!(parse(b"cmd &> &job").is_err());
    assert!(parse(b"cmd &> %log").is_err());
    assert!(parse(b"cmd &> <f").is_err());
    // A heredoc operator is an operator, not a path operand — bash rejects it
    // with a syntax error too.
    assert!(parse(b"cmd &> <<EOF").is_err());
    assert!(parse(b"cmd &>> | f").is_err());
}

#[test]
fn test_bare_combined_fd_path_operand_is_a_path() {
    // The `&>` family never uses a `dup` def: the attached form opens the magic
    // path, and the bare form keeps doing the same (LESSONS).
    let ParsedLine::Cmd(cmd) = parse(b"cmd &> /dev/fd/3").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![combined_def(RedirectDirection::Write, "/dev/fd/3"),]
    );
}

#[test]
fn test_bare_combined_operand_does_not_shift_the_heredoc_body() {
    let ParsedLine::Cmd(cmd) = parse(b"echo a &> f <<EOF\nbody\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    // The written order: the `&>` def (fd 1) precedes the here-doc (fd 0). The
    // bare operand took `f`, not the body, and the body kept its own def.
    assert_eq!(
        cmd.redirects,
        vec![
            combined_def(RedirectDirection::Write, "f"),
            RedirectDef::here_doc(0, c"body\n", true),
        ]
    );
    assert_eq!(cmd.args, vec![c"a".into()]);
}

// --- quoted bytes end a `&>`-family operator (tasks #250, #264) --------------
//
// POSIX XCU 2.9.1 builds a redirection out of **unquoted** bytes, so a quote
// byte ends the operator and begins the word. `parse_bg_redirect` gates the
// whole family on the mask (`redirect::quoted_bytes`): fully quoted words —
// the `fq` flag task #250 gated, implied by the mask rule — and words whose
// operator bytes are only *partially* quoted are plain arguments. The gate
// covers the whole function — the attached, bare, pidvar and force-pidvar
// arms alike — so the `pidvar` / `bg_force` asserts below are the point of
// the pidvar test: a gate on the redirect construction alone passes without
// them. Every expectation is `bash -c` (5.3.9) measured 2026-10-20 in a
// scratch dir.

/// The one-word shape of a fully quoted `&>`-family word: `cmd` is `echo`, the
/// word is the single argument, and the command carries no redirect, no job.
fn assert_quoted_word_is_an_argument(line: &str, want: &str) {
    let ParsedLine::Cmd(cmd) = parse(line.as_bytes()).unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.command, c"echo".into(), "{line:?}");
    assert_eq!(
        cmd.args,
        vec![sys::ShortCStr::from_vec(want.as_bytes().to_vec()).unwrap()],
        "{line:?}"
    );
    assert!(cmd.redirects.is_empty(), "{line:?}");
    assert!(
        cmd.pidvar.is_none(),
        "{line:?}: a fully quoted word records no job"
    );
    assert!(!cmd.bg_force, "{line:?}: a fully quoted word forces no job");
}

#[test]
fn test_fully_quoted_combined_word_is_an_argument() {
    // The attached write and append arms.
    for (line, want) in [
        ("echo '&>q.txt'", "&>q.txt"),
        ("echo \"&>q.txt\"", "&>q.txt"),
        ("echo '&>>q.txt'", "&>>q.txt"),
    ] {
        assert_quoted_word_is_an_argument(line, want);
    }
}

#[test]
fn test_fully_quoted_pidvar_forms_are_arguments() {
    // The two pidvar arms: `&>&name` and `&>|&name`.
    for (line, want) in [
        ("echo '&>&name'", "&>&name"),
        ("echo '&>|&name'", "&>|&name"),
    ] {
        assert_quoted_word_is_an_argument(line, want);
    }
}

#[test]
fn test_fully_quoted_bare_combined_operator_is_an_argument() {
    // The bare arms take the *next* token as their operand, so both words stay
    // arguments: bash prints `hi &> f` and opens nothing.
    for (line, op) in [("echo '&>' f", "&>"), ("echo '&>>' f", "&>>")] {
        let ParsedLine::Cmd(cmd) = parse(line.as_bytes()).unwrap() else {
            panic!("expected Cmd")
        };
        assert_eq!(cmd.command, c"echo".into(), "{line:?}");
        let op = sys::ShortCStr::from_vec(op.as_bytes().to_vec()).unwrap();
        assert_eq!(cmd.args, vec![op, c"f".into()], "{line:?}");
        assert!(cmd.redirects.is_empty(), "{line:?}");
    }
}

#[test]
fn test_fully_quoted_operator_with_expansion_is_an_argument() {
    // The whole word is quoted *including* its expansion, so its mask is
    // all-`true`. A gate keyed on mask emptiness would read this word as an
    // operator and create a file named `$(echo x)f` (measured, old tree);
    // `fq` is the flag the tokenizer sets for exactly this shape.
    let ParsedLine::Cmd(cmd) = parse(b"echo \"&>$(echo x)f\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"&>$(echo x)f".into()]);
    assert!(cmd.redirects.is_empty());
}

#[test]
fn test_quoted_operator_forms_keep_their_side_by_side_rule() {
    // Fully quoted: a plain word (bash parity, task #250's gate).
    let ParsedLine::Cmd(cmd) = parse(b"echo '&>' f").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"&>".into(), c"f".into()]);
    assert!(cmd.redirects.is_empty());
    // Unquoted: the operator, unchanged.
    let ParsedLine::Cmd(cmd) = parse(b"echo &> f").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![combined_def(RedirectDirection::Write, "f"),]
    );
    assert!(cmd.args.is_empty());
    // Partially quoted: a quote byte in the operator ends it (POSIX XCU
    // 2.9.1), so these words are arguments and open nothing — bash parity
    // for `'&>'f` (measured 2026-10-20: bash prints `&>f`, opens nothing).
    let ParsedLine::Cmd(cmd) = parse(b"echo '&>'f").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"&>f".into()]);
    assert!(cmd.redirects.is_empty(), "`'&>'f` is one plain word");
    assert!(
        cmd.pidvar.is_none() && !cmd.bg_force,
        "no job from a quoted operator"
    );
    // `"&">b`: fdshell's word rule keeps the whole word an argument; bash
    // splits at the unquoted `>` and writes `b` = `x &` (the word-split
    // family, POSIX_COMPATIBILITY "Operators are recognized at word starts").
    let ParsedLine::Cmd(cmd) = parse(b"echo a \"&\">b").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"a".into(), c"&>b".into()]);
    assert!(cmd.redirects.is_empty());
}

#[test]
fn test_fully_quoted_combined_in_pipeline_stage() {
    // Pipeline stages route through the same `finish_command` dispatch, so the
    // single gate covers them: no second path into the `&>` family.
    let ParsedLine::Pipeline(pl) = parse(b"echo '&>f' | cat").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pl.command_at(0).unwrap().args, vec![c"&>f".into()]);
    assert!(pl.command_at(0).unwrap().redirects.is_empty());
    assert!(pl.command_at(0).unwrap().pidvar.is_none());
    assert_eq!(pl.command_at(1).unwrap().command, c"cat".into());
    assert!(pl.command_at(1).unwrap().args.is_empty());
    assert!(pl.command_at(1).unwrap().redirects.is_empty());
}

#[test]
fn test_fully_quoted_bare_operator_does_not_swallow_a_heredoc() {
    // The byte-level heredoc scan honors quotes and counts the `<<EOF` body;
    // the token layer used to turn the quoted `&>` into a bare operator that
    // took the next word (`b`) — or the `<<EOF` itself, a parse error — as its
    // operand. After the gate both layers see the same words: the `&>` is an
    // argument and the heredoc is fd 0's redirect. bash parity, measured
    // 2026-10-20 (`echo a &> b` prints `a &> b`, opens nothing).
    let ParsedLine::Cmd(cmd) = parse(b"echo a '&>' b <<EOF\nbody\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"a".into(), c"&>".into(), c"b".into()]);
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_doc(0, c"body\n", true)]
    );
    let ParsedLine::Cmd(cmd) = parse(b"echo '&>' <<EOF\nbody\nEOF").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args, vec![c"&>".into()]);
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_doc(0, c"body\n", true)]
    );
}

#[test]
fn test_renameat2() {
    let ParsedLine::Cmd(cmd) =
        parse(b"builtin renameat2 --olddirfd %foo --newdirfd %bar baz qux").unwrap()
    else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.prefix, BuiltinPrefix::Builtin);
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
fn test_assign_array_empty() {
    let ParsedLine::AssignArrayEmpty { var } = parse(b"%arr=[]").unwrap() else {
        panic!("expected AssignArrayEmpty")
    };
    assert_eq!(var, c"arr".into());
}

#[test]
fn test_append_fd() {
    let ParsedLine::AppendFd { var, value } = parse(b"%arr+=%conn").unwrap() else {
        panic!("expected AppendFd")
    };
    assert_eq!(var, c"arr".into());
    assert_eq!(value, c"conn".into());
}

#[test]
fn test_assign_fd_index() {
    let ParsedLine::AssignFdIndex { var, value, index } = parse(b"%x=%arr[2]").unwrap() else {
        panic!("expected AssignFdIndex")
    };
    assert_eq!(var, c"x".into());
    assert_eq!(value, c"arr".into());
    assert_eq!(index, 2);
}

#[test]
fn test_assign_fd_index_zero() {
    let result = parse(b"%x=%arr[0]").unwrap();
    assert!(matches!(result, ParsedLine::AssignFdIndex { index: 0, .. }));
}

#[test]
fn test_assign_fd_non_numeric_index_is_plain_name() {
    let ParsedLine::AssignFd { var, value } = parse(b"%x=%arr[abc]").unwrap() else {
        panic!("expected AssignFd")
    };
    assert_eq!(var, c"x".into());
    assert_eq!(value, c"arr[abc]".into());
}

#[test]
fn test_unset_array_entry() {
    let ParsedLine::Unset(targets) = parse(b"unset %arr[%conn]").unwrap() else {
        panic!("expected Unset")
    };
    let [UnsetTarget::FdArrayEntry { var, source }] = targets.as_slice() else {
        panic!("expected one fd-array-entry target")
    };
    assert_eq!(*var, c"arr".into());
    assert_eq!(*source, c"conn".into());
}

#[test]
fn test_unset_array_entry_malformed_is_plain_unset() {
    let ParsedLine::Unset(targets) = parse(b"unset %arr[conn]").unwrap() else {
        panic!("expected Unset")
    };
    let [UnsetTarget::Fd(var)] = targets.as_slice() else {
        panic!("expected one fd target")
    };
    assert_eq!(*var, c"arr[conn]".into());
}

#[test]
fn test_assign_str_simple() {
    let ParsedLine::AssignStr(a) = parse(b"var=hello").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(a.name, c"var".into());
    assert_eq!(a.value, c"hello".into());
    assert_eq!(a.mask, vec![Unquoted; 5]);
}

#[test]
fn test_assign_str_empty_value() {
    let ParsedLine::AssignStr(a) = parse(b"var=").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(a.name, c"var".into());
    assert_eq!(a.value, c"".into());
    assert!(a.mask.is_empty());
}

#[test]
fn test_assign_str_quoted_spaces() {
    let ParsedLine::AssignStr(a) = parse(b"var=\"foo bar\"").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(a.name, c"var".into());
    assert_eq!(a.value, c"foo bar".into());
    assert_eq!(a.mask, runs(&[(true, 7)]));
}

#[test]
fn test_assign_str_no_lhs_is_not_assign() {
    let result = parse(b"=value").unwrap();
    assert!(!matches!(result, ParsedLine::AssignStr(_)));
}

#[test]
fn test_assign_str_fd_assign_takes_priority() {
    let result = parse(b"%x=%y").unwrap();
    assert!(matches!(result, ParsedLine::AssignFd { .. }));
}

#[test]
fn test_assign_str_percent_value_is_literal() {
    let ParsedLine::AssignStr(a) = parse(b"var=%othervar").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(a.name, c"var".into());
    assert_eq!(a.value, c"%othervar".into());
    assert_eq!(a.mask, vec![Unquoted; 9]);
}

#[test]
fn test_unset() {
    let ParsedLine::Unset(targets) = parse(b"unset %client").unwrap() else {
        panic!("expected Unset")
    };
    let [UnsetTarget::Fd(var)] = targets.as_slice() else {
        panic!("expected one fd target")
    };
    assert_eq!(*var, c"client".into());
}

#[test]
fn test_unset_missing_is_ok() {
    // The run layer decides "unbound" (rc 0, silent); the parse layer has no
    // operand it can reject, so `%nonexistent` parses like any other word.
    let ParsedLine::Unset(targets) = parse(b"unset %nonexistent").unwrap() else {
        panic!("expected Unset")
    };
    let [UnsetTarget::Fd(var)] = targets.as_slice() else {
        panic!("expected one fd target")
    };
    assert_eq!(*var, c"nonexistent".into());
}

#[test]
fn test_unset_plain_names() {
    // `unset name…` is bash's spelling: every word is a target, in order, and a
    // plain word is a shell variable (task #290).
    let ParsedLine::Unset(targets) = parse(b"unset v %f w").unwrap() else {
        panic!("expected Unset")
    };
    let [UnsetTarget::Var(v), UnsetTarget::Fd(f), UnsetTarget::Var(w)] = targets.as_slice() else {
        panic!("expected var, fd, var")
    };
    assert_eq!(*v, c"v".into());
    assert_eq!(*f, c"f".into());
    assert_eq!(*w, c"w".into());
}

#[test]
fn test_unset_no_operand_is_empty_list() {
    // bash runs a bare `unset` as a no-op at rc 0, so the parse layer must
    // produce a statement, not an error.
    let ParsedLine::Unset(targets) = parse(b"unset").unwrap() else {
        panic!("expected Unset")
    };
    assert!(targets.is_empty());
}

#[test]
fn test_execveat2_builtin() {
    let ParsedLine::Cmd(cmd) = parse(b"builtin execveat2 %MYBIN myprog arg1").unwrap() else {
        panic!("expected Cmd")
    };

    assert_eq!(cmd.prefix, BuiltinPrefix::Builtin);
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
    assert_eq!(pipe.stages.len(), 2);
    assert_eq!(pipe.command_at(0).unwrap().command, c"echo".into());
    assert_eq!(pipe.command_at(0).unwrap().args, vec![c"hello".into()]);
    assert_eq!(pipe.command_at(1).unwrap().command, c"wc".into());
    assert_eq!(pipe.command_at(1).unwrap().args, vec![c"-l".into()]);
}

#[test]
fn test_pipeline_three_commands() {
    let ParsedLine::Pipeline(pipe) = parse(b"a | b | c").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.stages.len(), 3);
    assert_eq!(pipe.command_at(0).unwrap().command, c"a".into());
    assert_eq!(pipe.command_at(1).unwrap().command, c"b".into());
    assert_eq!(pipe.command_at(2).unwrap().command, c"c".into());
}

#[test]
fn test_pipeline_with_captures() {
    let ParsedLine::Pipeline(pipe) = parse(b"cmd1 %>%a | cmd2 %>%b").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.stages.len(), 2);
    assert_eq!(pipe.command_at(0).unwrap().captures.len(), 1);
    assert_eq!(pipe.command_at(1).unwrap().captures.len(), 1);
}

#[test]
fn test_pipeline_with_redirect() {
    let ParsedLine::Pipeline(pipe) = parse(b"cmd1 2>%log | cmd2").unwrap() else {
        panic!("expected Pipeline")
    };
    assert_eq!(pipe.command_at(0).unwrap().redirects.len(), 1);
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

/// The quote bytes of the `umask` keyword word are transparent (task #335): the
/// detector reads the word's **text**, so every spelling is the `umask 022`
/// row's `ParsedLine`. The site is ungated on purpose — fdshell has no `umask`
/// builtin, so gating it would trade the mask for `umask: not found`
/// (POSIX_COMPATIBILITY.md, "Quoted keywords are command words, quoted control
/// keywords are not").
#[test]
fn test_umask_quoted_keyword_word_is_transparent() {
    for line in [&b"'umask' 022"[..], b"u\"mask\" 022", b"umask 022"] {
        let ParsedLine::Umask(mask) = parse(line).unwrap() else {
            panic!("expected Umask for {line:?}")
        };
        assert_eq!(mask, Some(0o022));
    }
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
    let expected: &[&[u8]] = &[
        b"if", b"true", b";", b"then", b";", b"umask", b"0o000", b";", b"fi",
    ];
    let expected_pos: &[usize] = &[0, 3, 7, 8, 12, 13, 19, 24, 25];
    assert_eq!(tokens.len(), expected.len());
    for (i, (token, pos, _, _, _, _)) in tokens.iter().enumerate() {
        assert_eq!(token.as_bytes().unwrap(), expected[i]);
        assert_eq!(*pos, expected_pos[i]);
    }
}

#[test]
fn parse_if_newline_separators() {
    let ParsedLine::If(ib) = parse(b"if true\nthen\numask 0o000\nfi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"true".into());
    assert_eq!(ib.then_body.data, c"umask 0o000".into());
    assert!(ib.elifs.is_empty());
    assert!(ib.else_body.is_none());
}

/// Task #236: `if test fi` has no command-position `then` and its `fi` sits in
/// the condition's **argument** position, so the closer never scores and the
/// `if` finds no `fi`: `missing 'fi'` (was `missing 'then'`, produced by the
/// ungated scorer closing the block at the argument-`fi`). Measured against
/// `bash -c`: rc 2 syntax error; fdshell reports the parse error with rc 1.
#[test]
fn if_without_then_returns_err() {
    let result = parse(b"if test fi");
    let e = match result {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    assert_eq!(
        e.current_context().to_string(),
        "missing 'fi'",
        "expected Reason('missing \\'fi\\'') error"
    );
}

#[test]
fn if_without_then_through_run_script_returns_err() {
    // This verifies the full path: run_script → run_cond_list → run_one → parse → tokens_to_if
    // Both paths should return the same error message.
    let cell = crate::state::ShellState::new();
    let cell = sys::fork_cell::ForkCell::new(cell);
    let result = crate::repl::run_script(&stext(b"if test fi"), &cell);
    assert!(result.is_err());
    let e = match result {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    // Parse errors (missing 'then', missing 'fi', etc.) map to CmdError::Parse.
    assert!(matches!(e.current_context(), CmdError::Parse));
}

#[test]
fn for_simple() {
    let line = b"for var in a b c; do echo $var; done";
    let tokens = token::tokenize(line).unwrap();
    let fb = for_block::tokens_to_for(&tokens, &stext(line)).unwrap();
    assert_eq!(fb.var, c"var".into());
    assert_eq!(fb.words, vec![c"a".into(), c"b".into(), c"c".into()]);
    assert_eq!(fb.body.data, c"echo $var".into());
}

#[test]
fn for_single_word() {
    let line = b"for x in foo; do cmd; done";
    let tokens = token::tokenize(line).unwrap();
    let fb = for_block::tokens_to_for(&tokens, &stext(line)).unwrap();
    assert_eq!(fb.var, c"x".into());
    assert_eq!(fb.words, vec![c"foo".into()]);
    assert_eq!(fb.body.data, c"cmd".into());
}

#[test]
fn for_newline_separators() {
    let line = b"for x in a b c\ndo\ncmd1; cmd2\ndone";
    let tokens = token::tokenize(line).unwrap();
    let fb = for_block::tokens_to_for(&tokens, &stext(line)).unwrap();
    assert_eq!(fb.var, c"x".into());
    assert_eq!(fb.words, vec![c"a".into(), c"b".into(), c"c".into()]);
    assert_eq!(fb.body.data, c"cmd1; cmd2".into());
}

#[test]
fn for_empty_words() {
    let line = b"for x in ; do cmd; done";
    let tokens = token::tokenize(line).unwrap();
    let fb = for_block::tokens_to_for(&tokens, &stext(line)).unwrap();
    assert_eq!(fb.var, c"x".into());
    assert!(fb.words.is_empty());
    assert_eq!(fb.body.data, c"cmd".into());
}

#[test]
fn for_missing_do_returns_err() {
    let line = b"for x in a; done";
    let tokens = token::tokenize(line).unwrap();
    assert!(for_block::tokens_to_for(&tokens, &stext(line)).is_err());
}

#[test]
fn for_missing_done_returns_err() {
    let line = b"for x in a; do cmd";
    let tokens = token::tokenize(line).unwrap();
    assert!(for_block::tokens_to_for(&tokens, &stext(line)).is_err());
}

#[test]
fn for_no_in_returns_err() {
    let line = b"for x a; do cmd; done";
    let tokens = token::tokenize(line).unwrap();
    assert!(for_block::tokens_to_for(&tokens, &stext(line)).is_err());
}

#[test]
fn for_not_starting_with_for_is_not_a_for() {
    let line = b"while x; do cmd; done";
    let tokens = token::tokenize(line).unwrap();
    assert!(for_block::tokens_to_for(&tokens, &stext(line)).is_err());
}

#[test]
fn for_parse_dispatch() {
    let ParsedLine::For(fb) = parse(b"for var in a b c; do echo $var; done").unwrap() else {
        panic!("expected For")
    };
    assert_eq!(fb.var, c"var".into());
    assert_eq!(fb.words, vec![c"a".into(), c"b".into(), c"c".into()]);
    assert_eq!(fb.body.data, c"echo $var".into());
}

#[test]
fn tokenize_backtick_command() {
    let result = token::tokenize(b"echo `seq 1 10`").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"echo".into());
    assert_eq!(result[1].0, c"`seq 1 10`".into());
}

#[test]
fn tokenize_dollar_paren_command() {
    let result = token::tokenize(b"echo $(seq 1 10)").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"echo".into());
    assert_eq!(result[1].0, c"$(seq 1 10)".into());
}

#[test]
fn tokenize_backtick_empty() {
    let result = token::tokenize(b"for x in ``; do body; done").unwrap();
    assert_eq!(result[3].0, c"``".into());
}

#[test]
fn tokenize_dollar_paren_nested() {
    let result = token::tokenize(b"$(echo $(seq 3))").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, c"$(echo $(seq 3))".into());
}

#[test]
fn tokenize_backtick_escape_positions() {
    // The body keeps every escape pair verbatim (`scan_backtick`'s rule): the
    // backslash shields the byte in this scan, and the child that runs the
    // body re-parses the pair as its own literal.
    let result = token::tokenize(b"`a\\`\\\\\\z` b").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"`a\\`\\\\\\z`".into());
    assert_eq!(result[0].1, 0);
    assert_eq!(result[1].0, c"b".into());
    assert_eq!(result[1].1, 10);
}

#[test]
fn tokenize_dollar_paren_positions() {
    let result = token::tokenize(b"$(a(b)c) x").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"$(a(b)c)".into());
    assert_eq!(result[0].1, 0);
    assert_eq!(result[1].0, c"x".into());
    assert_eq!(result[1].1, 9);
}

#[test]
fn tokenize_dollar_paren_paren_in_quotes_is_data() {
    let result = token::tokenize(b"x=$(echo \"a)b\") y").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"x=$(echo \"a)b\")".into());
    assert_eq!(result[0].1, 0);
    assert_eq!(result[1].0, c"y".into());
    assert_eq!(result[1].1, 16);
}

#[test]
fn tokenize_dollar_paren_open_paren_in_quotes_is_data() {
    let result = token::tokenize(b"$(a\"(b)c\") d").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"$(a\"(b)c\")".into());
    assert_eq!(result[1].0, c"d".into());
}

#[test]
fn tokenize_dollar_paren_escaped_quote_in_quotes_is_data() {
    let result = token::tokenize(b"x=$(echo \"a\\\"b)\")").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, c"x=$(echo \"a\\\"b)\")".into());
}

#[test]
fn tokenize_dollar_paren_escaped_quote_positions() {
    let result = token::tokenize(b"x=$(echo \"a\\\"b\") y").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"x=$(echo \"a\\\"b\")".into());
    assert_eq!(result[0].1, 0);
    assert_eq!(result[1].0, c"y".into());
    assert_eq!(
        result[1].1, 17,
        "pos must advance past both escape bytes before 'y'"
    );
}

#[test]
fn tokenize_dollar_paren_unclosed_quote_is_eof() {
    let result = token::tokenize(b"$(echo \"a)");
    assert!(matches!(
        result.unwrap_err().current_context(),
        ParseError::UnexpectedEof
    ));
}

#[test]
fn tokenize_dollar_paren_trailing_escape_in_quotes_is_eof() {
    let result = token::tokenize(b"$(echo \"a\\");
    assert!(matches!(
        result.unwrap_err().current_context(),
        ParseError::UnexpectedEof
    ));
}

#[test]
fn tokenize_quoted_escape_positions() {
    let result = token::tokenize(b"\"a\\\"b\" c").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, c"a\"b".into());
    assert_eq!(result[0].1, 0);
    assert_eq!(result[1].0, c"c".into());
    assert_eq!(result[1].1, 7);
}

#[test]
fn tokenize_dquote_backslash_non_special_preserved() {
    // `\n` is data inside double quotes: the backslash is retained.
    let tokens = token::tokenize(b"\"a\\nb\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"a\\nb".into());
    assert!(tokens[0].3);
}

#[test]
fn tokenize_dquote_backslash_before_special_dropped() {
    // `\"` drops the backslash; `\\`, `\$` and `` \` `` keep both bytes so
    // substitution resolves them (`\$`→an unexpanded `$`, `\\`→`\`, and
    // `` \` ``→a literal backtick that is never a command-substitution
    // delimiter — task #216).
    let cases = [
        (b"\"a\\\"b\"", c"a\"b"),
        (b"\"a\\`b\"", c"a\\`b"),
        (b"\"a\\\\b\"", c"a\\\\b"),
        (b"\"a\\$b\"", c"a\\$b"),
    ];
    for (input, expected) in cases {
        let tokens = token::tokenize(input).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].0, expected.into());
        assert!(tokens[0].3);
    }
}

#[test]
fn tokenize_dquote_backslash_newline_is_continuation() {
    // `\<newline>` inside double quotes is line continuation: both bytes removed.
    let tokens = token::tokenize(b"\"a\\\nb\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"ab".into());
    assert!(tokens[0].3);
}

#[test]
fn tokenize_dquote_backslash_at_eof_is_error() {
    let result = token::tokenize(b"\"a\\");
    assert!(matches!(
        result.unwrap_err().current_context(),
        ParseError::UnexpectedEof
    ));
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
    assert_eq!(wb.condition.data, c"true".into());
    assert_eq!(wb.body.data, c"echo x".into());
}

/// Task #236: `while true; do echo x done` has its `done` in the body's
/// **argument** position (behind the command word `echo`), so it never closes
/// the block: the parse reports `expected 'done'` (was: the argument-`done`
/// closed the block and the body `echo x` was accepted without the required
/// separator). Measured against `bash -c`: rc 2 syntax error; fdshell reports
/// the parse error with rc 1.
#[test]
fn while_parse_body_without_semi_before_done() {
    let err = parse(b"while true; do echo x done").err().unwrap();
    assert_eq!(err.current_context().to_string(), "expected 'done'");
}

#[test]
fn while_parse_with_semicolon_body() {
    let ParsedLine::While(wb) = parse(b"while false; do echo a; echo b; done").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.condition.data, c"false".into());
    let body_bytes = wb.body.as_bytes().unwrap();
    // Body is all tokens between "do" and "done", joined with spaces
    assert!(body_bytes.starts_with(b"echo"));
}

#[test]
fn while_parse_pipe_in_body() {
    let ParsedLine::While(wb) = parse(b"while true; do echo hello | cat; done").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.condition.data, c"true".into());
    assert_eq!(wb.body.data, c"echo hello | cat".into());
}

#[test]
fn while_parse_newline_separator() {
    let ParsedLine::While(wb) = parse(b"while umask 0o077\ndo\numask 0o000\ndone").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.condition.data, c"umask 0o077".into());
    assert_eq!(wb.body.data, c"umask 0o000".into());
}

#[test]
fn wait_builtin_bare_is_cmd() {
    let ParsedLine::Cmd(cmd) = parse(b"wait").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.command.eq_bytes(b"wait"));
    assert!(cmd.args.is_empty());
}

#[test]
fn wait_builtin_with_pid_is_cmd() {
    let ParsedLine::Cmd(cmd) = parse(b"wait 123").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.command.eq_bytes(b"wait"));
    assert_eq!(cmd.args.len(), 1);
    assert!(cmd.args[0].eq_bytes(b"123"));
}

#[test]
fn wait_builtin_quoted_pattern_is_cmd() {
    let ParsedLine::Cmd(cmd) = parse(b"wait \"readable\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.command.eq_bytes(b"wait"));
    assert_eq!(cmd.args.len(), 1);
    assert!(cmd.args[0].eq_bytes(b"readable"));
}

#[test]
fn wait_block_is_wait() {
    let ParsedLine::Wait(wb) = parse(b"wait\n readable %rd) echo hi ;;\ndone").unwrap() else {
        panic!("expected Wait")
    };
    assert_eq!(wb.arms.len(), 1);
}

#[test]
fn while_not_starting_with_while_is_a_cmd() {
    let tokens = token::tokenize(b"while_true; do body; done").unwrap();
    assert!(
        !tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"while"))
    );
}

#[test]
fn while_without_do_returns_err() {
    let line = b"while true; echo x; done";
    let tokens = token::tokenize(line).unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"while", &stext(line)).is_err());
}

#[test]
fn while_missing_done_at_end_returns_err() {
    let result = parse(b"while true; do echo x");
    assert!(result.is_err());
}

#[test]
fn while_do_not_preceded_by_semi_returns_err() {
    let line = b"while true do echo x; done";
    let tokens = token::tokenize(line).unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"while", &stext(line)).is_err());
}

#[test]
fn until_parse_dispatch() {
    let ParsedLine::Until(wb) = parse(b"until false; do echo x; done").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition.data, c"false".into());
    assert_eq!(wb.body.data, c"echo x".into());
}

/// Task #236: the `until` twin of the `while` pin — the body's argument-position
/// `done` never closes the block, so `expected 'done'` (bash: rc 2 syntax
/// error; fdshell: the parse error with rc 1).
#[test]
fn until_parse_body_without_semi_before_done() {
    let err = parse(b"until true; do echo x done").err().unwrap();
    assert_eq!(err.current_context().to_string(), "expected 'done'");
}

#[test]
fn until_parse_with_semicolon_body() {
    let ParsedLine::Until(wb) = parse(b"until false; do echo a; echo b; done").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition.data, c"false".into());
    let body_bytes = wb.body.as_bytes().unwrap();
    assert!(body_bytes.starts_with(b"echo"));
}

#[test]
fn until_parse_newline_separator() {
    let ParsedLine::Until(wb) = parse(b"until false\ndo\numask 0o000\ndone").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition.data, c"false".into());
    assert_eq!(wb.body.data, c"umask 0o000".into());
}

#[test]
fn until_not_starting_with_until_is_a_cmd() {
    let tokens = token::tokenize(b"until_true; do body; done").unwrap();
    assert!(
        !tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"until"))
    );
}

#[test]
fn until_without_do_returns_err() {
    let line = b"until true; echo x; done";
    let tokens = token::tokenize(line).unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"until", &stext(line)).is_err());
}

#[test]
fn until_missing_done_at_end_returns_err() {
    let result = parse(b"until true; do echo x");
    assert!(result.is_err());
}

#[test]
fn until_do_not_preceded_by_semi_returns_err() {
    let line = b"until true do echo x; done";
    let tokens = token::tokenize(line).unwrap();
    assert!(while_block::tokens_to_loop(&tokens, b"until", &stext(line)).is_err());
}

#[test]
fn until_parse_pipe_in_body() {
    let ParsedLine::Until(wb) = parse(b"until true; do echo hello | cat; done").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(wb.condition.data, c"true".into());
    assert_eq!(wb.body.data, c"echo hello | cat".into());
}

#[test]
fn if_single_elif() {
    let ParsedLine::If(ib) = parse(b"if false; then a; elif true; then b; fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"false".into());
    assert_eq!(ib.then_body.data, c"a".into());
    assert_eq!(ib.elifs.len(), 1);
    assert_eq!(ib.elifs[0].cond.data, c"true".into());
    assert_eq!(ib.elifs[0].body.data, c"b".into());
    assert!(ib.else_body.is_none());
}

#[test]
fn if_multiple_elifs() {
    let ParsedLine::If(ib) = parse(b"if a; then b; elif c; then d; elif e; then f; fi").unwrap()
    else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b".into());
    assert_eq!(ib.elifs.len(), 2);
    assert_eq!(ib.elifs[0].cond.data, c"c".into());
    assert_eq!(ib.elifs[0].body.data, c"d".into());
    assert_eq!(ib.elifs[1].cond.data, c"e".into());
    assert_eq!(ib.elifs[1].body.data, c"f".into());
}

#[test]
fn if_elif_with_else() {
    let ParsedLine::If(ib) = parse(b"if a; then b; elif c; then d; else e; fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b".into());
    assert_eq!(ib.elifs.len(), 1);
    assert_eq!(ib.elifs[0].cond.data, c"c".into());
    assert_eq!(ib.elifs[0].body.data, c"d".into());
    assert_eq!(ib.else_body.unwrap().data, c"e".into());
}

#[test]
fn if_elif_else_complex() {
    let ParsedLine::If(ib) =
        parse(b"if x; then y; elif z; then w; elif m; then n; else o; fi").unwrap()
    else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"x".into());
    assert_eq!(ib.then_body.data, c"y".into());
    assert_eq!(ib.elifs.len(), 2);
    assert_eq!(ib.elifs[0].cond.data, c"z".into());
    assert_eq!(ib.elifs[0].body.data, c"w".into());
    assert_eq!(ib.elifs[1].cond.data, c"m".into());
    assert_eq!(ib.elifs[1].body.data, c"n".into());
    assert_eq!(ib.else_body.unwrap().data, c"o".into());
}

#[test]
fn if_elif_semi_before_then() {
    let ParsedLine::If(ib) = parse(b"if a;then b;elif c;then d;fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b".into());
    assert_eq!(ib.elifs.len(), 1);
    assert_eq!(ib.elifs[0].cond.data, c"c".into());
    assert_eq!(ib.elifs[0].body.data, c"d".into());
}

#[test]
fn if_elif_semi_after_then() {
    let ParsedLine::If(ib) = parse(b"if a; then; b; elif c; then; d; fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b".into());
    assert_eq!(ib.elifs.len(), 1);
    assert_eq!(ib.elifs[0].cond.data, c"c".into());
    assert_eq!(ib.elifs[0].body.data, c"d".into());
}

#[test]
fn if_elif_missing_then_returns_err() {
    let result = parse(b"if a; then b; elif c; fi");
    match result {
        Ok(_) => panic!("expected error"),
        Err(e) => {
            assert_eq!(
                e.current_context().to_string(),
                "missing 'then' after 'elif'",
                "expected Reason('missing \\'then\\' after \\'elif\\') error"
            );
        }
    }
}

#[test]
fn if_else_only() {
    let ParsedLine::If(ib) = parse(b"if false; then true; else fallback; fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"false".into());
    assert_eq!(ib.then_body.data, c"true".into());
    assert!(ib.elifs.is_empty());
    assert_eq!(ib.else_body.unwrap().data, c"fallback".into());
}

#[test]
fn if_multiple_elifs_with_else() {
    let ParsedLine::If(ib) =
        parse(b"if a; then b; elif c; then d; elif e; then f; else g; fi").unwrap()
    else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b".into());
    assert_eq!(ib.elifs.len(), 2);
    assert_eq!(ib.elifs[0].cond.data, c"c".into());
    assert_eq!(ib.elifs[0].body.data, c"d".into());
    assert_eq!(ib.elifs[1].cond.data, c"e".into());
    assert_eq!(ib.elifs[1].body.data, c"f".into());
    assert_eq!(ib.else_body.unwrap().data, c"g".into());
}

#[test]
fn if_elif_multiline() {
    let ParsedLine::If(ib) = parse(
        b"if a
then
b
elif c
then
d
else
e
fi",
    )
    .unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b".into());
    assert_eq!(ib.elifs.len(), 1);
    assert_eq!(ib.elifs[0].cond.data, c"c".into());
    assert_eq!(ib.elifs[0].body.data, c"d".into());
    assert_eq!(ib.else_body.unwrap().data, c"e".into());
}

/// bash 5.3.9 rejects the empty `else` body (rc 2, task #233); the arm bodies
/// must be non-empty at parse time.
#[test]
fn if_elif_empty_else_body_is_err() {
    let err = parse(b"if a; then b; elif c; then d; else; fi")
        .err()
        .unwrap();
    assert_eq!(err.current_context().to_string(), "'else' body is empty");
}

/// The structural asserts of the old empty-`else` pin, on the green sibling:
/// the same shape with a non-empty `else` body parses as before.
#[test]
fn if_elif_with_else_body() {
    let ParsedLine::If(ib) = parse(b"if a; then b; elif c; then d; else e; fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b".into());
    assert_eq!(ib.elifs.len(), 1);
    assert_eq!(ib.elifs[0].cond.data, c"c".into());
    assert_eq!(ib.elifs[0].body.data, c"d".into());
    assert_eq!(ib.else_body.unwrap().data, c"e".into());
}

// --- empty arm bodies (task #233) ------------------------------------------
//
// POSIX #2.9 requires a non-empty `list` after `then`, after `elif … then`,
// after `else` and after `do`; bash 5.3.9 rejects every empty form (rc 2,
// measured). fdshell reports its own message with rc 1 (README "Errors").

/// Asserts the exact top-level message of a parse error.
fn body_err(line: &[u8], msg: &str) {
    let err = parse(line).err().unwrap();
    assert_eq!(err.current_context().to_string(), msg, "{line:?}");
}

#[test]
fn empty_then_body_is_err() {
    body_err(b"if true; then; fi", "'then' body is empty");
    body_err(b"if true; then ; fi", "'then' body is empty");
    body_err(b"if true; then ;; fi", "'then' body is empty");
    body_err(b"if true\nthen\nfi", "'then' body is empty");
    // The tokenizer drops comments, so a comment-only body is an empty
    // range — bash rejects it too: a comment is not a command.
    body_err(b"if true; then\n# c\nfi", "'then' body is empty");
}

#[test]
fn empty_elif_body_is_err() {
    body_err(b"if a; then b; elif c; then; fi", "'elif' body is empty");
}

#[test]
fn empty_else_body_is_err() {
    body_err(b"if a; then b; else; fi", "'else' body is empty");
}

#[test]
fn empty_do_body_is_err() {
    body_err(b"while true; do; done", "'do' body is empty");
    body_err(b"while true; do\n# c\ndone", "'do' body is empty");
    body_err(b"until false; do; done", "'do' body is empty");
    body_err(b"for i in a; do; done", "'do' body is empty");
}

/// Task #301 owns the message where the closing keyword is glued straight
/// behind a clause keyword: POSIX #2.9 wants a terminator in front of the
/// closer, so the missing `;` is the error, not an empty body (bash: rc 2 for
/// every row). Each row keeps a real `;`/newline in front of the closer and
/// stays an empty-body error above.
#[test]
fn glued_closer_reports_the_missing_semicolon_not_an_empty_body() {
    body_err(b"if true; then fi", "expected ';' before 'fi'");
    body_err(b"if a; then b; elif c; then fi", "expected ';' before 'fi'");
    body_err(b"if a; then b; else fi", "expected ';' before 'fi'");
    body_err(b"while true; do done", "expected ';' before 'done'");
}

#[test]
fn for_semi_before_done_reports_before_the_empty_body() {
    // Message contract: the `;`-before-`done` ensure fires first in `for`.
    body_err(b"for i in a; do done", "expected ';' before 'done'");
}

#[test]
fn empty_condition_pins_the_body_message() {
    // Task #299: the empty **condition** is its own error and is checked before
    // the body, so this form now reports `missing condition`, not
    // `'then' body is empty`.
    body_err(b"if ; then; fi", "missing condition");
    // No closer at all keeps `missing 'fi'` (the heredoc pin's contract).
    body_err(b"if true; then", "missing 'fi'");
}

// --- empty block conditions (task #299) ------------------------------------
//
// POSIX #2.9 requires a non-empty `list` as an `if`/`elif`/`while`/`until`
// condition; bash 5.3.9 rejects every empty form with rc 2 (measured), and an
// empty condition is **true** at run time, so `while ; do …; done` is an
// unbounded loop. fdshell reports rc 1 with the keyword family's own message
// (`missing condition` for `if`/`elif`, `expected condition` for the loops).

#[test]
fn empty_if_condition_is_err() {
    body_err(b"if ; then :; fi", "missing condition");
    body_err(b"if ;; then :; fi", "missing condition");
    // A newline is a `;` token and a comment is dropped by the tokenizer, so
    // both leave an empty condition (bash rc 2 for both).
    body_err(b"if\nthen :; fi", "missing condition");
    body_err(b"if\n# c\nthen :; fi", "missing condition");
    // The `&`-only slice is raw-**non**-empty and trimmed-empty: the row that
    // pins the test to the trimmed slice (LESSONS #299).
    body_err(b"if & then :; fi", "missing condition");
    body_err(b"if ; & then :; fi", "missing condition");
    body_err(b"if ; then echo x; else echo y; fi", "missing condition");
}

#[test]
fn empty_elif_condition_is_err() {
    body_err(b"if a; then b; elif ; then c; fi", "missing condition");
    body_err(b"if a; then b; elif & then c; fi", "missing condition");
    // The `;;` slice keeps the older message: the plain-`;` clause check runs
    // first (message contract).
    body_err(b"if a; then b; elif ;; then c; fi", "malformed if block");
}

#[test]
fn empty_loop_condition_is_err() {
    body_err(b"while ; do :; done", "expected condition");
    body_err(b"while ;; do :; done", "expected condition");
    body_err(b"while\n# c\ndo :; done", "expected condition");
    body_err(b"while & do :; done", "expected condition");
    body_err(b"until ; do :; done", "expected condition");
    body_err(b"until & do :; done", "expected condition");
}

/// The shapes near the rule that bash accepts (or that fdshell's documented
/// leniency family already pins) must stay parseable: an empty **last part**
/// of a cond-list is not an empty condition, and a `for` word list and the
/// arith clause are not conditions at all.
#[test]
fn non_empty_conditions_stay_green() {
    assert!(parse(b"if\ntrue; then :; fi").is_ok());
    assert!(parse(b"if true & then :; fi").is_ok());
    assert!(parse(b"if true && & then :; fi").is_ok());
    assert!(parse(b"while : & do :; break; done").is_ok());
    // bash rc 0: an empty word list iterates zero times — not a divergence.
    assert!(parse(b"for i in; do echo a; done").is_ok());
    // README: the arith clause's empty condition is true (a different surface).
    assert!(parse(b"for ((;;)) do :; break; done").is_ok());
    assert!(parse(b"case x in esac").is_ok());
}

#[test]
fn non_empty_and_case_bodies_stay_green() {
    assert!(parse(b"if true; then echo a; fi").is_ok());
    assert!(parse(b"if true; then echo a; else echo b; fi").is_ok());
    assert!(parse(b"while false; do echo a; done").is_ok());
    assert!(parse(b"for i in a; do echo a; done").is_ok());
    // bash rc 0: an empty word list iterates zero times — not a divergence.
    assert!(parse(b"for i in; do echo a; done").is_ok());
    // bash accepts an empty `case` clause body (rc 0): the rule is not
    // applied to `case_block.rs`.
    assert!(parse(b"case \"x\" in a) ;; esac").is_ok());
    // A `;;` body line is a token range of separators only: after trimming
    // nothing is left, so task #233 rejects it (bash rc 2 too).
    body_err(b"if true; then\n;;\nfi", "'then' body is empty");
}

#[test]
fn if_condition_with_equals() {
    let ParsedLine::If(ib) = parse(b"if x=y; then z; fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"x=y".into());
    assert_eq!(ib.then_body.data, c"z".into());
}

#[test]
fn if_multi_word_then_body() {
    let ParsedLine::If(ib) = parse(b"if a; then b c; fi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.condition.data, c"a".into());
    assert_eq!(ib.then_body.data, c"b c".into());
}

/// A `;`-terminated heredoc in a single-line block **body**: the block text
/// carries the body regions after the closing keyword (append mode), so the
/// arm's text is rebuilt from the regions its own operators claim into a
/// self-contained heredoc (command bytes + newline + bodies).
#[test]
fn if_then_body_heredoc_of_a_single_line_block_is_self_contained() {
    let ParsedLine::If(ib) = parse(b"if true; then cat <<T; fi\nbodyT\nT").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.then_body.data, c"cat <<T\nbodyT\nT".into());
    assert!(ib.cond_bodies.is_empty());
}

/// The same for an `elif` arm's body and for the `else` body.
#[test]
fn if_elif_and_else_body_heredocs_are_self_contained() {
    let ParsedLine::If(ib) =
        parse(b"if false; then :; elif true; then cat <<E; fi\nbodyE\nE").unwrap()
    else {
        panic!("expected If")
    };
    assert_eq!(ib.elifs[0].body.data, c"cat <<E\nbodyE\nE".into());

    let ParsedLine::If(ib) = parse(b"if false; then :; else cat <<T; fi\nbodyT\nT").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.else_body.unwrap().data, c"cat <<T\nbodyT\nT".into());
}

/// Two heredoc operators in one arm body: both bodies are appended, in
/// operator order.
#[test]
fn if_then_body_with_two_heredocs_appends_both_bodies() {
    let ParsedLine::If(ib) =
        parse(b"if true; then cat <<A; cat <<B; fi\nbodyA\nA\nbodyB\nB").unwrap()
    else {
        panic!("expected If")
    };
    assert_eq!(
        ib.then_body.data,
        c"cat <<A; cat <<B\nbodyA\nA\nbodyB\nB".into()
    );
}

/// A single-line loop body's heredoc — `while` and `until` share the builder.
#[test]
fn loop_body_heredoc_of_a_single_line_block_is_self_contained() {
    let ParsedLine::While(wb) = parse(b"while true; do cat <<T; done\nbodyT\nT").unwrap() else {
        panic!("expected While")
    };
    assert_eq!(wb.body.data, c"cat <<T\nbodyT\nT".into());

    let ParsedLine::Until(ub) = parse(b"until false; do cat <<T; done\nbodyT\nT").unwrap() else {
        panic!("expected Until")
    };
    assert_eq!(ub.body.data, c"cat <<T\nbodyT\nT".into());
}

/// Task #273: the `for` body is the same arm-body shape — its text is rebuilt
/// from the regions its own operators claim, so it is self-contained too.
#[test]
fn for_body_heredoc_of_a_single_line_block_is_self_contained() {
    let ParsedLine::For(fb) = parse(b"for x in a; do cat <<T; done\nbodyT\nT").unwrap() else {
        panic!("expected For")
    };
    assert_eq!(fb.body.data, c"cat <<T\nbodyT\nT".into());
}

/// Task #273: a `case` clause body of a single-line `case`, whose bodies sit
/// past `esac`. Both arm shapes: `;;`-terminated and the last (bare) one, so
/// each `part_text` slice of `case_clause::extract::body` is pinned.
#[test]
fn case_clause_body_heredoc_of_a_single_line_block_is_self_contained() {
    let ParsedLine::Case(cb) = parse(b"case x in x) cat <<T; ;; esac\nbodyT\nT").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 1);
    assert_eq!(cb.clauses[0].body.data, c"cat <<T\nbodyT\nT".into());

    let ParsedLine::Case(cb) =
        parse(b"case x in x) cat <<T;; y) cat <<U;; esac\nbodyT\nT\nbodyU\nU").unwrap()
    else {
        panic!("expected Case")
    };
    // The first arm's region is not the text's last one, so it keeps its
    // terminating newline (`reconstruct` appends regions verbatim); the arm
    // body runs with that newline as a separator, and the second arm claims the
    // next region in the line's operator order.
    assert_eq!(cb.clauses[0].body.data, c"cat <<T\nbodyT\nT\n".into());
    assert_eq!(cb.clauses[1].body.data, c"cat <<U\nbodyU\nU".into());
}

/// A multi-line body-position heredoc keeps the verbatim arm text (the
/// delimiter line is an unblanked token inside the arm's token range): the
/// extension mode `part_text` must not rewrite. `if`/`while` were pinned by
/// task #238, `for`/`case` by task #273 — the same byte-identity requirement.
#[test]
fn if_then_body_heredoc_of_a_multi_line_block_stays_verbatim() {
    let ParsedLine::If(ib) = parse(b"if true; then cat <<T\nbodyT\nT\nfi").unwrap() else {
        panic!("expected If")
    };
    assert_eq!(ib.then_body.data, c"cat <<T\nbodyT\nT".into());
}

#[test]
fn for_body_heredoc_of_a_multi_line_block_stays_verbatim() {
    let ParsedLine::For(fb) = parse(b"for x in a; do\ncat <<T\nbodyT\nT\ndone").unwrap() else {
        panic!("expected For")
    };
    assert_eq!(fb.body.data, c"cat <<T\nbodyT\nT".into());
}

#[test]
fn case_clause_body_heredoc_of_a_multi_line_block_stays_verbatim() {
    let ParsedLine::Case(cb) = parse(b"case x in x) cat <<T\nbodyT\nT\n;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses[0].body.data, c"cat <<T\nbodyT\nT".into());
}

/// Task #273: the closing-keyword truncation must not swallow a missing
/// closer. With the `done` gone the depth walk never reaches 0, so the `for`
/// keeps reporting `expected 'done'` and never reaches the body build.
#[test]
fn for_heredoc_body_without_done_reports_expected_done() {
    let err = parse(b"for x in a; do cat <<T\nbodyT").err().unwrap();
    assert_eq!(err.current_context().to_string(), "expected 'done'");
}

/// The `case` twins: no closer at all, and a `done` written in a clause body.
/// The first keeps `case: missing 'esac'`. The second is a **successful parse**
/// since task #309: the closing-keyword walk is seeded with the `case`'s own
/// closer, so a `done` (a loop closer) mismatches, stays word data, and the
/// block reaches its real `esac` — the clause body keeps the `done` as its last
/// word. Running it is the command-not-found half (`done` is no command), rc 1
/// with the resolve error, pinned end-to-end by
/// `tests/heredoc.rs::case_body_heredoc_of_a_clause_terminated_by_done_runs_the_body`;
/// bash rejects the line rc 2 (the rc-1-vs-2 family). `master` stopped the walk
/// at the `done` and reported `missing 'esac'`.
#[test]
fn case_heredoc_body_without_esac_reports_missing_esac() {
    let err = parse(b"case x in x) cat <<T; ;;").err().unwrap();
    assert_eq!(err.current_context().to_string(), "case: missing 'esac'");
    let ParsedLine::Case(cb) = parse(b"case x in x) cat <<T; done;; esac\nbodyT\nT").unwrap()
    else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses[0].body.data, c"cat <<T; done\nbodyT\nT".into());
}

/// Task #236: `if a; then b fi` has no `;` before `fi` (task #301 owns the
/// missing separator), and the `fi` sits in the body's **argument** position
/// (behind the command word `b`), so it never scores as the closer: the `if`
/// reports `missing 'fi'` (was: the argument-`fi` closed the block and the
/// body range came out empty, `'then' body is empty`). bash rejects it too
/// (rc 2, task #233).
#[test]
fn if_body_before_a_glued_fi_is_a_missing_fi_err() {
    let err = parse(b"if a; then b fi").err().unwrap();
    assert_eq!(err.current_context().to_string(), "missing 'fi'");
}

#[test]
fn parse_elifs_empty_pairs() {
    use super::elif::parse_elifs;
    let tokens: Vec<Token> = vec![];
    let pairs: Vec<(usize, usize)> = vec![];
    let result = parse_elifs(&tokens, &pairs, None, 0, &stext(b""));
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn parse_elifs_single() {
    use super::elif::parse_elifs;
    // Tokens: elif, cond, ;, then, ;, body, ;, fi
    // Indices: 0,     1,    2, 3,     4, 5,    6, 7
    let line = b"elif cond; then; body; fi";
    let tokens = token::tokenize(line).unwrap();
    let pairs = vec![(0, 3)];
    let result = parse_elifs(&tokens, &pairs, None, 7, &stext(line));
    assert!(result.is_ok());
    let elifs = result.unwrap();
    assert_eq!(elifs.len(), 1);
    assert_eq!(elifs[0].cond.data, c"cond".into());
    assert_eq!(elifs[0].body.data, c"body".into());
}

#[test]
fn parse_elifs_multiple() {
    use super::elif::parse_elifs;
    // Tokens: elif, c1, ;, then, ;, b1, ;, elif, c2, ;, then, ;, b2, ;, fi
    // Indices: 0,     1,  2, 3,     4, 5,    6, 7,    8,    9, 10,     11, 12, 13, 14
    let line = b"elif c1; then; b1; elif c2; then; b2; fi";
    let tokens = token::tokenize(line).unwrap();
    let pairs = vec![(0, 3), (7, 10)];
    let result = parse_elifs(&tokens, &pairs, None, 14, &stext(line));
    assert!(result.is_ok());
    let elifs = result.unwrap();
    assert_eq!(elifs.len(), 2);
    assert_eq!(elifs[0].cond.data, c"c1".into());
    assert_eq!(elifs[0].body.data, c"b1".into());
    assert_eq!(elifs[1].cond.data, c"c2".into());
    assert_eq!(elifs[1].body.data, c"b2".into());
}

#[test]
fn parse_elifs_with_else() {
    use super::elif::parse_elifs;
    // Tokens: elif, cond, ;, then, ;, body, ;, else, else_body, ;, fi
    // Indices: 0,     1,    2, 3,     4, 5,    6, 7,    8,         9, 10
    let line = b"elif cond; then; body; else; else_body; fi";
    let tokens = token::tokenize(line).unwrap();
    let pairs = vec![(0, 3)];
    let result = parse_elifs(&tokens, &pairs, Some(7), 10, &stext(line));
    assert!(result.is_ok());
    let elifs = result.unwrap();
    assert_eq!(elifs.len(), 1);
    assert_eq!(elifs[0].cond.data, c"cond".into());
    assert_eq!(elifs[0].body.data, c"body".into());
}

#[test]
fn parse_else_body_simple() {
    use super::elif::parse_else_body;
    // Tokens: else, fallback, ;, fi
    // Indices: 0,      1,        2, 3
    let line = b"else fallback; fi";
    let tokens = token::tokenize(line).unwrap();
    let result = parse_else_body(&tokens, 0, 3, &stext(line)).unwrap();
    assert_eq!(result.data, c"fallback".into());
}

#[test]
fn parse_else_body_multiple_tokens() {
    use super::elif::parse_else_body;
    // Tokens: else, cmd1, ;, cmd2, ;, fi
    // Indices: 0,      1,    2, 3,    4, 5
    let line = b"else cmd1; cmd2; fi";
    let tokens = token::tokenize(line).unwrap();
    let result = parse_else_body(&tokens, 0, 5, &stext(line)).unwrap();
    assert_eq!(result.data, c"cmd1; cmd2".into());
}

#[test]
fn parse_elifs_missing_condition_err() {
    use super::elif::parse_elifs;
    // Tokens: elif, then, ; — no condition between elif and then
    // Indices: 0,     1,    2
    let line = b"elif then;";
    let tokens = token::tokenize(line).unwrap();
    let pairs = vec![(0, 1)];
    let result = parse_elifs(&tokens, &pairs, None, 2, &stext(line));
    assert!(result.is_err());
}

#[test]
fn parse_elifs_missing_body_err() {
    use super::elif::parse_elifs;
    // Tokens: elif, cond, ;, then — no body after then
    // Indices: 0,     1,    2, 3
    let line = b"elif cond; then";
    let tokens = token::tokenize(line).unwrap();
    let pairs = vec![(0, 3)];
    let result = parse_elifs(&tokens, &pairs, None, 3, &stext(line));
    assert!(result.is_err());
}

/// bash 5.3.9 rejects `else fi` with no body (rc 2, task #233).
#[test]
fn parse_else_body_empty() {
    use super::elif::parse_else_body;
    // Tokens: else, fi — no body between else and fi
    // Indices: 0,      1
    let line = b"else fi";
    let tokens = token::tokenize(line).unwrap();
    let err = parse_else_body(&tokens, 0, 1, &stext(line)).err().unwrap();
    assert_eq!(err.current_context().to_string(), "'else' body is empty");
}

#[test]
fn parse_elifs_condition_trailing_semi_err() {
    use super::elif::parse_elifs;
    // Tokens: elif, c, ;, ;, then, b, ;, fi
    // Indices: 0,     1, 2, 3, 4,    5, 6, 7
    // ti=4, cond range = 1..3 = [c, ;], last is ';' → MalformedIfBlock
    let line = b"elif c; ; then b; fi";
    let tokens = token::tokenize(line).unwrap();
    let pairs = vec![(0, 4)];
    let result = parse_elifs(&tokens, &pairs, None, 7, &stext(line));
    assert!(result.is_err());
}

#[test]
fn parse_elifs_body_trailing_semi_err() {
    use super::elif::parse_elifs;
    // Tokens: elif, c, ;, then, b, ;, ;, fi
    // Indices: 0,     1, 2, 3,    4, 5, 6, 7
    // next=7, body range = 4..6 = [b, ;], last is ';' → MalformedIfBlock
    let line = b"elif c; then b; ; fi";
    let tokens = token::tokenize(line).unwrap();
    let pairs = vec![(0, 3)];
    let result = parse_elifs(&tokens, &pairs, None, 7, &stext(line));
    assert!(result.is_err());
}

#[test]
fn parse_else_body_trailing_semi_err() {
    use super::elif::parse_else_body;
    // Tokens: else, b, ;, ;, fi
    // Indices: 0,    1, 2, 3, 4
    // fi_idx=4, raw range = 1..3 = [b, ;], last is ';' → MalformedIfBlock
    let line = b"else b; ; fi";
    let tokens = token::tokenize(line).unwrap();
    let result = parse_else_body(&tokens, 0, 4, &stext(line));
    assert!(result.is_err());
}

#[test]
fn if_else_trailing_semi_err() {
    // End-to-end: double ';' before 'fi' routes through tokens_to_if → else_str?
    let result = parse(b"if a; then b; else c; ; fi");
    assert!(result.is_err());
    match result {
        Err(e) => assert_eq!(
            e.current_context().to_string(),
            "malformed if block",
            "expected MalformedIfBlock error"
        ),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn case_simple() {
    let ParsedLine::Case(cb) = parse(b"case \"foo\" in foo) echo one;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.word, c"foo".into());
    assert_eq!(cb.clauses.len(), 1);
    assert_eq!(cb.clauses[0].patterns.len(), 1);
    assert_eq!(cb.clauses[0].patterns[0].text, c"foo".into());
    assert_eq!(cb.clauses[0].body.data, c"echo one".into());
}

#[test]
fn case_word_carries_its_quote_mask() {
    // The word's mask travels to `case_exec` with the word (task #269), so the
    // quoted escape pair is folded by the quoted arm and keeps its backslash.
    let ParsedLine::Case(cb) = parse(b"case \"a\\ b\" in \"a b\") echo m;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.word, c"a\\ b".into());
    assert_eq!(cb.word_mask, runs(&[(true, 4)]));
}

#[test]
fn case_multiple_clauses() {
    let ParsedLine::Case(cb) = parse(b"case \"x\" in a) echo one;; b) echo two;; esac").unwrap()
    else {
        panic!("expected Case")
    };
    assert_eq!(cb.word, c"x".into());
    assert_eq!(cb.clauses.len(), 2);
    assert_eq!(cb.clauses[0].patterns[0].text, c"a".into());
    assert_eq!(cb.clauses[0].body.data, c"echo one".into());
    assert_eq!(cb.clauses[1].patterns[0].text, c"b".into());
    assert_eq!(cb.clauses[1].body.data, c"echo two".into());
}

#[test]
fn case_alternative_patterns() {
    let ParsedLine::Case(cb) = parse(b"case \"x\" in a|x) echo yes;; *) echo no;; esac").unwrap()
    else {
        panic!("expected Case")
    };
    assert_eq!(cb.word, c"x".into());
    assert_eq!(cb.clauses.len(), 2);
    assert_eq!(cb.clauses[0].patterns.len(), 2);
    assert_eq!(cb.clauses[0].patterns[0].text, c"a".into());
    assert_eq!(cb.clauses[0].patterns[1].text, c"x".into());
    assert_eq!(cb.clauses[1].patterns[0].text, c"*".into());
}

/// The pattern side of task #269's word rule (task #280): each pattern travels
/// with a mask parallel to its text, and the escape pair stays in the text —
/// who wrote it is the mask's answer, and it decides how the pair folds.
#[test]
fn case_pattern_carries_its_quote_mask() {
    let ParsedLine::Case(cb) = parse(b"case x in \"a\\ b\") echo m;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses[0].patterns[0].text, c"a\\ b".into());
    assert_eq!(cb.clauses[0].patterns[0].mask, runs(&[(true, 4)]));

    // Unquoted, the same 4-byte text means a foldable escape pair.
    let ParsedLine::Case(cb) = parse(b"case x in a\\ b) echo m;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses[0].patterns[0].text, c"a\\ b".into());
    assert_eq!(cb.clauses[0].patterns[0].mask, vec![Unquoted; 4]);
}

#[test]
fn case_pattern_pair_masks_differ() {
    // The anti-off-by-one pair of one clause's two `|`-separated patterns
    // (LESSONS: a same-quoting pair cannot tell an off-by-one from an
    // off-by-two): the two masks differ, so a shifted mask slice misreads one
    // of them and the pair cannot both agree.
    let ParsedLine::Case(cb) = parse(b"case x in \"a?\"|a?) echo m;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses[0].patterns.len(), 2);
    assert_eq!(cb.clauses[0].patterns[0].text, c"a?".into());
    assert_eq!(cb.clauses[0].patterns[0].mask, runs(&[(true, 2)]));
    assert_eq!(cb.clauses[0].patterns[1].text, c"a?".into());
    assert_eq!(cb.clauses[0].patterns[1].mask, vec![Unquoted; 2]);
}

/// fdshell joins a multi-token pattern with a space where bash rejects the shape
/// (`case "a b" in a b)` is a bash syntax error) — a parse-layer divergence,
/// pinned here as "fdshell accepts". The inserted space takes a `false` mask
/// bit, so it stays a literal space byte and never becomes a pattern char.
#[test]
fn case_pattern_join_marks_its_inserted_space_unquoted() {
    let ParsedLine::Case(cb) = parse(b"case \"a b\" in a b) echo m;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses[0].patterns[0].text, c"a b".into());
    assert_eq!(cb.clauses[0].patterns[0].mask, vec![Unquoted; 3]);
}

#[test]
fn case_multiline() {
    let ParsedLine::Case(cb) = parse(
        b"case \"foo\"
in
foo)
echo one
;;
*)
echo other
;;
esac",
    )
    .unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.word, c"foo".into());
    assert_eq!(cb.clauses.len(), 2);
    assert_eq!(cb.clauses[0].body.data, c"echo one".into());
    assert_eq!(cb.clauses[1].body.data, c"echo other".into());
}

#[test]
fn case_missing_in_returns_err() {
    let result = parse(b"case \"foo\" foo) echo one;; esac");
    match result {
        Ok(_) => panic!("expected error"),
        Err(e) => {
            assert_eq!(e.current_context().to_string(), "case: missing 'in'");
        }
    }
}

#[test]
fn case_missing_esac_returns_err() {
    let result = parse(b"case \"foo\" in foo) echo one;;");
    match result {
        Ok(_) => panic!("expected error"),
        Err(e) => {
            assert_eq!(e.current_context().to_string(), "case: missing 'esac'");
        }
    }
}

#[test]
fn case_empty_pattern_returns_err() {
    let result = parse(b"case \"x\" in |) echo yes;; esac");
    match result {
        Ok(_) => panic!("expected error"),
        Err(e) => {
            assert_eq!(e.current_context().to_string(), "case: empty pattern");
        }
    }
}

#[test]
fn case_missing_close_paren_returns_err() {
    let result = parse(b"case \"x\" in echo yes;; esac");
    match result {
        Ok(_) => panic!("expected error"),
        Err(e) => {
            assert_eq!(e.current_context().to_string(), "case: missing ')'");
        }
    }
}

#[test]
fn case_last_clause_no_semi_semi() {
    let ParsedLine::Case(cb) = parse(b"case \"x\" in a) echo one;; *) echo two esac").unwrap()
    else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 2);
    assert_eq!(cb.clauses[0].body.data, c"echo one".into());
    assert_eq!(cb.clauses[1].body.data, c"echo two".into());
}

/// `up_to_esac`'s leniency (task #273) covers the `;;`-less last clause of a
/// heredoc-free block only: a clause that carries a body heredoc has its body
/// tokens *after* the `esac`, so the fallback's "last token is `esac`" check
/// fails and the shape stays the block's own missing-`esac` rejection (bash
/// reports a syntax error, rc 2; fdshell rc 1 on `master` and here — the
/// integration path reports the generic parse error of the unclosed block span,
/// `tests/heredoc.rs::case_body_heredoc_of_a_clause_terminated_by_done_reports_missing_esac`
/// pins the family's message shape).
#[test]
fn case_last_clause_body_heredoc_of_a_late_esac_is_not_lenient() {
    assert!(parse(b"case x in x) cat <<T esac\nbodyT\nT").is_err());
}

#[test]
fn case_empty_body_clause() {
    let ParsedLine::Case(cb) = parse(b"case \"x\" in a) ;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 1);
    assert_eq!(cb.clauses[0].patterns[0].text, c"a".into());
    assert!(cb.clauses[0].body.data.is_empty());
}

/// Task #300: the same empty body, written **without** the `;;` — a clause whose
/// `)` is the token right before `esac` is a clause closed at `esac`, not a
/// clause missing its paren (bash runs `case x in a) esac` at rc 0). The
/// positional test that used to fire here read `pos == esac_idx` right after a
/// successful `)` as the missing-paren shape.
#[test]
fn case_clause_closed_by_esac_has_an_empty_body() {
    let ParsedLine::Case(cb) = parse(b"case \"x\" in a) esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 1);
    assert_eq!(cb.clauses[0].patterns[0].text, c"a".into());
    assert!(cb.clauses[0].body.data.is_empty());
    // The multi-line spelling, whose newline becomes a `;` token and already
    // landed one short of `esac` (the asymmetry the bug turned on).
    let ParsedLine::Case(cb) = parse(b"case \"x\" in\na)\nesac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 1);
    assert!(cb.clauses[0].body.data.is_empty());
}

/// The clause-separation sibling of task #300: the next clause's pattern paren
/// written inside an arm is bash's `syntax error near unexpected token ')'`
/// (rc 2) and fdshell's own message, at its rc 1.
#[test]
fn case_clauses_without_a_semi_report_missing_semi() {
    for line in [
        b"case \"x\" in a) b) ;; esac".as_slice(),
        b"case \"x\" in a) b) esac",
        b"case x in a) *) esac",
        b"case x in a) echo hi; b) ;; esac",
    ] {
        let err = parse(line)
            .err()
            .unwrap_or_else(|| panic!("{line:?} is ok"));
        assert_eq!(
            err.current_context().to_string(),
            "case: missing ';;' between clauses",
            "{line:?}"
        );
    }
}

#[test]
fn case_long_body_catches_skip() {
    let ParsedLine::Case(cb) =
        parse(b"case \"x\" in a) echo hello world foo bar baz ;; b) echo two;; esac").unwrap()
    else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 2);
    assert_eq!(cb.clauses[0].patterns[0].text, c"a".into());
    assert_eq!(
        cb.clauses[0].body.data,
        c"echo hello world foo bar baz".into()
    );
    assert_eq!(cb.clauses[1].patterns[0].text, c"b".into());
    assert_eq!(cb.clauses[1].body.data, c"echo two".into());
}

#[test]
fn case_esac_as_pattern_word() {
    let ParsedLine::Case(cb) = parse(b"case \"x\" in esac) echo yes;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 1);
    assert_eq!(cb.clauses[0].patterns[0].text, c"esac".into());
    assert_eq!(cb.clauses[0].body.data, c"echo yes".into());
}

#[test]
fn case_leading_semi_skipped() {
    let ParsedLine::Case(cb) = parse(b"case \"x\" in ; a) echo one;; esac").unwrap() else {
        panic!("expected Case")
    };
    assert_eq!(cb.clauses.len(), 1);
    assert_eq!(cb.clauses[0].patterns[0].text, c"a".into());
    assert_eq!(cb.clauses[0].body.data, c"echo one".into());
}

#[test]
fn tokenize_pipe_after_gt_token() {
    let tokens = token::tokenize(b"foo>bar | cmd").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].0, c"foo>bar".into());
    assert_eq!(tokens[1].0, c"|".into());
    assert_eq!(tokens[2].0, c"cmd".into());
}

#[test]
fn tokenize_pipe_position() {
    let tokens = token::tokenize(b"a|b").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].0, c"a".into());
    assert_eq!(tokens[0].1, 0);
    assert_eq!(tokens[1].0, c"|".into());
    assert_eq!(tokens[1].1, 1);
    assert_eq!(tokens[2].0, c"b".into());
    assert_eq!(tokens[2].1, 2, "post-pipe word starts after the pipe");
    assert_eq!(tokens[0].2, 1, "pre-pipe token ends at the '|'");
    assert_eq!(tokens[1].2, 2, "pipe token end");
    assert_eq!(tokens[2].2, 3, "final token end");
}

#[test]
fn tokenize_caret_pipe_position() {
    let tokens = token::tokenize(b"%>%fd|cmd").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].0, c"%>%fd".into());
    assert_eq!(tokens[1].0, c"|".into());
    assert_eq!(tokens[1].1, 5);
    assert_eq!(tokens[2].0, c"cmd".into());
    assert_eq!(tokens[2].1, 6, "post-pipe word starts after the pipe");
}

#[test]
fn tokenize_close_paren_position() {
    let tokens = token::tokenize(b"case x in a) echo one;; esac").unwrap();
    let paren_token = tokens
        .iter()
        .find(|(t, _, _, _, _, _)| t.eq_bytes(b")"))
        .unwrap();
    assert_eq!(paren_token.0, c")".into());
    assert_eq!(paren_token.1, 11);
}

#[test]
fn tokenize_comment_shifts_positions() {
    let tokens = token::tokenize(b"hello # comment\nworld").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].0, c"hello".into());
    assert_eq!(tokens[1].0, c"world".into());
}

#[test]
fn tokenize_comment_no_newline_shifts_positions() {
    let tokens = token::tokenize(b"hello # trailing comment").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"hello".into());
}

#[test]
fn tokenize_comment_with_trailing_token_position() {
    let tokens = token::tokenize(b"a # comment\nb").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].0, c"a".into());
    assert_eq!(tokens[0].1, 0);
    assert_eq!(tokens[1].0, c"b".into());
    assert_eq!(tokens[1].1, 12);
}

#[test]
fn tokenize_token_ending_before_comment() {
    let tokens = token::tokenize(b"ab # c").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"ab".into());
    assert_eq!(tokens[0].1, 0);
    assert_eq!(
        tokens[0].2, 2,
        "end must be at the end of the word, before the comment"
    );
}

#[test]
fn tokenize_hash_mid_word_is_literal() {
    // `#` inside a word is data, not a comment.
    let tokens = token::tokenize(b"ab# c").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].0, c"ab#".into());
    assert_eq!(tokens[1].0, c"c".into());
}

#[test]
fn tokenize_empty_quote_then_hash_kept() {
    // A closing quote still counts as word content, so a `#` right after it is
    // literal even though the accumulated bytes are empty.
    let tokens = token::tokenize(b"\"\"#x").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"#x".into());
    assert!(!tokens[0].3, "#x is not fully quoted");
}

#[test]
fn tokenize_unbalanced_quote_error_position() {
    let result = token::tokenize(b"\"unclosed");
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(
        pos, 0,
        "unbalanced quote should be at position 0 (opening quote)"
    );
}

#[test]
fn tokenize_unbalanced_dollar_paren_error_position() {
    let result = token::tokenize(b"$(unclosed");
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos, 0, "unbalanced $() should be at position 0");
}

#[test]
fn tokenize_unbalanced_backtick_error_position() {
    let result = token::tokenize(b"`unclosed");
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos, 0, "unbalanced backtick should be at position 0");
}

#[test]
fn tokenize_amp_pipe_valid() {
    let tokens = token::tokenize(b"&|").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].0, c"&".into());
    assert_eq!(tokens[1].0, c"|".into());
}

#[test]
fn tokenize_percent_pipe_valid() {
    let tokens = token::tokenize(b"%|").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].0, c"%".into());
    assert_eq!(tokens[1].0, c"|".into());
}

#[test]
fn tokenize_fully_quoted_dollar_at_end() {
    let tokens = token::tokenize(b"printf \"%s\\n\" \"$@\"").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].0, c"printf".into());
    assert!(!tokens[0].3);
    assert_eq!(tokens[1].0, c"%s\\n".into());
    assert!(tokens[1].3);
    assert_eq!(tokens[2].0, c"$@".into());
    assert!(tokens[2].3);
}

#[test]
fn tokenize_fully_quoted_dollar_at_middle() {
    let tokens = token::tokenize(b"printf \"%s\\n\" \"$@\" extra").unwrap();
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0].0, c"printf".into());
    assert!(!tokens[0].3);
    assert_eq!(tokens[1].0, c"%s\\n".into());
    assert!(tokens[1].3);
    assert_eq!(tokens[2].0, c"$@".into());
    assert!(tokens[2].3);
    assert_eq!(tokens[3].0, c"extra".into());
    assert!(!tokens[3].3);
}

#[test]
fn tokenize_fully_quoted_dollar_star() {
    let tokens = token::tokenize(b"$*").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"$*".into());
    assert!(!tokens[0].3);
}

#[test]
fn tokenize_fully_quoted_literal() {
    let tokens = token::tokenize(b"\"hello\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"hello".into());
    assert!(tokens[0].3);
}

#[test]
fn tokenize_unquoted_prefix_not_fully_quoted() {
    let tokens = token::tokenize(b"foo\"bar\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"foobar".into());
    assert!(!tokens[0].3);
}

#[test]
fn tokenize_mixed_quoted_unquoted() {
    let tokens = token::tokenize(b"\"$@\" \"$*\" literal").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].0, c"$@".into());
    assert!(tokens[0].3);
    assert_eq!(tokens[1].0, c"$*".into());
    assert!(tokens[1].3);
    assert_eq!(tokens[2].0, c"literal".into());
    assert!(!tokens[2].3);
}

#[test]
fn tokenize_fully_quoted_with_separator() {
    let tokens = token::tokenize(b"\"$@\";").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].0, c"$@".into());
    assert!(tokens[0].3);
    assert_eq!(tokens[1].0, c";".into());
    assert!(!tokens[1].3);
}

#[test]
fn tokenize_fully_quoted_with_pipe() {
    let tokens = token::tokenize(b"\"$@\" | cat").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].0, c"$@".into());
    assert!(tokens[0].3);
    assert_eq!(tokens[1].0, c"|".into());
    assert!(!tokens[1].3);
    assert_eq!(tokens[2].0, c"cat".into());
    assert!(!tokens[2].3);
}

#[test]
fn tokenize_adjacent_quotes_still_fully_quoted() {
    let tokens = token::tokenize(b"\"a\"\"b\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"ab".into());
    assert!(tokens[0].3);
}

#[test]
fn tokenize_dollar_paren_not_fully_quoted() {
    let tokens = token::tokenize(b"\"x\"$(y)").unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(!tokens[0].3);
}

#[test]
fn tokenize_empty_adjacent_quotes() {
    // Adjacent empty quotes are one quoted empty word, not nothing.
    let tokens = token::tokenize(b"\"\"\"\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0].0.is_empty());
    assert!(tokens[0].3);
    assert!(tokens[0].4.is_empty());
}

#[test]
fn tokenize_empty_quotes_are_one_empty_token() {
    let tokens = token::tokenize(b"\"\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0].0.is_empty());
    assert!(tokens[0].3, "a bare `\"\"` word is fully quoted");
    assert!(tokens[0].4.is_empty());
}

#[test]
fn tokenize_empty_quote_then_word() {
    let tokens = token::tokenize(b"\"\" x").unwrap();
    assert_eq!(tokens.len(), 2);
    assert!(tokens[0].0.is_empty());
    assert!(tokens[0].3);
    assert!(tokens[0].4.is_empty());
    assert_eq!(tokens[1].0, c"x".into());
    assert_eq!(tokens[1].4, vec![Unquoted]);
}

#[test]
fn tokenize_empty_quote_before_separators() {
    // The flag resets at separators, so a later unquoted word is unaffected.
    let tokens = token::tokenize(b"\"\"; x").unwrap();
    assert_eq!(tokens.len(), 3);
    assert!(tokens[0].0.is_empty());
    assert!(tokens[0].3);
    assert_eq!(tokens[1].0, c";".into());
    assert_eq!(tokens[2].0, c"x".into());
    assert!(!tokens[2].3);
}

#[test]
fn tokenize_dollar_paren_alone_stays_one_token() {
    // Unchanged: a `$()` word is literal text at tokenization time.
    let tokens = token::tokenize(b"$(true)").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"$(true)".into());
    assert!(!tokens[0].3);
}

#[test]
fn tokenize_quoted_dollar_paren_is_all_true_mask() {
    // Unchanged: the quotes protect every byte, mask is all-true.
    let tokens = token::tokenize(b"\"$(true)\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, c"$(true)".into());
    assert!(tokens[0].3);
    assert_eq!(tokens[0].4, runs(&[(true, 7)]));
}

#[test]
fn parse_empty_quoted_arg_kept_with_empty_mask() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd \"\" x").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.command, c"cmd".into());
    assert_eq!(cmd.args, vec![c"".into(), c"x".into()]);
    assert_eq!(cmd.args_mask, vec![vec![], vec![Unquoted]]);
    assert_eq!(cmd.args_quoted, vec![true, false]);
}

#[test]
fn parse_mixed_quoting_args_quoted_flags_stay_aligned() {
    // `a ""$(true) b ""`: the flag vector must map to the right words — a
    // mixed list pins the offset (an off-by-one would shift every flag).
    let ParsedLine::Cmd(cmd) = parse(b"cmd a \"\"$(true) b \"\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args.len(), 4);
    assert_eq!(cmd.args_quoted, vec![false, true, false, true]);
}

#[test]
fn parse_empty_quoted_command_is_empty_command() {
    // A leading `""` is an empty command word, not a silent skip.
    let ParsedLine::Cmd(cmd) = parse(b"\"\" true").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.command.is_empty());
    assert_eq!(cmd.args, vec![c"true".into()]);
}

#[test]
fn parse_here_string_empty_quoted_word() {
    let ParsedLine::Cmd(cmd) = parse(b"cat <<<\"\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(0, c"", Vec::new(), Vec::new())]
    );
    assert!(cmd.args.is_empty());
}

#[test]
fn parse_here_string_empty_quoted_word_keeps_later_args() {
    // `""` is the here-string word; `b` stays an argument.
    let ParsedLine::Cmd(cmd) = parse(b"echo a <<<\"\" b").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.redirects,
        vec![RedirectDef::here_string(0, c"", Vec::new(), Vec::new())]
    );
    assert_eq!(cmd.args, vec![c"a".into(), c"b".into()]);
}

#[test]
fn tokenize_mixed_quotes_mark_quoted_bytes() {
    // `x"a b"c`: the quote boundaries are erased from the text but the mask
    // keeps them — those bytes must survive IFS splitting intact.
    let tokens = token::tokenize(b"x\"a b\"c").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.as_bytes().unwrap(), b"xa bc");
    assert!(!tokens[0].3);
    assert_eq!(tokens[0].4, runs(&[(false, 1), (true, 3), (false, 1)]));
}

#[test]
fn tokenize_fully_quoted_word_has_all_true_mask() {
    let tokens = token::tokenize(b"\"a b\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0].3);
    assert_eq!(tokens[0].4, runs(&[(true, 3)]));
}

#[test]
fn tokenize_unquoted_word_has_all_false_mask() {
    let tokens = token::tokenize(b"abc").unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(!tokens[0].3);
    assert_eq!(tokens[0].4, vec![Unquoted; 3]);
}

#[test]
fn parse_fully_quoted_dollar_at_preserves_flag() {
    // Regression: end-to-end test that "$@" through parse → CommandLine gets fq=true
    let ParsedLine::Cmd(cmd) = parse(b"printf \"%s\\n\" \"$@\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args.len(), 2);
    assert_eq!(cmd.args[0].as_bytes().unwrap(), b"%s\\n");
    assert_eq!(cmd.args[1].as_bytes().unwrap(), b"$@");
    assert_eq!(cmd.args_mask.len(), 2);
    assert!(cmd.args_mask[0].iter().copied().all(MaskBit::quoted));
    assert!(cmd.args_mask[1].iter().copied().all(MaskBit::quoted));
    assert_eq!(cmd.args_quoted, vec![true, true]);
}

#[test]
fn parse_fully_quoted_dollar_at_middle_preserves_flag() {
    // Regression: "$@" followed by another arg should keep fq=true on "$@"
    let ParsedLine::Cmd(cmd) = parse(b"printf \"%s\\n\" \"$@\" extra").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args.len(), 3);
    assert_eq!(cmd.args[0].as_bytes().unwrap(), b"%s\\n");
    assert_eq!(cmd.args[1].as_bytes().unwrap(), b"$@");
    assert_eq!(cmd.args[2].as_bytes().unwrap(), b"extra");
    assert_eq!(cmd.args_mask.len(), 3);
    assert!(cmd.args_mask[0].iter().copied().all(MaskBit::quoted));
    assert!(cmd.args_mask[1].iter().copied().all(MaskBit::quoted));
    assert!(cmd.args_mask[2].iter().copied().all(|b| !b.quoted()));
    assert_eq!(cmd.args_quoted, vec![true, true, false]);
}

#[test]
fn parse_fully_quoted_dollar_star_preserves_flag() {
    let ParsedLine::Cmd(cmd) = parse(b"echo \"$*\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args.len(), 1);
    assert_eq!(cmd.args[0].as_bytes().unwrap(), b"$*");
    assert_eq!(cmd.args_mask.len(), 1);
    assert!(cmd.args_mask[0].iter().copied().all(MaskBit::quoted));
    assert_eq!(cmd.args_quoted, vec![true]);
}

#[test]
fn parse_unquoted_dollar_at_has_false_flag() {
    let ParsedLine::Cmd(cmd) = parse(b"echo $@").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args.len(), 1);
    assert_eq!(cmd.args[0].as_bytes().unwrap(), b"$@");
    assert_eq!(cmd.args_mask.len(), 1);
    assert!(cmd.args_mask[0].iter().copied().all(|b| !b.quoted()));
    assert_eq!(cmd.args_quoted, vec![false]);
}

#[test]
fn parse_fully_quoted_literal_preserves_flag() {
    let ParsedLine::Cmd(cmd) = parse(b"echo \"hello world\"").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args.len(), 1);
    assert_eq!(cmd.args[0].as_bytes().unwrap(), b"hello world");
    assert_eq!(cmd.args_mask.len(), 1);
    assert!(cmd.args_mask[0].iter().copied().all(MaskBit::quoted));
    assert_eq!(cmd.args_quoted, vec![true]);
}

#[test]
fn parse_mixed_quoted_args_preserves_flags() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd \"$@\" \"$*\" literal").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.args.len(), 3);
    assert_eq!(cmd.args_mask.len(), 3);
    assert!(cmd.args_mask[0].iter().copied().all(MaskBit::quoted));
    assert!(cmd.args_mask[1].iter().copied().all(MaskBit::quoted));
    assert!(cmd.args_mask[2].iter().copied().all(|b| !b.quoted()));
    assert_eq!(cmd.args_quoted, vec![true, true, false]);
}

#[test]
fn amp_after_gt_is_dup_redirect() {
    let ParsedLine::Cmd(cmd) = parse(b"builtin echo 2>&1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.redirects, vec![RedirectDef::dup(2, 1)]);
    assert!(cmd.args.is_empty());
}

#[test]
fn parse_capture_empty_var_is_error() {
    let s = sys::ShortCStr::from(c"%>%");
    assert!(super::capture::parse_capture(&s, pos()).is_err());
}

#[test]
fn parse_capture_tagged_empty_var_is_error() {
    let s = sys::ShortCStr::from(c"%tag>%");
    assert!(super::capture::parse_capture(&s, pos()).is_err());
}

#[test]
fn parse_capture_unbounded_has_no_cap() {
    let s = sys::ShortCStr::from(c"%>%arr");
    let c = super::capture::parse_capture(&s, pos()).unwrap().unwrap();
    assert_eq!(c.var, c"arr".into());
    assert_eq!(c.cap, None);
}

#[test]
fn parse_capture_bounded() {
    let s = sys::ShortCStr::from(c"%>%arr[2]");
    let c = super::capture::parse_capture(&s, pos()).unwrap().unwrap();
    assert_eq!(c.var, c"arr".into());
    assert_eq!(c.cap, Some(2));
    assert_eq!(c.tag, None);
}

#[test]
fn parse_capture_tagged_bounded() {
    let s = sys::ShortCStr::from(c"%tag>%arr[3]");
    let c = super::capture::parse_capture(&s, pos()).unwrap().unwrap();
    assert_eq!(c.var, c"arr".into());
    assert_eq!(c.cap, Some(3));
    assert_eq!(c.tag, Some(c"tag".into()));
}

#[test]
fn parse_capture_malformed_index_is_plain_name() {
    let s = sys::ShortCStr::from(c"%>%arr[2x]");
    let c = super::capture::parse_capture(&s, pos()).unwrap().unwrap();
    assert_eq!(c.var, c"arr[2x]".into());
    assert_eq!(c.cap, None);
}

#[test]
fn find_preceded_by_semi_start_zero_no_match() {
    let line = b"echo hi";
    let tokens = [
        (c"echo".into(), 0, 4, false, vec![], vec![]),
        (c"hi".into(), 5, 7, false, vec![], vec![]),
    ];
    assert_eq!(
        super::semi::find_preceded_by(&tokens, 0, b"echo", line, super::semi::Term::Semi),
        None
    );
}

#[test]
fn find_preceded_by_semi_skips_to_start() {
    let line = b";done;done";
    let tokens = [
        (c";".into(), 0, 1, false, vec![], vec![]),
        (c"done".into(), 1, 5, false, vec![], vec![]),
        (c";".into(), 6, 7, false, vec![], vec![]),
        (c"done".into(), 7, 11, false, vec![], vec![]),
    ];
    assert_eq!(
        super::semi::find_preceded_by(&tokens, 2, b"done", line, super::semi::Term::Semi),
        Some(3)
    );
}

// --- `((expr))` arithmetic command (keyword) ---

/// The raw expression body of an `ArithCommand` parse, or `None`.
fn arith_body(line: &[u8]) -> Option<Vec<u8>> {
    let ParsedLine::ArithCommand(body) = parse(line).unwrap() else {
        return None;
    };
    Some(body.as_bytes().unwrap().to_vec())
}

#[test]
fn arith_command_detects_body() {
    assert_eq!(arith_body(b"((1+2))"), Some(b"1+2".to_vec()));
}

#[test]
fn arith_command_keeps_inner_spacing() {
    assert_eq!(arith_body(b"(( 1 + 2 ))"), Some(b" 1 + 2 ".to_vec()));
}

#[test]
fn arith_command_skips_leading_whitespace() {
    assert_eq!(arith_body(b"  ((1+2))"), Some(b"1+2".to_vec()));
}

#[test]
fn arith_command_trailing_words_is_malformed() {
    assert!(parse(b"((x)) foo").is_err());
}

#[test]
fn arith_command_unbalanced_is_malformed() {
    assert!(parse(b"((x").is_err());
    assert!(parse(b"((").is_err());
    assert!(parse(b"((x)").is_err());
}

#[test]
fn arith_command_redirect_is_malformed() {
    assert!(parse(b"((x)) >%f").is_err());
}

#[test]
fn arith_command_keyword_wins_over_assignment() {
    // A statement starting with `((` is always the keyword (README), so
    // `((x)=1` is a malformed arith command, not an assignment.
    assert!(parse(b"((x)=1").is_err());
}

#[test]
fn echo_double_paren_is_plain_command() {
    // A statement that contains `((…))` but does not *start* with `((` is an
    // ordinary command (the arith keyword only fires on a leading `((`). The
    // tokenizer still splits the `)` bytes into separators, as for any word.
    let ParsedLine::Cmd(cmd) = parse(b"echo ((x))").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.command, c"echo".into());
}

#[test]
fn test_env_assign_prefix_on_command() {
    let ParsedLine::Cmd(cmd) = parse(b"FOO=bar cmd arg").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.prefix, BuiltinPrefix::None);
    assert_eq!(cmd.command, c"cmd".into());
    assert_eq!(cmd.env_assigns, vec![assign("FOO", "bar", false)]);
    assert_eq!(cmd.args, vec![c"arg".into()]);
}

#[test]
fn test_env_assign_prefix_builtin_detection_runs_on_command_word() {
    // Builtin recognition runs on the command word after the prefix, not on
    // the first (assignment) word: `FOO=bar printf hi` is a builtin command.
    let ParsedLine::Cmd(cmd) = parse(b"FOO=bar printf hi").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.prefix, BuiltinPrefix::Builtin);
    assert_eq!(cmd.command, c"printf".into());
    assert_eq!(cmd.env_assigns, vec![assign("FOO", "bar", false)]);
    assert_eq!(cmd.args, vec![c"hi".into()]);
}

#[test]
fn test_env_assign_compound_prefix() {
    let ParsedLine::Cmd(cmd) = parse(b"FOO=bar BAZ=qux cmd").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(
        cmd.env_assigns,
        vec![assign("FOO", "bar", false), assign("BAZ", "qux", false)]
    );
    assert_eq!(cmd.command, c"cmd".into());
}

#[test]
fn test_env_assign_bare_statement_is_assign_strs() {
    let ParsedLine::AssignStrs(assigns) = parse(b"FOO=bar BAZ=qux").unwrap() else {
        panic!("expected AssignStrs")
    };
    assert_eq!(
        assigns,
        vec![assign("FOO", "bar", false), assign("BAZ", "qux", false)]
    );
}

#[test]
fn test_env_assign_single_bare_statement_is_assign_str() {
    let ParsedLine::AssignStr(a) = parse(b"FOO=bar").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(a.name, c"FOO".into());
    assert_eq!(a.value, c"bar".into());
    assert_eq!(a.mask, vec![Unquoted; 3]);
}

#[test]
fn test_env_assign_bare_before_semicolon_token_is_assign_str() {
    // A raw `;` token (unit-parse level; the statement splitter removes it
    // in the production flow): the assignment word before it stays a bare
    // assignment.
    let ParsedLine::AssignStr(a) = parse(b"FOO=bar ;").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(a.name, c"FOO".into());
    assert_eq!(a.value, c"bar".into());
    assert_eq!(a.mask, vec![Unquoted; 3]);
}

#[test]
fn test_env_assign_prefix_bare_value() {
    let ParsedLine::Cmd(cmd) = parse(b"FOO= cmd").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.env_assigns, vec![assign("FOO", "", false)]);
    assert_eq!(cmd.command, c"cmd".into());
}

#[test]
fn test_env_assign_quoted_prefix_is_scoped() {
    let ParsedLine::Cmd(cmd) = parse(b"\"FOO=bar\" cmd").unwrap() else {
        panic!("expected Cmd")
    };
    // The whole word is quoted, so the value's mask is all-`true`: the value
    // is expanded as quoted text (task #217).
    assert_eq!(cmd.env_assigns, vec![assign("FOO", "bar", true)]);
    assert_eq!(cmd.command, c"cmd".into());
}

#[test]
fn test_env_assign_prefix_before_pipeline_is_parse_error() {
    // Divergence from bash: a scoped prefix cannot head a pipeline stage;
    // `|` after an assignment word leaves no command word for the prefix.
    assert!(parse(b"FOO=bar | cat").is_err());
}

#[test]
fn test_env_assign_prefix_before_keyword_runs_keyword_as_command() {
    // Divergence from bash: `FOO=bar if …` treats `if` as a plain command
    // word, so the prefix scopes onto it.
    let ParsedLine::Cmd(cmd) = parse(b"FOO=bar if").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.env_assigns, vec![assign("FOO", "bar", false)]);
    assert_eq!(cmd.command, c"if".into());
}

#[test]
fn test_word_with_equals_not_leading_is_arg() {
    let ParsedLine::Cmd(cmd) = parse(b"cmd FOO=bar").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.env_assigns.is_empty());
    assert_eq!(cmd.args, vec![c"FOO=bar".into()]);
}

#[test]
fn test_fdstyle_word_after_prefix_ends_the_prefix() {
    // A `%`-prefixed word after the assignment prefix is a command word, not
    // an assignment word: the prefix keeps only the words before it.
    let ParsedLine::Cmd(cmd) = parse(b"FOO=bar %foo=baz").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.env_assigns, vec![assign("FOO", "bar", false)]);
    assert_eq!(cmd.command, c"%foo=baz".into());
}

#[test]
fn test_fdstyle_word_leading_is_arg_not_prefix() {
    // `%foo=bar` is not an assignment word (`%` names a file descriptor).
    let ParsedLine::Cmd(cmd) = parse(b"%foo=bar cmd").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.env_assigns.is_empty());
    assert_eq!(cmd.command, c"%foo=bar".into());
}

/// POSIX #4.1: an escape pair in the name makes the word a plain command word,
/// never an assignment — `\X=1` and `X\=1` are words. Word 0 keeps its raw text
/// at parse (task #268): the escape pair is folded by the word-0 expansion pass
/// (`command_word::expand`), so the command that runs is the folded `X=1`.
#[test]
fn test_env_assign_escaped_name_is_not_an_assignment() {
    let ParsedLine::Cmd(cmd) = parse(b"\\X=1 cmd").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.env_assigns.is_empty());
    assert_eq!(cmd.command, c"\\X=1".into());
    assert_eq!(cmd.command_mask, vec![Unquoted; 4]);
    assert_eq!(cmd.args, vec![c"cmd".into()]);

    let ParsedLine::Cmd(cmd) = parse(b"X\\=1 cmd").unwrap() else {
        panic!("expected Cmd")
    };
    assert!(cmd.env_assigns.is_empty());
    assert_eq!(cmd.command, c"X\\=1".into());
    assert_eq!(cmd.command_mask, vec![Unquoted; 4]);
    assert_eq!(cmd.args, vec![c"cmd".into()]);
}

/// The assignment value keeps the escape pair: substitution drops the backslash
/// later and the escaped space stays inside the value (never an IFS split).
/// The value's mask is the **value's** own 4 bytes, all unquoted — not the 6
/// of the word (`X=a\ b`), so a slice shifted by one is caught here.
#[test]
fn test_env_assign_value_keeps_the_escape_pair() {
    let ParsedLine::Cmd(cmd) = parse(b"X=a\\ b cmd").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.env_assigns, vec![assign("X", "a\\ b", false)]);
    assert_eq!(cmd.env_assigns[0].mask, vec![Unquoted; 4]);
    assert_eq!(cmd.command, c"cmd".into());
}

/// The mask slice offset, pinned with two prefix words whose quoting
/// **differs** (LESSONS: an off-by-one and an off-by-two slice are
/// indistinguishable in a same-quoting pair). `x="a\ b"` carries a quoted
/// escape pair that must keep its mask bits; `y=b\ c` is fully unquoted.
#[test]
fn test_env_assign_value_mask_is_sliced_at_the_equals() {
    let ParsedLine::Cmd(cmd) = parse(b"x=\"a\\ b\" y=b\\ c cmd").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.env_assigns.len(), 2);
    assert_eq!(cmd.env_assigns[0].value, c"a\\ b".into());
    assert_eq!(cmd.env_assigns[0].mask, runs(&[(true, 4)]));
    assert_eq!(cmd.env_assigns[1].value, c"b\\ c".into());
    assert_eq!(cmd.env_assigns[1].mask, vec![Unquoted; 4]);
    assert_eq!(cmd.command, c"cmd".into());
}

/// The offset is `name.len() + 1`, measured on the **name's** bytes, not on the
/// raw span: a quoted name (`"x"`) is one word byte and one mask bit, so the
/// value mask starts at the value's first byte.
#[test]
fn test_env_assign_value_mask_of_a_quoted_name() {
    let ParsedLine::AssignStr(a) = parse(b"\"x\"=\"a\\ b\"").unwrap() else {
        panic!("expected AssignStr")
    };
    assert_eq!(a.name, c"x".into());
    assert_eq!(a.value, c"a\\ b".into());
    assert_eq!(a.mask, runs(&[(true, 4)]));
}

/// The value's zero-byte span slice (`value_spans`, task #321) shares
/// `value_mask`'s offset, pinned on raw lists: `abc=~/x""` (name 3, pair index
/// 7 → 3, the value's own end) and `abc=~""` (index 4 → 0, the value's start).
/// A 1-char name cannot tell `i - off` from `i / off` (`3-2 == 3/2`); with
/// `off = 4`, `7-4 = 3` (the tilde expands) and `7/4 = 1` (it is suppressed).
/// The drop case is the pair inside the name (`a""=1` is the text `a=`, index
/// 1 < 2): an inclusive-only drop keeps it and shifts to 0.
#[test]
fn test_env_assign_value_spans_slice_at_the_equals() {
    for (spans, name_len, want) in [
        (&[7][..], 3, vec![3]),
        (&[4][..], 3, vec![0]),
        (&[1][..], 1, Vec::new()),
        (&[0, 4, 7][..], 3, vec![0, 3]),
        (&[][..], 3, Vec::new()),
    ] {
        assert_eq!(
            value_spans(spans, name_len),
            want,
            "{spans:?} name_len {name_len}"
        );
    }
}

/// The 1:1 invariant `value_mask` rests on: the tokenizer pushes one mask bit
/// per word byte, so a word and its mask always have the same length. Pinned
/// as a test, not as a `debug_assert!` (nextest runs debug binaries, and a
/// debug assert would abort the test binary).
#[test]
fn test_token_mask_is_parallel_to_the_word() {
    for line in [
        "x=\"a b\" 'c d' e\\ f $(printf a) `printf b` g",
        "\"\" \"\"$(true)",
        "%fd <%q \"a\\$b\" $x*y",
    ] {
        let tokens = token::tokenize_statement(line.as_bytes()).unwrap();
        for (word, _, _, _, mask, _spans) in &tokens {
            assert_eq!(
                mask.len(),
                word.len(),
                "mask/word length mismatch for {line:?}"
            );
        }
    }
}

/// `;` ends the statement, so a statement that is only a `;` has no prefix word
/// at all: `prefix_len`'s bound is the first `;`. (A widened bound — `i + 1` →
/// `i * 1`, `<` → `<=` — counts the word in front of the `;`: this returns 1.)
#[test]
fn test_env_assign_prefix_len_stops_at_the_semicolon() {
    let tokens = [word_token(b"A=1", 0), word_token(b";", 4)];
    assert_eq!(envassign::prefix_len(&tokens), 0);
}

/// The last word of a statement is never a prefix word: a trailing assignment
/// has no command word to attach to, so the prefix stops one word short. (The
/// same widened bound — `i + 1` → `i * 1`, `<` → `<=` — counts it: this returns
/// 2; `i + 1` → `i - 1` underflows at `i = 0`, and `take_while` → `skip_while`
/// or a deleted `!` collapses the bound to 0, so every row that expects a
/// non-zero prefix fails.)
#[test]
fn test_env_assign_prefix_len_never_counts_the_last_word() {
    let tokens = [word_token(b"A=1", 0), word_token(b"B=2", 4)];
    assert_eq!(envassign::prefix_len(&tokens), 1);
}

/// The `command`/`builtin` keyword path must keep the command *name* word out of
/// the scoped-assignment range: `args_from = prefix + kw + 1` with `kw = 1`, so
/// widening the range by one word (`args_from - 1` → `args_from`, and the `+`
/// neighbour) collects the assignment-shaped name `X=1` as a scoped assignment.
/// In-process pin of the `args_from - 1` bound — the integration kill
/// `command_keyword_keeps_the_name_word_out_of_the_env_prefix` needs a scratch
/// `PATH` file literally named `X=1`; this one needs no fork.
#[test]
fn test_env_assign_prefix_stops_before_the_keyword_name_word() {
    let ParsedLine::Cmd(cmd) = parse(b"command X=1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.prefix, BuiltinPrefix::Command);
    assert_eq!(cmd.command, c"X=1".into());
    assert!(cmd.env_assigns.is_empty());
    assert_eq!(cmd.args, Vec::new());

    let ParsedLine::Cmd(cmd) = parse(b"builtin X=1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.prefix, BuiltinPrefix::Builtin);
    assert_eq!(cmd.command, c"X=1".into());
    assert!(cmd.env_assigns.is_empty());

    // The same bound with a non-empty scoped prefix: the prefix keeps only its
    // own words, never the keyword and never the assignment-shaped name.
    let ParsedLine::Cmd(cmd) = parse(b"A=2 command X=1").unwrap() else {
        panic!("expected Cmd")
    };
    assert_eq!(cmd.env_assigns, vec![assign("A", "2", false)]);
    assert_eq!(cmd.command, c"X=1".into());
    assert_eq!(cmd.args, Vec::new());
}

/// A word-start plain `&` is a **command-list terminator** in every block
/// position (task #354): the body slice keeps the `&` — the body's own scan is
/// what marks the background flag — and each closer glued behind it is found.
/// A `&`-terminated **condition** is cut at the `&` and carries the job's fact
/// as the parsed block's `cond_background` flag (task #356).
#[test]
fn ampersand_in_a_block_body_is_a_list_terminator() {
    let line = b"for i in 1; do echo hi & done";
    let fb = for_block::tokens_to_for(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(fb.body.data, c"echo hi &".into());

    let line = b"{ echo hi & }";
    let gb = group_block::tokens_to_group(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(gb.body.data, c"echo hi &".into());
    assert!(gb.redirects.is_empty());

    let line = b"while : & do echo x; done";
    let wb = while_block::tokens_to_loop(&token::tokenize(line).unwrap(), b"while", &stext(line))
        .unwrap();
    assert_eq!(
        wb.condition.data,
        c":".into(),
        "the `&` is cut from the cond"
    );
    assert!(wb.cond_background, "the cut `&` is the job's flag");
    assert_eq!(wb.body.data, c"echo x".into());

    let line = b"while :; do echo hi & done";
    let wb = while_block::tokens_to_loop(&token::tokenize(line).unwrap(), b"while", &stext(line))
        .unwrap();
    assert_eq!(wb.body.data, c"echo hi &".into());
    assert!(!wb.cond_background, "a body `&` never sets the cond flag");

    let line = b"if true; then echo hi & fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.then_body.data, c"echo hi &".into());
    assert!(!ib.cond_background, "a body `&` never sets the cond flag");

    let line = b"if true & then echo x; fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.condition.data, c"true".into());
    assert_eq!(ib.then_body.data, c"echo x".into());
    assert!(ib.cond_background);

    for line in [
        b"if true &\nthen echo x; fi".as_slice(),
        b"if true &\n\nthen echo x; fi".as_slice(),
    ] {
        let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
        assert_eq!(ib.condition.data, c"true".into(), "{line:?}");
        assert!(
            ib.cond_background,
            "a newline `;` token steps back: {line:?}"
        );
    }

    let line = b"while : &\ndo echo x; done";
    let wb = while_block::tokens_to_loop(&token::tokenize(line).unwrap(), b"while", &stext(line))
        .unwrap();
    assert_eq!(wb.condition.data, c":".into());
    assert!(wb.cond_background);

    let line = b"if true; then echo a & else echo b & fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.then_body.data, c"echo a &".into());
    assert_eq!(ib.else_body.unwrap().data, c"echo b &".into());

    let line = b"if false; then :; elif true & then echo e & fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.elifs[0].cond.data, c"true".into());
    assert_eq!(ib.elifs[0].body.data, c"echo e &".into());
    assert!(ib.elifs[0].cond_background);
    assert!(!ib.cond_background, "the `if` condition stays foreground");

    let line = b"if false; then :; elif true; then echo e; fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert!(!ib.elifs[0].cond_background);

    let line = b"f(){ echo hi & }";
    let fd =
        function_block::tokens_to_function(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(fd.name, c"f".into());
    assert_eq!(fd.body.data, c"echo hi &".into());
}

/// The `& ;` written inside a block body stays **in the body text** (task
/// #355): the body slice carries the split, and the body's own scan is what
/// rejects it (bash's rc-2 family, fdshell rc 1, nothing runs). Every shape
/// of the accepted #354 family gains exactly the glued `;`.
#[test]
fn ampersand_then_semicolon_stays_in_the_body_text() {
    let line = b"for i in 1; do echo hi & ; done";
    let fb = for_block::tokens_to_for(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(fb.body.data, c"echo hi & ;".into());

    let line = b"while :; do echo hi & ; break; done";
    let wb = while_block::tokens_to_loop(&token::tokenize(line).unwrap(), b"while", &stext(line))
        .unwrap();
    assert_eq!(wb.body.data, c"echo hi & ; break".into());

    let line = b"if true; then echo a & ; fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.then_body.data, c"echo a & ;".into());

    let line = b"if true; then :; else echo b & ; fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.else_body.unwrap().data, c"echo b & ;".into());

    let line = b"if false; then :; elif true; then echo e & ; fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.elifs[0].body.data, c"echo e & ;".into());

    let line = b"f(){ echo hi & ; }";
    let fd =
        function_block::tokens_to_function(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(fd.body.data, c"echo hi & ;".into());

    // The condition slice carries the split too (its scan rejects it) and the
    // `&`-terminator flag stays false: the real `;` byte stops the walk, so
    // `& ;` records no job (task #356, #355's rc-1 family).
    let line = b"if true & ; then echo x; fi";
    let ib = if_block::tokens_to_if(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(ib.condition.data, c"true & ;".into());
    assert!(!ib.cond_background);
}

/// The `&`-terminated arm of the `case` clause: the arm body keeps the `&` and
/// the clause closer (`;;`, `;;esac`, `esac`) is found behind it. The `& ;`
/// form keeps the glued `;` too (task #355): the pair head is the statement
/// terminator, not the `;;` separator — while the **glued** `;;` behind the
/// `&` stays the separator bash accepts (the four rc-0 forms above).
#[test]
fn ampersand_terminated_case_clause_keeps_the_ampersand() {
    for line in [
        b"case x in x) echo a & ;; esac".as_slice(),
        b"case x in x) echo a &;; esac".as_slice(),
        b"case x in x) echo a & ;;esac".as_slice(),
        b"case x in x) echo a & esac".as_slice(),
    ] {
        let block = case_block::tokens_to_case(&token::tokenize(line).unwrap(), &stext(line))
            .unwrap_or_else(|e| panic!("{line:?}: {e}"));
        assert_eq!(block.clauses.len(), 1, "{line:?}");
        assert_eq!(block.clauses[0].body.data, c"echo a &".into(), "{line:?}");
    }
    let line = b"case x in x) echo a & ; ;; esac";
    let block = case_block::tokens_to_case(&token::tokenize(line).unwrap(), &stext(line)).unwrap();
    assert_eq!(block.clauses.len(), 1);
    assert_eq!(block.clauses[0].body.data, c"echo a & ;".into());
}

/// `for`'s word list stays strict: the `&` is **not** a `do`-position
/// terminator, so `for i in 1 & do :; done` is a parse error (bash rc 2).
#[test]
fn ampersand_in_a_for_word_list_is_not_a_do_terminator() {
    let line = b"for i in 1 & do :; done";
    assert!(for_block::tokens_to_for(&token::tokenize(line).unwrap(), &stext(line)).is_err());
}

// --- the redirection written after a keyword block's closer (task #298) -----

/// The trailing redirections of whatever keyword block `line` parses to.
fn block_redirects(line: &[u8]) -> Vec<RedirectDef> {
    let parsed = parse(line).unwrap_or_else(|e| panic!("{line:?}: {e}"));
    match parsed {
        ParsedLine::While(b) | ParsedLine::Until(b) => b.redirects,
        ParsedLine::If(b) => b.redirects,
        ParsedLine::For(b) => b.redirects,
        ParsedLine::ArithFor(b) => b.redirects,
        ParsedLine::Case(b) => b.redirects,
        ParsedLine::Wait(b) => b.redirects,
        _ => panic!("not a block: {line:?}"),
    }
}

/// The tail rule of every builder that owns a closer: the redirection written
/// after the closing keyword is the **block's** own, in source order, exactly
/// the def the group's tail gives for the same operator.
#[test]
fn a_redirect_after_a_block_closer_is_the_blocks_own() {
    for line in [
        b"while :; do echo a; break; done > f".as_slice(),
        b"until :; do echo a; break; done > f".as_slice(),
        b"if :; then echo a; fi > f".as_slice(),
        b"for x in a; do echo A; done > f".as_slice(),
        b"for ((i=0;i<1;i++)); do echo A; done > f".as_slice(),
        b"case x in x) echo C;; esac > f".as_slice(),
        b"wait\n readable %rd) echo hi ;;\ndone > f".as_slice(),
        // The glued closer (no space): the same block, the same def.
        b"while :; do echo a; break; done>f".as_slice(),
        b"until :; do echo a; break; done>f".as_slice(),
        b"if :; then echo a; fi>f".as_slice(),
        b"for x in a; do echo A; done>f".as_slice(),
        b"for ((i=0;i<1;i++)); do echo A; done>f".as_slice(),
        b"case x in x) echo C;; esac>f".as_slice(),
    ] {
        assert_eq!(
            block_redirects(line),
            vec![path_def(1, RedirectDirection::Write, "f")],
            "{line:?}"
        );
    }
}

/// The operator family the tail reads on a closer: append, an io-number, a dup,
/// a close, a read, and several redirections in source order.
#[test]
fn the_tail_reads_the_whole_redirect_family() {
    assert_eq!(
        block_redirects(b"while :; do :; done >> f"),
        vec![path_def(1, RedirectDirection::Append, "f")]
    );
    assert_eq!(
        block_redirects(b"while :; do :; done 3> f"),
        vec![path_def(3, RedirectDirection::Write, "f")]
    );
    assert_eq!(
        block_redirects(b"if :; then :; fi < g"),
        vec![path_def(0, RedirectDirection::Read, "g")]
    );
    assert_eq!(
        block_redirects(b"for x in a; do :; done 2>&1"),
        vec![RedirectDef {
            export_to: 2,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Dup(1),
        }]
    );
    assert_eq!(
        block_redirects(b"case x in x) :;; esac >&-"),
        vec![RedirectDef {
            export_to: 1,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::Close,
        }]
    );
    assert_eq!(
        block_redirects(b"while :; do :; done > f 2> g"),
        vec![
            path_def(1, RedirectDirection::Write, "f"),
            path_def(2, RedirectDirection::Write, "g")
        ]
    );
    // The same family, glued to the closer.
    assert_eq!(
        block_redirects(b"while :; do :; done>>f"),
        vec![path_def(1, RedirectDirection::Append, "f")]
    );
    assert_eq!(
        block_redirects(b"if :; then :; fi<g"),
        vec![path_def(0, RedirectDirection::Read, "g")]
    );
    assert_eq!(
        block_redirects(b"while :; do :; done>f 2> g"),
        vec![
            path_def(1, RedirectDirection::Write, "f"),
            path_def(2, RedirectDirection::Write, "g")
        ]
    );
}

/// The leniency boundary, in every builder: the bare closer takes no
/// redirection, a plain word behind it is task #405's (the block runs, the word
/// stays as it is today), and a here-doc/here-string operator on the closer is
/// #408's.
#[test]
fn a_word_and_a_heredoc_operator_behind_a_closer_stay_their_owners() {
    for line in [
        b"while :; do :; done".as_slice(),
        b"if :; then :; fi".as_slice(),
        b"for x in a; do :; done".as_slice(),
        b"for ((i=0;i<1;i++)); do :; done".as_slice(),
        b"case x in x) :;; esac".as_slice(),
        b"while :; do :; done x".as_slice(),
        b"if :; then :; fi x".as_slice(),
        b"case x in x) :;; esac x".as_slice(),
        b"while :; do cat <<T; done".as_slice(),
        b"while :; do :; done <<T".as_slice(),
        b"if :; then :; fi <<<hi".as_slice(),
        b"for x in a; do :; done 3<<T".as_slice(),
        b"case x in x) :;; esac \">\" f".as_slice(),
    ] {
        assert!(block_redirects(line).is_empty(), "{line:?}");
    }
}

/// A redirect operator with no operand behind a closer is the house redirect
/// error (bash: rc 2), not a silently dropped tail.
#[test]
fn an_operand_less_operator_behind_a_closer_is_the_redirect_error() {
    for line in [
        b"while :; do :; done >".as_slice(),
        b"if :; then :; fi 2>".as_slice(),
    ] {
        assert_eq!(
            parse(line).err().unwrap().current_context().to_string(),
            "invalid redirect syntax",
            "{line:?}"
        );
    }
}
