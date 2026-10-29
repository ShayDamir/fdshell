#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::operator::operator_at;
use super::{delimiter_token_indices, is_operator, layout, operator_count};
use crate::error::parse::ParseError;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

/// Tokenize (with heredoc bodies blanked) and extract the bodies.
fn specs(line: &[u8]) -> Vec<super::HeredocBody> {
    let tokens = tokenize_statement(line).unwrap();
    layout(line, &tokens).unwrap()
}

#[test]
fn layout_attached_delimiter_expands() {
    let s = specs(b"cat <<EOF\nbody\nEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(s[0].expand);
}

#[test]
fn layout_separate_delimiter_expands() {
    let s = specs(b"cat << EOF\nbody\nEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(s[0].expand);
}

#[test]
fn layout_quoted_delimiter_is_literal() {
    let s = specs(b"cat <<\"Q\"\nx\nQ");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"x\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

/// The delimiter word is a word at the token level too: every quote byte is
/// removed, so the terminator line is the stripped word, and any quoting in
/// the word makes the body literal (task #207).
#[test]
fn layout_delimiter_quote_removal_is_literal() {
    for (line, want) in [
        (b"cat <<a'b'c\n$X\nabc" as &[u8], b"$X\n" as &[u8]),
        (b"cat <<'x'y'z'\n$X\nxyz", b"$X\n"),
        (b"cat <<ab'c'\n$X\nabc", b"$X\n"),
        (b"cat <<'a'bc\n$X\nabc", b"$X\n"),
        (b"cat <<- a'b'c\n\t$X\n\tabc", b"$X\n"),
        (b"cat <<a\"b\"c\n$X\nabc", b"$X\n"),
        (b"cat <<\"\"Q\"x\"\n$X\nQx", b"$X\n"),
        (b"cat <<a\"b c\"d\n$X\nab cd", b"$X\n"),
        (b"cat <<\"a b\"\n$X\na b", b"$X\n"),
    ] {
        let s = specs(line);
        assert_eq!(s.len(), 1, "line={:?}", line);
        assert_eq!(
            s[0].body,
            ShortCStr::from_vec(want.to_vec()).unwrap(),
            "line={:?}",
            line
        );
        assert!(!s[0].expand, "a quoted delimiter never expands");
    }
}

/// An escaped quote is data in the delimiter **text** (`<<a\"b` delimits the
/// literal `a"b`), and the pair that produced it is a POSIX quote operator, so
/// the word is **quoted** and the body literal (POSIX XCU 2.3, task #263).
/// A delimiter word that contains a `<<` is one operator at both levels on its
/// command line (pinned in `scan/heredoc/ops/tests.rs` and by the agreement
/// rows above). Its multi-line form (`cat <<'a <<b'` + body + `a <<b`) is not
/// layout-pinnable yet: the terminator line is part of the statement text, so
/// its own `<<b` counts as an operator and the body allocation shifts — the
/// terminator-line defect, filed as task #266.
#[test]
fn layout_escaped_quote_delimiter_word_is_a_literal_delimiter() {
    let (op, extra) = operator_at(
        b"cat <<a\\\"b",
        &tokenize_statement(b"cat <<a\\\"b").unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(op.delim, b"a\"b" as &[u8]);
    assert!(op.quoted);
    assert_eq!(extra, 0);
}

#[test]
fn layout_dash_form_strips_leading_tabs_and_expands() {
    let s = specs(b"cat <<-EOF\n\tone\n\t\ttwo\nthree\n\tEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"one\ntwo\nthree\n".to_vec()).unwrap()
    );
    assert!(s[0].expand);
}

#[test]
fn layout_dash_quoted_delimiter_is_literal_and_stripped() {
    let s = specs(b"cat <<-\"Q\"\n\t$X\n\tQ");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"$X\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

#[test]
fn layout_dash_separate_word_strips() {
    let s = specs(b"cat <<- EOF\n\tbody\n\tEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(s[0].expand);
}

#[test]
fn layout_dash_bare_marker_inherited_by_next_word() {
    // `<<- -`: the marker is inherited by the bare-form delimiter word `-`.
    let s = specs(b"cat <<- -\n\tX\n\t-");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"X\n".to_vec()).unwrap());
    assert!(s[0].expand);
}

#[test]
fn layout_mixed_dash_and_plain_keep_independent_flags() {
    let s = specs(b"cat <<-A <<B\n\ta\nb\n\tA\nc\nB");
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"a\nb\n".to_vec()).unwrap());
    assert!(s[0].expand);
    assert_eq!(s[1].body, ShortCStr::from_vec(b"c\n".to_vec()).unwrap());
    assert!(s[1].expand);
}

#[test]
fn layout_empty_body() {
    let s = specs(b"cat <<EOF\nEOF");
    assert_eq!(s.len(), 1);
    assert!(s[0].body.is_empty());
}

#[test]
fn layout_empty_quoted_delimiter() {
    // `<<""` mirrors `<<<""`: the body ends at the first blank line, which
    // must be part of the statement span.
    let s = specs(b"cat <<\"\"\nbody\n\n");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

#[test]
fn layout_quoted_newline_on_command_line() {
    // The first unquoted newline ends the command line; a newline inside the
    // quotes on it is not a line break.
    let s = specs(b"cat <<EOF \"a\nb\"\nbody\nEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
}

#[test]
fn layout_body_is_opaque_to_tokenizer() {
    // Unbalanced quotes in the body must not break tokenization.
    let s = specs(b"cat <<EOF\n\"unbalanced\nEOF\necho \"x\"");
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"\"unbalanced\n".to_vec()).unwrap()
    );
}

/// Nothing is left after the command line: the body is **empty** and the
/// statement runs — bash's end-of-file shape, warning and rc 0 (task #364). The
/// warning itself is asserted in the integration tests.
#[test]
fn layout_missing_newline_is_accepted_with_an_empty_body() {
    let s = specs(b"cat <<EOF");
    assert_eq!(s.len(), 1);
    assert!(s[0].body.is_empty());
    assert!(s[0].expand);
}

/// A zero-byte final line (EOF right after the body) is not the blank delimiter
/// line, so the delimiter never arrives: the body is the remaining lines and the
/// statement runs (bash warns, prints `body`, rc 0 — task #364).
#[test]
fn layout_empty_delimiter_eof_takes_the_remaining_lines() {
    let s = specs(b"cat <<\"\"\nbody\n");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

/// The delimiter line is missing at the end of the text, so the body is every
/// line the stream leaves — unterminated last line included, closed to a whole
/// line (`eof::close_last_line`): bash prints `body` and `nothing` on their own
/// lines with its warning at rc 0 (task #364).
#[test]
fn layout_missing_delimiter_takes_the_remaining_lines() {
    let s = specs(b"cat <<NOPE\nbody\nnothing");
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"body\nnothing\n".to_vec()).unwrap()
    );
    assert!(s[0].expand);
}

#[test]
fn layout_bare_operator_at_eol_is_invalid_redirect() {
    let line = b"cat <<";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::InvalidRedirect
    ));
}

#[test]
fn layout_bare_operator_before_semicolon_is_invalid_redirect() {
    let line = b"cat << ;";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::InvalidRedirect
    ));
}

#[test]
fn layout_dash_bare_operator_at_eol_is_invalid_redirect() {
    let line = b"cat <<-";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::InvalidRedirect
    ));
}

#[test]
fn layout_dash_bare_operator_before_semicolon_is_invalid_redirect() {
    let line = b"cat <<- ;";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::InvalidRedirect
    ));
}

#[test]
fn layout_here_string_is_not_heredoc() {
    let line = b"cat <<<x";
    let tokens = tokenize_statement(line).unwrap();
    assert!(layout(line, &tokens).unwrap().is_empty());
}

#[test]
fn layout_attached_word_is_not_operator() {
    let s = specs(b"x<<EOF\nEOF");
    assert!(s.is_empty());
}

#[test]
fn layout_substitution_closing_paren_keeps_word() {
    // A `$( )`-closing `)` does not split the word: `$(x)<<EOF` is one token
    // that does not start with `<<`, so it is argument text, not an operator.
    // The body lines are real statements, never swallowed.
    let line = b"cat $(x)<<EOF\nb\nEOF";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(tokens.len(), 6);
    assert!(
        tokens[1].0.eq_bytes(b"$(x)<<EOF"),
        "the token must stay whole"
    );
    assert!(layout(line, &tokens).unwrap().is_empty());
}

#[test]
fn layout_bare_operator_before_redirect_word_is_invalid() {
    // A bare `<<` takes the next word as delimiter; a word shaped like a
    // redirect/operator is not a delimiter.
    for line in [b"cat << <x", b"cat << >x", b"cat << &x", b"cat << %x"] {
        let tokens = tokenize_statement(line).unwrap();
        assert!(
            matches!(
                layout(line, &tokens).unwrap_err().current_context(),
                ParseError::InvalidRedirect
            ),
            "line={:?}",
            core::str::from_utf8(line).unwrap()
        );
    }
}

/// Task #323: the same guard on a **quoted** operand does not fire — POSIX
/// #4.1 removes quotes after tokenization, so `"<< x"`'s word is the plain
/// `<x`. Each bare form yields one operator whose delimiter is the
/// quote-removed word, marked quoted, with the `<<-` marker carried; the
/// multi-line body ends at the stripped terminator and stays literal.
#[test]
fn layout_bare_quoted_operator_delimiter_is_a_delimiter() {
    for (line, want) in [
        (b"cat << \"<x\"".as_slice(), b"<x".as_slice()),
        (b"cat << \"&x\"", b"&x"),
        (b"cat << \"%x\"", b"%x"),
        (b"cat <<- \"<x\"", b"<x"),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let (op, extra) = operator_at(line, &tokens, 1).unwrap();
        assert_eq!(op.delim.as_slice(), want, "line={:?}", line);
        assert!(op.quoted, "line={:?}", line);
        assert_eq!(op.strip, line.starts_with(b"cat <<-"), "line={:?}", line);
        assert_eq!(extra, 1, "the bare form consumes the delimiter word");
    }
    for (line, body) in [
        (b"cat << \"<x\"\nbody\n<x".as_slice(), b"body\n".as_slice()),
        (b"cat << \"&x\"\nbody\n&x", b"body\n"),
        (b"cat << \"%x\"\nbody\n%x", b"body\n"),
        (b"cat <<- \"<x\"\n\tbody\n\t<x", b"body\n"),
    ] {
        let s = specs(line);
        assert_eq!(s.len(), 1, "line={:?}", line);
        assert_eq!(
            s[0].body,
            ShortCStr::from_vec(body.to_vec()).unwrap(),
            "line={:?}",
            line
        );
        assert!(!s[0].expand, "a quoted delimiter never expands");
    }
}

/// Task #330: the same guard on a **quoted empty** operand does not fire
/// either — POSIX #2.3 removes quotes after tokenization, so `""` / `''` *is*
/// a word: the **empty** delimiter, terminated by the blank line (a tabs-only
/// line in the `<<-` form, which untabs to empty). The token text is empty and
/// the mask has no bit for a zero-byte quoted region, so the raw-span test
/// (`parse::word_quoted`) is what separates the quoted pair from the missing
/// operand the empty arm rejects. Each bare form yields one operator with an
/// empty, quoted delimiter, the `<<-` marker carried, the io-number honoured,
/// and a literal body ending at the empty line.
#[test]
fn layout_bare_quoted_empty_delimiter_is_a_delimiter() {
    for (line, strip, fd) in [
        (b"cat << \"\"".as_slice(), false, 0i32),
        (b"cat << ''", false, 0),
        (b"cat <<- \"\"", true, 0),
        (b"cat 3<< \"\"", false, 3),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let (op, extra) = operator_at(line, &tokens, 1).unwrap();
        assert_eq!(op.delim.as_slice(), b"", "line={:?}", line);
        assert!(op.quoted, "a quoted empty pair quotes the body");
        assert_eq!(op.strip, strip, "line={:?}", line);
        assert_eq!(op.fd, fd, "line={:?}", line);
        assert_eq!(extra, 1, "the bare form consumes the delimiter word");
    }
    for (line, body) in [
        (b"cat << \"\"\nbody\n\n".as_slice(), b"body\n".as_slice()),
        (b"cat << ''\nbody\n\n", b"body\n"),
        (b"cat <<- \"\"\n\tbody\n\t\n", b"body\n"),
        (b"cat << \"\"\n\n", b""),
    ] {
        let s = specs(line);
        assert_eq!(s.len(), 1, "line={:?}", line);
        assert_eq!(
            s[0].body,
            ShortCStr::from_vec(body.to_vec()).unwrap(),
            "line={:?}",
            line
        );
        assert!(!s[0].expand, "a quoted delimiter never expands");
    }
}

#[test]
fn operator_count_counts_word_zero() {
    // Word 0 counts when its token is an operator: POSIX's
    // `simple_command = [NAME=value…] REDIRECT… [word…]` has no mandatory
    // command word, so `<<EOF x` is the body-fed `x` and `<<EOF` alone is the
    // redirections-only statement (task #345). The byte level agrees.
    let line = b"<<EOF x";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(line, &tokens), 1);
    assert_eq!(crate::scan::heredoc::operator_count(line, 0, line.len()), 1);
}

#[test]
fn operator_count_builtin_prefix() {
    // The builtin prefix shifts the command word; `<<EOF` is still an operator.
    let line = b"builtin cat <<EOF";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(line, &tokens), 1);
}

#[test]
fn operator_count_pipe_position_is_an_operator() {
    // The word right after a pipeline `|` is that stage's word 0, and a
    // here-doc operator there is the stage's redirections-only command
    // (`a | <<B` is `a` piped into an empty command fed by the body, rc 0 in
    // bash) — task #345. The byte level counts the same 1.
    let line = b"a | <<B";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(line, &tokens), 1);
    assert_eq!(crate::scan::heredoc::operator_count(line, 0, line.len()), 1);
}

#[test]
fn operator_count_per_stage() {
    let line = b"cat <<A | tr <<B";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(line, &tokens[0..2]), 1);
    assert_eq!(operator_count(line, &tokens[3..5]), 1);
}

/// The byte-level and token-level operator counts agree on every supported
/// form — attached, bare, `<<-`, quoted — and on `#` comments (the tokenizer
/// never yields a `<<` from one, so the byte scanner must skip it whole).
/// The quoted bare delimiters (task #323) are the mask-gated pair: the byte
/// level reads the raw span's first byte and the token level reads the
/// operand's quote mask, so `cat << "<x"` counts 1 on both sides
/// (`scan/heredoc/ops/tests.rs::bare_quoted_operator_delimiter_counts_one_at_both_levels`
/// pins the byte side; the unquoted twins stay errors — the guard rows above).
#[test]
fn operator_count_byte_and_token_levels_agree() {
    for line in [
        b"cat <<EOF" as &[u8],
        b"cat << EOF",
        b"cat <<-EOF",
        b"cat <<- EOF",
        b"cat <<\"Q\"",
        b"cat <<A && cat <<B",
        b"cat <<A # <<B\n",
        b"echo a#b <<X",
        b"cat <<A | tr <<B",
        b"cat << \"<x\"",
        b"cat << \"&x\"",
        b"cat << \"%x\"",
        b"cat <<- \"<x\"",
        // Task #261's four residual classes, measured on `master` and pinned
        // here at one place (`operator_count_pipe_predecessors_agree`,
        // `operator_count_invalid_delimiter_agree`,
        // `operator_count_leading_newline_agree` carry the rest).
        b"x\\| <<EOF",
        b"\"|\" <<EOF",
        b"'|' <<EOF",
        b">> << <<EOF",
        b"cat <<- <<EOF",
        b"\\n<<EOF".as_slice(),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
    }
}

/// A **terminator line** is heredoc structure at both levels: a text that
/// carries one counts only the operators written on its command lines, and
/// `layout` allocates exactly one spec per operator. The byte side of the same
/// rule is `scan/heredoc/opaque/tests.rs`; the two counts must agree, or the
/// global per-line body allocation shifts and the delimiter-shaped residue is
/// parsed as a command (task #266).
#[test]
fn a_terminator_line_is_not_an_operator_at_either_level() {
    for (line, want) in [
        (b"cat <<'a <<b'\nbody\na <<b".as_slice(), 1usize),
        (b"cat <<\"a <<b\"\nbody\na <<b", 1),
        (b"cat <<a\\\"b\nbody\na\"b", 1),
        (b"cat <<'a|b'\nbody\na|b", 1),
        (b"cat <<'a$(b'\nbody\na$(b", 1),
        (b"cat <<'a`b'\nbody\na`b", 1),
        (b"cat <<'a <<b' <<EOF\nbodyA\nbodyB\na <<b\nEOF", 2),
        (b"cat <<EOF <<'a <<b'\nbodyA\nbodyB\nEOF\na <<b", 2),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        assert_eq!(operator_count(line, &tokens), want, "line={:?}", line);
        assert_eq!(
            layout(line, &tokens).unwrap().len(),
            want,
            "line={:?}",
            line
        );
    }
}

/// POSIX #2.2 `>|`: the `|` absorbed into a clobber operator is an operator
/// byte, so it is neither a pipeline position nor a word break. The two counts
/// must agree on every clobber form, including `cat >| <<EOF` (one operator:
/// the token-level `<<EOF` is an operator, and the byte scan must not treat the
/// absorbed `|` as a pipe) and `echo a>|<<EOF` (no operator: the `<<` bytes are
/// inside one word). A divergence shifts the body regions and the per-command
/// spec allocation silently.
///
/// The pipeline-pipe rows (`| <<EOF`, `a >| b | <<EOF`, `cat "a>"| <<EOF`,
/// `cat x\>| <<EOF`, `cat a>b>| <<EOF`) count 1 at both levels since task
/// #345: a real pipeline pipe ends the previous stage's word and begins the
/// `<<` word, so the stage is a redirections-only command.
#[test]
fn operator_count_clobber_forms_agree_at_both_levels() {
    for (line, want) in [
        (b"cat >| q <<EOF" as &[u8], 1usize),
        (b"cat >| <<EOF", 1),
        (b"echo a>|<<EOF", 0),
        (b"a >| b | <<EOF", 1),
        (b"cat >| q <<EOF && cat <<A", 2),
        (b"| <<EOF", 1),
        // The shared rule (`parse::redirect::clobber_word`) is applied to the
        // raw word bytes with the quote mask: a quoted `>` never terminates an
        // operator word, an escape pair shields its second byte, and an
        // unquoted `>`/`<` earlier in the word disqualifies it — so the `|` is
        // a pipeline pipe and begins the `<<` word (1).
        (b"cat \"a>\"| <<EOF", 1),
        (b"echo \"a>\"|b <<EOF", 1),
        (b"cat x\\>| <<EOF", 1),
        (b"cat a>b>| <<EOF", 1),
        (b"cat x\\>a>| <<EOF", 1),
        (b"cat \"a>\"x\\y>| <<EOF", 1),
        // The escape pair is skipped only outside quotes: inside `"a\>"` the `>`
        // stays quoted, so the word ends at the real `>` after `x` and the `|`
        // is absorbed. A pair skip inside quotes would read the quoted `>` as an
        // unquoted operator byte and count 0.
        (b"cat \"a\\>\"x>| <<EOF", 1),
        // A `%`-leading capture word absorbs the `|` (the `%` rule ignores earlier
        // operator bytes), so the `<<` counts 1 at both levels.
        (b"cat %>>| <<EOF", 1),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(operator_count(line, &tokens), want, "line={:?}", line);
    }
}

/// Every **word-0 / word-break** shape agrees byte-for-token (task #345 moved
/// this family onto the operator side): the gate at both levels is "the `<<`
/// begins a word" — the run start, whitespace, `;`, a newline, a top-level `)`,
/// a pipeline `|`. Task #185 pinned the mirror when it was "a preceding token
/// that is not a pipeline `|`" (`> <<EOF` counted 0 byte-level and 1
/// token-level then); a divergence shifts the body regions and the per-command
/// spec allocation silently, so the rows below assert **both** levels in one
/// table, as the LESSONS entry requires.
#[test]
fn operator_count_leading_operator_forms_agree() {
    for (line, want) in [
        (b"> <<EOF" as &[u8], 1usize),
        (b">| <<EOF", 1),
        (b">> <<EOF", 1),
        (b">& <<EOF", 1),
        (b"<> <<EOF", 1),
        (b"< <<EOF", 1),
        (b">>x <<EOF", 1),
        (b">> >& <<EOF", 1),
        (b"> << EOF", 1),
        (b"; <<EOF", 1),
        (b"& <<EOF", 1),
        (b"&& <<EOF", 1),
        (b"( <<EOF", 1),
        (b"cat ; > <<EOF", 1),
        (b">| x <<EOF", 1),
        (b"> <<EOF <<B", 2),
        (b"> <<EOF && cat <<A", 2),
        // The word-0 family: the run start is a word start, a pipeline `|`
        // begins a word, whitespace begins a word, and the line behind a
        // comment begins at its newline — 1 at both levels.
        (b"<<EOF", 1),
        (b"<<EOF x", 1),
        (b"<<EOF >f", 1),
        (b"| <<EOF", 1),
        (b"| <<EOF x", 1),
        (b"  <<EOF", 1),
        (b"  <<-EOF", 1),
        (b"> | <<EOF", 1),
        (b"a|<<EOF", 1),
        (b"cat \"a>\"| <<EOF", 1),
        (b"x\n<<EOF", 1),
        (b"&&\n<<EOF", 1),
        (b"true &&\n<<B", 1),
        (b"# x\n<<EOF", 1),
        (b"<<-EOF", 1),
        (b"3<<EOF", 1),
        (b"<<EOF <<B", 2),
        // The bare `<<` at word 0 is an operator that takes the next word as
        // its delimiter, so `<< <<EOF` is two operators at both levels (the
        // second one's delimiter word is operator-shaped, the parse site's
        // `InvalidRedirect` family — task #261's class 3).
        (b"<< <<EOF", 2),
        // The zeros that stay: `>x<<EOF`/`><<EOF` keep the `<<` inside one
        // word, a quoted `<<` is data, and `true&&<<B` is one word for the
        // tokenizer (fdshell never splits a word at an operator byte — the
        // glued-pair family).
        (b">x<<EOF", 0),
        (b"><<EOF", 0),
        (b"> \"<<EOF\"", 0),
        (b"x<<EOF", 0),
        (b"a<<EOF", 0),
        (b"true&&<<B", 0),
        (b">>x<<EOF", 0),
        // Whitespace leading: both `<<` begin words (the first at the
        // whitespace break, the second at the space after `EOF`) — 2, where the
        // pre-#345 rule counted 1 (`is_operator`'s word-0 arm).
        (b"  <<EOF <<B", 2),
        // Multi-statement agreements: `;`/`&&`/`|` split the line into runs, and
        // each run keeps its own operator at both levels.
        (b"cat <<A ; <<B", 2),
        (b"cat <<A && <<B", 2),
        (b"cat <<A && cat <<B", 2),
        (b"cat <<A | tr <<B", 2),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(byte, want, "line={:?}", line);
    }
}

/// The byte-level and token-level counts agree on the quote-removed delimiter
/// words, including the words that contain a `<<` (row 9 of the task's table:
/// the byte scan must consume a `"…"` span whole like the `'…'` span, or it
/// counts a phantom second operator) (task #207, LESSONS). The **bare quoted
/// empty pair** (task #330) is the count twin of the empty-arm fix: the byte
/// scan always counted it (its raw span is non-empty), and the token layer
/// used to bail on it — the counts now agree, so the region allocation the
/// byte level claims is the one the parser fills.
#[test]
fn operator_count_quoted_delimiter_forms_agree() {
    for (line, want) in [
        (b"cat <<a'b'c" as &[u8], 1usize),
        (b"cat <<'x'y'z'", 1),
        (b"cat <<ab'c'", 1),
        (b"cat <<'a'bc", 1),
        (b"cat <<- a'b'c", 1),
        (b"cat <<a\"b\"c", 1),
        (b"cat <<\"\"Q\"x\"", 1),
        (b"cat << \"\"", 1),
        (b"cat <<- \"\"", 1),
        (b"cat << ''", 1),
        (b"cat 3<< \"\"", 1),
        (b"cat << \"\" && cat << \"\"", 2),
        (b"cat <<\"a b\"", 1),
        (b"cat <<a\"b c\"d", 1),
        (b"cat <<a\\\"b", 1),
        (b"cat <<'a <<b'", 1),
        (b"cat <<\"a <<b\"", 1),
        (b"cat << 'a <<b'", 1),
        (b"cat <<-\"a <<b\"", 1),
        (b"cat <<a'b'c && cat <<d'e'f", 2),
        (b"cat <<\"a <<b\" | tr <<\"c <<d\"", 2),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(byte, want, "line={:?}", line);
    }
}

/// Class 1 of task #261: the pipeline-pipe predecessor test at both levels.
/// An escaped `\|` and a quoted `"|"`, `'|'` are **word** tokens whose text is
/// `|`, so the `<<` behind them is an operator — `is_operator` reads the
/// predecessor token's raw span (`parse::redirect::pipe_token_span`), and the
/// byte scan reads the mask + escape walk (`parse::redirect::pipeline_pipe`).
/// Before the fix the escaped pair read 0/1 and the quoted pair 1/0: the same
/// token-text-vs-raw-bytes trap in both directions.
///
/// Task #261 pinned the real pipeline pipe at **0** (the word behind `|` is
/// the stage's command word). Task #345 moved that row to 1: the stage behind
/// a pipeline pipe is `simple_command = REDIRECT…`, a redirections-only command
/// fed by the body. The pipe **split** rule is untouched — these lines are
/// still pipelines at both levels.
#[test]
fn operator_count_pipe_predecessors_agree() {
    for (line, want) in [
        (b"| <<EOF".as_slice(), 1usize),
        (b"> | <<EOF", 1),
        (b"a >| b | <<EOF", 1),
        (b"cat \"a>\"| <<EOF", 1),
        (b"cat x\\>| <<EOF", 1),
        (b"\\| <<EOF", 1),
        (b"x\\| <<EOF", 1),
        (b"echo x\\| <<EOF", 1),
        (b"\"|\" <<EOF", 1),
        (b"'|' <<EOF", 1),
        (b"echo \"|\" <<EOF", 1),
        (b"echo '|' <<EOF", 1),
        (b"cat x\\>a>| <<EOF", 1),
        (b"echo x\\| <<EOF\nbody\nEOF", 1),
        (b"echo \"|\" <<EOF\nbody\nEOF", 1),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(byte, want, "line={:?}", line);
    }
}

/// Class 3 of task #261: an invalid/missing delimiter word never changes the
/// **count**. `is_operator` has no delimiter test, so the token layer counts
/// every `<<` (the trailing `<<` of `>> << <<EOF` included, and it bails
/// `InvalidRedirect` at the parse site), while the byte scan used to map a
/// `delimiter_span` `None` to a count of 0 for the whole window. The region
/// layer keeps its `None` (`scan/heredoc/ops/tests.rs::count_counts_sites_delims_keeps_none`),
/// so no body region is claimed on these lines and every error path stays as
/// pinned (`safe/fdshell/tests/heredoc.rs::heredoc_invalid_delimiter_*`).
#[test]
fn operator_count_invalid_delimiter_agree() {
    for (line, want) in [
        (b">> << <<EOF".as_slice(), 2usize),
        (b"cat <<- <<EOF", 2),
        (b"cat << <x", 1),
        (b"cat << &x", 1),
        (b"cat << %x", 1),
        (b"cat << \"<x\"", 1),
        (b"cat <<- <<EOF ; cat <<B", 3),
        (b"cat <<- <<EOF && cat <<B", 3),
        (b">> << <<EOF && cat <<B", 3),
        (b"cat <<A ; cat <<- <<X ; cat <<B", 4),
        (b"if true; then cat <<- <<X; fi", 2),
        (b"cat <<- <<X; fi", 2),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(byte, want, "line={:?}", line);
    }
}

/// Class 4 of task #261: a newline is a separator token (`token/step.rs`
/// emits `;` for `\n` and `)`, and no token for ` `/`\t`), so a
/// newline-leading run has a preceding token and its `<<` is an operator at
/// both levels. Task #345 moved the space/tab/run-start rows to 1 as well:
/// every one of those bytes (and the window start itself) is a **word break**,
/// and word 0 of a redirections-only statement is the operator.
#[test]
fn operator_count_leading_newline_agree() {
    for (line, want) in [
        (b"\n<<EOF".as_slice(), 1usize),
        (b"\n<<EOF <<B", 2),
        (b"\n<<EOF\nbody\nEOF", 1),
        (b"true &&\n<<B\nbody\nB", 1),
        (b"x\n<<EOF\nbody\nEOF", 1),
        (b"&&\n<<EOF", 1),
        (b"<<EOF", 1),
        (b" <<EOF", 1),
        (b"\t<<EOF", 1),
        (b"# x\n<<EOF", 1),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(byte, want, "line={:?}", line);
    }
    // The agreement holds per window, and an end-of-file body line sits behind
    // the window's operator (task #364): the byte scan sees both sites of
    // `cat <<A\n<<B` (2), the tokenizer sees one operator and one blanked body
    // line (1). The parse layer follows the token layer, and bash agrees — the
    // `<<B` line is body data, printed raw with the warning at rc 0
    // (measured 2026-10-29).
    let line = b"cat <<A\n<<B";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(crate::scan::heredoc::operator_count(line, 0, line.len()), 2);
    assert_eq!(operator_count(line, &tokens), 1);
}

/// Class 2 of task #261: the **pinned divergence**, asserted as a divergence.
/// The `(( … ))` and `$( … )` regions are opaque to the byte scan
/// (`ScanState::advance` consumes `((`/`$(` whole and the `bare` gate hides
/// everything inside), and that opacity is load-bearing: `x=$(cat <<EOF … )`
/// runs only because the byte scan ignores the `<<` inside the substitution,
/// and counting the arithmetic `<<` of `(( 1 << 2 )) && cat <<B` would empty
/// the line's region stream and steal `cat <<B`'s body. The token layer
/// counts those `<<` (they are ordinary tokens) and nothing observable
/// depends on it: `((expr)`-with-trailing-words is intercepted in
/// `parse/arith_cmd` before any heredoc allocation. So these pairs are
/// pinned byte≠token, and a future "alignment" must re-read this test.
#[test]
fn arith_region_count_divergence_is_pinned() {
    for (line, byte, token) in [
        (b"(( <<EOF".as_slice(), 0usize, 1usize),
        (b"(( 1 <<EOF", 0, 1),
        (b"(( x <<EOF", 0, 1),
        (b"(( 1 << 2 ))", 0, 1),
        (b"(( <<EOF\nbody\nEOF", 0, 1),
        // The line the region-corruption argument rests on: the arith `<<`
        // counted at the byte level would claim `2` as its delimiter, find no
        // `2` line, and leave `cat <<B` without a body.
        (b"(( 1 << 2 )) && cat <<B", 1, 2),
        (b"(( 1 << 2 )); cat <<B\nbody\nB".as_slice(), 1, 2),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        assert_eq!(
            crate::scan::heredoc::operator_count(line, 0, line.len()),
            byte,
            "line={:?}",
            line
        );
        assert_eq!(operator_count(line, &tokens), token, "line={:?}", line);
    }
    // The agreements of the same family: `((1<<2))` is one word at both levels
    // (`io::word_start` rejects a digit run glued to a longer word), and a
    // substitution is one token, so its `<<` counts 0 at both levels.
    for (line, want) in [
        (b"((1<<2))".as_slice(), 0usize),
        (b"echo $((1<<2))", 0),
        (b"x=$(cat <<EOF\nbody\nEOF\n)", 0),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(byte, want, "line={:?}", line);
    }
}

#[test]
fn is_operator_bounds_guard() {
    let line = b"cat <<EOF";
    let tokens = tokenize_statement(line).unwrap();
    assert!(!is_operator(line, &tokens, 0));
    assert!(!is_operator(line, &tokens, tokens.len() + 1));
}

#[test]
fn delimiter_indices_bare_form_takes_next_token() {
    let line = b"cat << {a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(delimiter_token_indices(line, &tokens), vec![2]);
}

#[test]
fn delimiter_indices_attached_form_is_operator_token() {
    let line = b"cat <<{a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(delimiter_token_indices(line, &tokens), vec![1]);
}

#[test]
fn delimiter_indices_dash_bare_form_takes_next_token() {
    let line = b"cat <<- {a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(delimiter_token_indices(line, &tokens), vec![2]);
}

#[test]
fn delimiter_indices_dash_attached_form_is_operator_token() {
    let line = b"cat <<-{a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(delimiter_token_indices(line, &tokens), vec![1]);
}

#[test]
fn delimiter_indices_bare_form_at_end_of_input() {
    // A bare `<<` with no following word protects nothing.
    let line = b"cat <<";
    let tokens = tokenize_statement(line).unwrap();
    assert!(delimiter_token_indices(line, &tokens).is_empty());
}

/// POSIX #4.1: the delimiter word folds its unquoted escape pairs, so
/// `<<E\OF` delimits `EOF` and the delimiter line `EOF` ends the body.
/// POSIX XCU 2.3: that same pair is a quote operator, so the body is literal
/// (task #263 — `expand` is false, exactly like `<<"EOF"`).
#[test]
fn layout_escaped_delimiter_folds() {
    let s = specs(b"cat <<E\\OF\n$X\nEOF\n");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"$X\n".to_vec()).unwrap());
    assert!(!s[0].expand);
    // A delimiter line keeps its raw bytes, so `E\OF` never matches the folded
    // delimiter `EOF` — and never can, so the heredoc is the end-of-file shape
    // and the line is body data (task #364). bash prints `body` and `E\OF` raw
    // with its warning at rc 0 (measured 2026-10-29).
    let s = specs(b"cat <<EOF\nbody\nE\\OF\n");
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"body\nE\\OF\n".to_vec()).unwrap()
    );
    assert!(s[0].expand);
}

/// A quoted delimiter keeps the pair inside the quotes, so `<<"E\OF"`
/// delimits the literal `E\OF` and the body is literal.
#[test]
fn layout_quoted_escaped_delimiter_is_literal() {
    let s = specs(b"cat <<\"E\\OF\"\nbody\nE\\OF\n");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

/// POSIX XCU 2.3: a delimiter word that contains any quote operator is quoted,
/// and `\` is one — so `<<\E\O\F`, whose word has no `"`/`'` at all, makes the
/// body literal while the word still folds to the delimiter `EOF` (task #263).
#[test]
fn layout_backslash_delimiter_is_literal() {
    for line in [
        b"cat <<\\E\\O\\F\n$X\nEOF\n" as &[u8],
        b"cat <<E\\OF\n$X\nEOF\n",
        b"cat <<EO\\F\n$X\nEOF\n",
        b"cat <<\\EOF\n$X\nEOF\n",
    ] {
        let s = specs(line);
        assert_eq!(s.len(), 1, "line={:?}", line);
        assert_eq!(
            s[0].body,
            ShortCStr::from_vec(b"$X\n".to_vec()).unwrap(),
            "line={:?}",
            line
        );
        assert!(
            !s[0].expand,
            "a `\\` in the word quotes the body: line={:?}",
            line
        );
    }
}

/// A quoted body is verbatim bytes on both counts: `$`-expansion is off
/// (`expand == false`) and no `\<newline>` continuation is stripped from it
/// (the same `op.quoted` gate drives both, `parse/heredoc.rs`).
#[test]
fn layout_escaped_delimiter_keeps_the_body_verbatim() {
    for (line, want) in [
        (
            b"cat <<E\\OF\n$(echo hi)\nEOF\n" as &[u8],
            b"$(echo hi)\n" as &[u8],
        ),
        (b"cat <<E\\OF\n`echo hi`\nEOF\n", b"`echo hi`\n"),
        (b"cat <<E\\OF\n\\$X\nEOF\n", b"\\$X\n"),
        (b"cat <<E\\OF\na\\\nb\nEOF\n", b"a\\\nb\n"),
    ] {
        let s = specs(line);
        assert_eq!(s.len(), 1, "line={:?}", line);
        assert_eq!(
            s[0].body,
            ShortCStr::from_vec(want.to_vec()).unwrap(),
            "line={:?}",
            line
        );
        assert!(!s[0].expand, "line={:?}", line);
    }
}

/// The `<<-` form keeps stripping tabs with an escape-quoted delimiter: the
/// flag is derived in the fold, independent of `strip`.
#[test]
fn layout_dashed_escaped_delimiter_is_literal_and_stripped() {
    let s = specs(b"cat <<-E\\OF\n\t$X\n\tEOF\n");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"$X\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

/// The bare form inherits the marker and quotes the body the same way: the
/// delimiter word is read raw at the token level too (`operator_at`).
#[test]
fn layout_bare_escaped_delimiter_is_literal() {
    for line in [
        b"cat << E\\OF\n$X\nEOF\n" as &[u8],
        b"cat <<- E\\OF\n\t$X\n\tEOF\n",
        b"cat << \\E\\O\\F\n$X\nEOF\n",
    ] {
        let s = specs(line);
        assert_eq!(s.len(), 1, "line={:?}", line);
        assert_eq!(
            s[0].body,
            ShortCStr::from_vec(b"$X\n".to_vec()).unwrap(),
            "line={:?}",
            line
        );
        assert!(!s[0].expand, "line={:?}", line);
    }
}

/// Two escape-quoted delimiters of one command are two operators, both quoted
/// (the per-operator flag is not diluted by the neighbour).
#[test]
fn layout_two_escaped_delimiters_of_one_command_are_literal() {
    let s = specs(b"cat <<A\\X <<B\\Y\n$X\nAX\n$X\nBY\n");
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"$X\n".to_vec()).unwrap());
    assert_eq!(s[1].body, ShortCStr::from_vec(b"$X\n".to_vec()).unwrap());
    assert!(!s[0].expand);
    assert!(!s[1].expand);
}

/// An escaped `$` in the delimiter word folds to the literal `$HOME` delimiter
/// and quotes the body: the body's `$( … )` is never run (task #263).
#[test]
fn layout_escaped_dollar_delimiter_is_literal() {
    let s = specs(b"cat <<\\$HOME\n$(echo hi)\n$HOME\n");
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"$(echo hi)\n".to_vec()).unwrap()
    );
    assert!(!s[0].expand);
}

/// POSIX #4.1: the escape pair `\<` folds to a literal `<`, so `echo a\<<X`
/// is one word with no operator (the byte-level scan agrees, `ops.rs`).
#[test]
fn layout_escaped_operator_byte_is_not_an_operator() {
    let line = b"echo a\\<<X";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(line, &tokens), 0);
    assert!(specs(line).is_empty());
    // A real operator after an escaped `<` is still found.
    let line = b"echo a\\< <<X";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(line, &tokens), 1);
}

/// POSIX #4.2: in an expandable body a `\<newline>` pair is a line
/// continuation, removed in the parse layer so the body's substitution (a
/// quoted context, which keeps `\<char>`) never sees it.
#[test]
fn layout_strips_body_line_continuations() {
    let s = specs(b"cat <<EOF\na\\\nb\nEOF\n");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"ab\n".to_vec()).unwrap());
    assert!(s[0].expand);
    // `\\` before the newline is a pair, so the newline is body data.
    let s = specs(b"cat <<EOF\na\\\\\nb\nEOF\n");
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"a\\\\\nb\n".to_vec()).unwrap()
    );
}

/// A quoted body is verbatim bytes: no continuation is stripped from it.
#[test]
fn layout_quoted_body_keeps_its_continuation_pair() {
    let s = specs(b"cat <<'EOF'\na\\\nb\nEOF\n");
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"a\\\nb\n".to_vec()).unwrap()
    );
    assert!(!s[0].expand);
}

/// The io-number form is one operator at both levels with the prefix as its
/// descriptor (`3<<T` → fd 3, `003<<T` → fd 3); every word-data shape whose
/// *text* reads like an operator (`3"<<"T`, `"3"<<T`, `\3<<T`, `x3<<T`,
/// `>3<<T`, `x<<T`) counts 0 at both levels, because `is_operator` tests the
/// token's raw span through the shared `span_operator` (the byte/token
/// agreement rule, LESSONS; task #276). The io-number at a **run start**
/// (`3<<T`, ` 3<<T`) is word 0 of a redirections-only statement: 1 with its fd
/// at both levels (task #345).
#[test]
fn operator_count_io_number_forms_agree() {
    for (line, want) in [
        (b"cat 3<<T".as_slice(), 1usize),
        (b"cat 0<<T", 1),
        (b"cat 003<<T", 1),
        (b"cat 3<<-T", 1),
        (b"cat 3<< T", 1),
        (b"cat 3<<'Q'", 1),
        (b"cat 3<<T 4<<U", 2),
        (b"cat ;3<<T", 1),
        (b"cat 3<<T && cat 4<<U", 2),
        (b"cat 3<<T | tr", 1),
        (b"builtin cat 3<<T", 1),
        (b"3<<T", 1),
        (b" 3<<T", 1),
        (b"cat x<<T", 0),
        (b"cat x3<<T", 0),
        (b"cat -3<<T", 0),
        (b"cat $FD<<T", 0),
        (b"cat \"3\"<<T", 0),
        (b"cat 3\"<<\"T", 0),
        (b"cat 3'<<'T", 0),
        (b"cat \\3<<T", 0),
        (b"cat 3\\\\<<T", 0),
        (b">3<<T", 0),
        (b"cat a|3<<T", 0),
        (b"cat 3<<<x", 0),
        (b"cat <<<x", 0),
        // A digit run behind a *quoted* space sits at a raw-byte word start
        // inside the token's span: `span_operator` only accepts a prefix that
        // *begins* the span, so these stay word data at both levels (0/0).
        // Without that guard the token layer counts an fd-3 operator with a
        // shifted delimiter while the byte scan counts 0 — the exact mismatch
        // the guard exists to prevent (task #276's review, mutant
        // `io.rs: run == start -> true`).
        (b"cat x\" 3<<T\"", 0),
        (b"cat \" 3<<T\"", 0),
        (b"cat x' 3<<T'", 0),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
        assert_eq!(byte, operator_count(line, &tokens), "line={:?}", line);
        assert_eq!(operator_count(line, &tokens), want, "line={:?}", line);
    }
}

/// The operator carries the prefix as its target descriptor, and the attached
/// and bare delimiter rules are unchanged by the prefix: `3<<T` attaches on `T`
/// (extra 0), `3<< T` takes the next word (extra 1), `3<<-T` keeps the marker.
#[test]
fn operator_at_reports_the_prefixed_fd() {
    for (line, index, fd, delim, quoted, extra) in [
        (
            b"cat 3<<T".as_slice(),
            1usize,
            3i32,
            b"T".as_slice(),
            false,
            0usize,
        ),
        (b"cat 0<<T", 1, 0, b"T", false, 0),
        (b"cat 003<<T", 1, 3, b"T", false, 0),
        (b"cat 12 34<<T", 2, 34, b"T", false, 0),
        (b"cat 3<<-T", 1, 3, b"T", false, 0),
        (b"cat 3<< T", 1, 3, b"T", false, 1),
        (b"cat 3<<'Q'", 1, 3, b"Q", true, 0),
        (b"cat 3<<\"Q\"", 1, 3, b"Q", true, 0),
        (b"cat 3<<T 4<<U", 1, 3, b"T", false, 0),
        (b"cat 3<<T 4<<U", 2, 4, b"U", false, 0),
    ] {
        let tokens = tokenize_statement(line).unwrap();
        let (op, got) = operator_at(line, &tokens, index).unwrap();
        assert_eq!(
            (op.fd, op.delim.as_slice(), op.quoted, got),
            (fd, delim, quoted, extra),
            "line={:?}",
            line
        );
    }
    let line = b"cat 3<<T 4<<U";
    let tokens = tokenize_statement(line).unwrap();
    let ops = super::operators(line, &tokens).unwrap();
    assert_eq!(ops.iter().map(|op| op.fd).collect::<Vec<i32>>(), vec![3, 4]);
}

/// The fd rule is the `N>file` rule, so the run that overflows `i32` is a
/// parse error naming the prefix — the byte scan counts it (it is syntactically
/// an operator), and the parse reports it instead of silently writing to fd 0.
#[test]
fn operator_at_rejects_an_io_number_that_is_not_a_descriptor() {
    let line = b"cat 99999999999999999999999<<T";
    let tokens = tokenize_statement(line).unwrap();
    let byte = crate::scan::heredoc::operator_count(line, 0, line.len());
    assert_eq!(byte, operator_count(line, &tokens), "count agreement");
    assert_eq!(
        operator_count(line, &tokens),
        1,
        "syntactically an operator"
    );
    let err = operator_at(line, &tokens, 1).err().unwrap();
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidHeredocFd { .. }
    ));
    // A prefix that fits is an fd, and `2147483647` is the largest one.
    let line = b"cat 2147483647<<T";
    let tokens = tokenize_statement(line).unwrap();
    let (op, _extra) = operator_at(line, &tokens, 1).unwrap();
    assert_eq!(op.fd, i32::MAX);
}

/// The **escaped delimiter word** agrees at both levels (task #347, the shape
/// #349 measured): the region layer's byte walk consumes every `\X` pair whole,
/// so its `Operator`s fold to exactly the token layer's — same delimiter bytes,
/// same quoted/strip flags, same descriptors. `skip`'s pins
/// (`scan/heredoc/tests.rs::skip_escaped_delimiter_word_is_one_word`) show the
/// region is claimed; this is the shape half of the same rule: a byte walk that
/// stepped over the `\` alone would fold a truncated word (`a` for `a\ b`) and
/// the two lists would differ.
#[test]
fn escaped_delimiter_words_fold_the_same_at_both_levels() {
    let shape = |ops: &[crate::scan::heredoc::Operator]| -> Vec<(Vec<u8>, bool, bool, i32)> {
        ops.iter()
            .map(|op| (op.delim.clone(), op.quoted, op.strip, op.fd))
            .collect()
    };
    for line in [
        b"cat <<a\\ b\na b" as &[u8],
        b"cat <<a\\\tb\na\tb",
        b"cat <<\\ b\n b",
        b"cat <<-a\\ b\n\ta b",
        b"cat <<a\\;b\na;b",
        b"cat <<a\\|b\na|b",
        b"cat <<a\\)\na)",
        b"cat <<\\;\n;",
        b"cat << \\;\n;",
        b"cat <<\\|\n|",
        b"cat 3<<\\;\n;",
        b"cat <<\\; <<\\|\n;\n|",
    ] {
        let tokens = tokenize_statement(line).unwrap();
        // The byte layer's window is the command line (the run window the
        // region layer is handed), never the whole text.
        let to = line.iter().position(|&b| b == b'\n').unwrap_or(line.len());
        let byte = shape(&crate::scan::heredoc::operator_delims(line, 0, to).unwrap());
        let token = shape(&super::operators(line, &tokens).unwrap());
        assert!(
            !byte.is_empty(),
            "every row is a claimed operator: {line:?}"
        );
        assert_eq!(byte, token, "line={line:?}");
    }
}
