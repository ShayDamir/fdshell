#![allow(clippy::unwrap_used)]
use super::{build, leads};
use crate::parse::heredoc::HeredocBody;
use crate::parse::token::tokenize_statement;
use crate::parse::{CommandLine, Token, envassign};
use crate::redirect::RedirectSource;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use sys::Position;

/// `(tokens, heredoc specs)` of the statement, as `parse_command` receives them.
fn parts(line: &[u8]) -> (Vec<Token>, Vec<HeredocBody>) {
    let tokens = tokenize_statement(line).unwrap();
    let specs = crate::parse::heredoc::layout(line, &tokens).unwrap();
    (tokens, specs)
}

/// Whether the gate fires on the statement; panics on a parse error, which the
/// [`gate_err`] table pins separately.
fn gate(line: &[u8]) -> bool {
    let (tokens, specs) = parts(line);
    leads(&tokens, envassign::prefix_len(&tokens), line, &specs).unwrap()
}

/// The gate's parse-error text (a bare operator with no operand is the only
/// shape that produces one).
fn gate_err(line: &[u8]) -> String {
    let (tokens, specs) = parts(line);
    match leads(&tokens, envassign::prefix_len(&tokens), line, &specs) {
        Ok(v) => panic!("expected a parse error for {line:?}, gate={v}"),
        Err(e) => e.current_context().to_string(),
    }
}

fn built(line: &[u8]) -> CommandLine {
    let (tokens, specs) = parts(line);
    build(
        &tokens,
        envassign::prefix_len(&tokens),
        line,
        &specs,
        Position::new(1, 1),
    )
    .unwrap()
}

/// `(fd, target)` of each redirect — the shape, with the mask plumbing ignored:
/// the path text, `dupN`, `close`, `heredoc`, the here-string word.
fn shape(cmd: &CommandLine) -> Vec<(i32, Vec<u8>)> {
    cmd.redirects
        .iter()
        .map(|r| (r.export_to, target(&r.source)))
        .collect()
}

fn target(source: &RedirectSource) -> Vec<u8> {
    match source {
        RedirectSource::Path { path, .. }
        | RedirectSource::Var(path)
        | RedirectSource::HereString { word: path, .. } => path.as_bytes().unwrap_or(&[]).to_vec(),
        RedirectSource::Dup(from) => format!("dup{from}").into_bytes(),
        RedirectSource::Close => Vec::from(b"close".as_slice()),
        RedirectSource::HereDoc { .. } => Vec::from(b"heredoc".as_slice()),
    }
}

/// Every plain redirection shape at word 0 opens the gate: the same three
/// predicates the collection loop uses.
#[test]
fn the_gate_fires_on_every_redirect_shape() {
    for line in [
        b">f" as &[u8],
        b"> f",
        b">>f",
        b">|f",
        b"<>f",
        b"<f",
        b"2>f",
        b"2<f",
        b">f 2>&1",
        b">&2",
        b">&1",
        b">&-",
        b"0<&-",
        b"&>f",
        b"&>>f",
        b"<<<x",
        b"3<<<x",
        b">f g",
        b"VAR=x >f",
        b">$(echo f)",
    ] {
        assert!(gate(line), "{line:?} must open the gate");
    }
}

/// Word data never opens the gate: a plain word, a **quoted** operator word
/// (every quoting gate is inherited), a non-digit io-number prefix, the bare
/// `&`, the `%` capture word, and the `<<` token the here-doc machinery owns.
#[test]
fn the_gate_stays_off_on_word_data() {
    for line in [
        b"echo" as &[u8],
        b"echo a >f",
        b"'>f'",
        b"\"&>f\"",
        b"\"2>f\"",
        b"'>&2'",
        b"x>f",
        b"%f>x",
        b"2 > f",
        b"&",
        b"%o",
        b"<<EOF",
        b">&x",
    ] {
        assert!(!gate(line), "{line:?} must stay word data");
    }
}

/// A statement carrying a here-doc body keeps the gate **off**, which is what
/// preserves the pinned leading-operator family (`> <<EOF`, `>| x <<EOF`, the
/// async `cat <<A & <<B`) and routes the here-doc-only statement to its owner.
#[test]
fn a_heredoc_body_keeps_the_gate_off() {
    for line in [
        b">x <<EOF\nbody\nEOF" as &[u8],
        b">| x <<EOF\nbody\nEOF",
        b">>x <<EOF\nbody\nEOF",
        b"<<EOF\nbody\nEOF",
        b"cat <<EOF\nbody\nEOF",
    ] {
        assert!(!gate(line), "a heredoc spec keeps {line:?} off the gate");
    }
}

/// A bare operator with no operand keeps the collection loop's own parse error:
/// the gate calls the same predicate, so nothing new is reported.
#[test]
fn a_bare_operator_without_operand_keeps_the_parse_error() {
    for line in [b">" as &[u8], b">>", b"<", b"<>", b">|", b"&>", b"> >"] {
        assert!(
            gate_err(line).contains("invalid redirect"),
            "{line:?}: {}",
            gate_err(line)
        );
    }
}

/// `>f` builds the empty command: no command word, no argument, one redirect,
/// and `command_quoted == false` — the flag the run-side guard reads.
#[test]
fn build_leaves_the_command_word_empty() {
    let cmd = built(b">f");
    assert!(cmd.command.is_empty());
    assert!(!cmd.command_quoted);
    assert!(cmd.command_mask.is_empty());
    assert!(cmd.command_span.is_empty());
    assert!(cmd.args.is_empty());
    assert!(cmd.env_assigns.is_empty());
    assert_eq!(shape(&cmd), vec![(1, b"f".to_vec())]);
}

/// The leading `NAME=value` words stay the command's scoped prefix.
#[test]
fn build_keeps_the_scoped_prefix() {
    let cmd = built(b"VAR=x ECHO=echo >f");
    assert_eq!(cmd.env_assigns.len(), 2);
    assert!(
        cmd.env_assigns
            .first()
            .is_some_and(|a| a.name.eq_bytes(b"VAR"))
    );
    assert!(cmd.command.is_empty());
    assert_eq!(shape(&cmd), vec![(1, b"f".to_vec())]);
}

/// The command slot is open: the first plain word becomes the command word
/// (`>f g` runs `g` with the redirect, as bash), the rest are its arguments.
#[test]
fn the_first_plain_word_becomes_the_command_word() {
    let cmd = built(b">f g");
    assert!(cmd.command.eq_bytes(b"g"));
    assert!(!cmd.command_quoted);
    assert!(cmd.args.is_empty());
    assert_eq!(shape(&cmd), vec![(1, b"f".to_vec())]);
    let cmd = built(b">f g h");
    assert!(cmd.command.eq_bytes(b"g"));
    assert_eq!(
        cmd.args
            .iter()
            .map(|a| a.as_bytes().unwrap())
            .collect::<Vec<_>>(),
        vec![b"h".as_slice()]
    );
    // A quoted word 0 keeps its quoted flag, the run-side guard's gate.
    let cmd = built(b">f 'g'");
    assert!(cmd.command.eq_bytes(b"g"));
    assert!(cmd.command_quoted);
}

/// The dup and close sources are the loop's, and several redirections keep
/// their written order.
#[test]
fn build_keeps_the_dup_and_multi_redirect_shapes() {
    assert_eq!(shape(&built(b">&2")), vec![(1, b"dup2".to_vec())]);
    assert_eq!(shape(&built(b">&-")), vec![(1, b"close".to_vec())]);
    assert_eq!(shape(&built(b"2>&1")), vec![(2, b"dup1".to_vec())]);
    assert_eq!(
        shape(&built(b">f 2>&1")),
        vec![(1, b"f".to_vec()), (2, b"dup1".to_vec())]
    );
    assert_eq!(
        shape(&built(b">f >g")),
        vec![(1, b"f".to_vec()), (1, b"g".to_vec())]
    );
    assert_eq!(shape(&built(b"<<<x")), vec![(0, b"x".to_vec())]);
}

/// The `&>` family and a substituted target are the loop's shapes too: one
/// redirect, and the target word travels **raw** with its own mask — the
/// expansion is the run side's (`substituted_target_creates_the_expanded_name`
/// in `tests/redirect.rs` pins it end to end).
#[test]
fn build_keeps_the_bg_and_substituted_shapes() {
    assert_eq!(shape(&built(b"&>f")), vec![(1, b"f".to_vec())]);
    assert_eq!(shape(&built(b"&>>f")), vec![(1, b"f".to_vec())]);
    assert_eq!(
        shape(&built(b">$(echo f)")),
        vec![(1, b"$(echo f)".to_vec())]
    );
    assert!(built(b">$(echo f)").command.is_empty());
}

/// `>>&x` is the collection loop's own shape: the operator is `>>` and `&x` is
/// its **path** operand (fdshell never splits a word at an operator byte, so
/// `echo a >>&x` appends to the file `&x` today). The gate inherits that
/// reading, so the statement is a redirections-only command; bash rejects the
/// word outright (rc 2). The `>&x` twin stays word data, because the dup arm
/// answers `None` for a non-numeric, non-`%` tail.
#[test]
fn an_ampersand_operand_is_the_loops_own_shape() {
    assert!(gate(b">>&x"));
    assert_eq!(shape(&built(b">>&x")), vec![(1, b"&x".to_vec())]);
    assert!(!gate(b">&x"));
}
