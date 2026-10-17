//! Tests for the byte-level `<<` operator scan, including `#` comment
//! handling (the two counts — byte-level and token-level — must agree).

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::operator_delims;

/// A `#` comment (outside quotes, at a word start) is skipped whole: a `<<`
/// inside it is not an operator.
#[test]
fn comment_hides_operators() {
    let line = b"cat <<A # <<B\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"A");
}

/// A `#` mid-word is a literal byte, not a comment start: the operator after
/// the word is still found.
#[test]
fn comment_mid_word_is_not_comment() {
    let line = b"echo a#b <<X";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"X");
}

/// A `#` comment with no trailing newline ends the scan: nothing after it on
/// the line is an operator.
#[test]
fn comment_without_newline_ends_scan() {
    let line = b"cat <<A # <<B";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"A");
}

/// The comment skip lands just past the comment's newline: an operator on the
/// next line of the range is still found.
#[test]
fn comment_skip_resumes_on_next_line() {
    let line = b"cat <<A ##\ncat <<B";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].delim, b"A");
    assert_eq!(ops[1].delim, b"B");
}

/// A comment at the run start is skipped whole: the skip lands past its
/// newline, never inside the comment text, so a word-start `<<` in the comment
/// is not an operator.
#[test]
fn comment_at_run_start_skips_its_text() {
    let line = b"# a <<EOF\ncat";
    assert_eq!(operator_delims(line, 0, line.len()).unwrap().len(), 0);
}

/// POSIX #4.1: the escape pair `\<` folds to a literal `<`, so `echo a\<<X`
/// keeps the word `a<<X` and has no operator (the tokenizer agrees: the word
/// is one token, no `<<` is emitted).
#[test]
fn escaped_operator_byte_is_not_an_operator() {
    let line = b"echo a\\<<X";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0, "the `\\<` pair shields the `<`");
    // A real operator after the word is still found.
    let line = b"echo a\\< <<X";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"X");
}

/// A bare delimiter word folds its unquoted escape pairs (`<<E\OF` delimits
/// `EOF`), and a trailing `\` in the delimiter keeps its backslash.
#[test]
fn delimiter_folds_escape_pairs() {
    let line = b"cat <<E\\OF\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"EOF");
    assert!(!ops[0].strip);
    let line = b"cat <<\\!EOF\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"!EOF");
}

/// A quoted delimiter keeps the pair inside the quotes (POSIX #4.2), and the
/// `<<-` marker folds the escape pairs outside the quotes.
#[test]
fn quoted_delimiter_keeps_the_pair() {
    let line = b"cat <<\"E\\OF\"\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"E\\OF");
    assert!(ops[0].quoted);
    let line = b"cat <<-E\\OF\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"EOF");
    assert!(ops[0].strip);
    assert!(!ops[0].quoted);
}

/// The delimiter word is a word at the byte level too: quote removal drops
/// every `"`/`'` byte (not one surrounding pair) and any quoting marks the
/// delimiter quoted, so the body is literal (task #207).
#[test]
fn delimiter_strips_every_quote_byte() {
    for (line, want, strip, quoted) in [
        (b"cat <<a'b'c" as &[u8], b"abc" as &[u8], false, true),
        (b"cat <<'x'y'z'", b"xyz", false, true),
        (b"cat <<ab'c'", b"abc", false, true),
        (b"cat <<'a'bc", b"abc", false, true),
        (b"cat <<- a'b'c", b"abc", true, true),
        (b"cat <<a\"b\"c", b"abc", false, true),
        (b"cat <<\"\"Q\"x\"", b"Qx", false, true),
        (b"cat <<\"a b\"", b"a b", false, true),
        (b"cat <<a\"b c\"d", b"ab cd", false, true),
        (b"cat << 'a b'", b"a b", false, true),
        // An escaped quote is data: the delimiter quotes nothing, so the body
        // expands (`<<a\"b` — task #263 owns the `\`-quotes-the-delimiter rule).
        (b"cat <<a\\\"b", b"a\"b", false, false),
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(ops.len(), 1, "line={:?}", line);
        assert_eq!(ops[0].delim, want, "line={:?}", line);
        assert_eq!(ops[0].strip, strip, "line={:?}", line);
        assert_eq!(ops[0].quoted, quoted, "line={:?}", line);
    }
}

/// A quoted delimiter word that contains `<<` is ONE word: the byte scan
/// consumes a `"…"` span whole, exactly like the `'…'` span, so it finds one
/// operator and not two (the byte/token agreement rule, LESSONS).
#[test]
fn quoted_delimiter_containing_a_chevron_is_one_operator() {
    for line in [
        b"cat <<'a <<b'" as &[u8],
        b"cat <<\"a <<b\"",
        b"cat << 'a <<b'",
        b"cat <<-\"a <<b\"",
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(ops.len(), 1, "line={:?}", line);
        assert_eq!(ops[0].delim, b"a <<b" as &[u8], "line={:?}", line);
        assert!(ops[0].quoted, "line={:?}", line);
        assert_eq!(
            ops[0].strip,
            line.starts_with(b"cat <<-"),
            "line={:?}",
            line
        );
    }
}

/// POSIX #2.2 `>|`: the `|` absorbed into the clobber operator is an operator
/// byte, so it neither breaks a word nor forms a pipeline position. The
/// token-level agreement of these lines is asserted in
/// `parse/heredoc/tests.rs` (the byte scan cannot reach the token layer).
#[test]
fn clobber_pipe_is_an_operator_byte_not_a_pipeline() {
    // `cat`, `>|`, `q`, `<<EOF` — the `<<` word starts at the space after `q`.
    let line = b"cat >| q <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // The `<<` word starts at the space after the absorbed `>|` token, and the
    // absorbed `|` is not a pipeline position.
    let line = b"cat >| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // `a>|<<EOF` is one token: no word-break byte precedes the `<<`.
    let line = b"echo a>|<<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0);

    // A pipeline pipe that is not `>`-preceded keeps `<<` a command word.
    let line = b"a | <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0);

    // The `k == 0` edge: a `|` at the run start (with only whitespace between
    // it and the `<<`) is a pipeline position, so the `>`-preceded test must
    // not underflow.
    let line = b"| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0);

    // A `>|` at the run start: the absorbed `|` is not a pipeline position (the
    // `k == 0` guard is not reached, and `clobber_pipe` sees the `>` at k-1),
    // and the `>` is a preceding *token*, so the `<<` counts 1 — the same count
    // the token layer gives (task #185: the byte scan mirrors
    // `parse::heredoc::is_operator`'s "a preceding token that is not a pipeline
    // `|`", not "a command word has been seen").
    let line = b">| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // The shared rule reads the word's raw bytes, so a quoted `>` does not
    // terminate an operator word: `cat "a>"| <<EOF` counts 0 at both levels
    // (`echo "a>"|b` is a pipeline on master and bash, rc 1 and 127), and with
    // a word after the pipe the `<<` is an operator: `echo "a>"|b <<EOF` counts 1.
    let line = b"cat \"a>\"| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0, "the quoted `>` is not an operator byte");
    let line = b"echo \"a>\"|b <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // An escape pair shields its second byte, and an unquoted operator byte
    // earlier in the word disqualifies it, so neither word is `>`-terminated
    // and the `|` is a pipeline position: 0 at both levels (bash is a syntax
    // error at `<<`, rc 2; master measures rc 1 for both lines).
    let line = b"cat x\\>| <<EOF";
    assert_eq!(operator_delims(line, 0, line.len()).unwrap().len(), 0);
    let line = b"cat a>b>| <<EOF";
    assert_eq!(operator_delims(line, 0, line.len()).unwrap().len(), 0);
    // A word that carries an escape pair and ends at a real `>` is
    // `>`-terminated, so the `|` is absorbed and the `<<` counts 1 (the token
    // layer counts 1 for the same line).
    let line = b"cat x\\>a>| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // The mask is read from the line start, not from the word's start: the word
    // here begins with the closing `"`, and the quoted `>` inside `"a>"` is not
    // an operator byte, so `x\y>` terminates the word and the `|` is absorbed
    // (count 1 at both levels). A word-local mask scan would read that closing
    // quote as opening and count 0 at the byte level.
    let line = b"cat \"a>\"x\\y>| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // The escape pair is skipped only outside quotes: inside `"a\>"` the `>` is
    // quoted, so the word runs to the real `>` after `x` and the `|` is
    // absorbed (count 1 at both levels). Skipping the pair inside quotes would
    // make the quoted `>` an unquoted operator byte and count 0.
    let line = b"cat \"a\\>\"x>| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // A `%`-leading capture word keeps its own absorption, so the earlier `>`
    // does not disqualify it and the `|` is absorbed: count 1 at both levels.
    // The word's first byte is what the `%` rule reads, so a scan that starts
    // one byte late loses the `%` and counts 0.
    let line = b"cat %>>| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");
}

/// A run that opens with a redirect, `;`, `&` or an escape pair counts its
/// `<<`: the guard is a preceding *token*, not a preceding *word* —
/// `is_word_break` ends the word at `>`, so a word-seen flag can never
/// express it. Mirrors `parse::heredoc::is_operator` (`i != 0 && prev != b"|"`);
/// the token-level agreement is pinned in `parse/heredoc/tests.rs` (task #185).
/// The zeros are the run-start `<<`, the pipeline pipe, the bytes inside one
/// word, and the runs whose leading bytes are only whitespace — whitespace
/// never sets the flag, so those are the runs that pin `is_token_byte` down.
/// `to` is the full run: a truncated `to` makes `delimiter_span` return `None`
/// and the scan reports no operators at all.
#[test]
fn redirect_operator_leading_run_counts_the_operator() {
    for (line, want) in [
        (b"> <<EOF" as &[u8], 1usize),
        (b">> <<EOF", 1),
        (b">& <<EOF", 1),
        (b"<> <<EOF", 1),
        (b"< <<EOF", 1),
        (b"; <<EOF", 1),
        (b"& <<EOF", 1),
        (b"\\< <<EOF", 1),
        (b">> >& <<EOF", 1),
        (b"> << EOF", 1),
        (b"> <<-EOF", 1),
        // A `<<` at the run start is the command word, a pipeline `|` before it
        // is a stage boundary, and `><<EOF`/`>x<<EOF` keep the `<<` inside one
        // word: no operator at either level.
        (b"<<EOF", 0),
        (b"<<EOF x", 0),
        (b"> | <<EOF", 0),
        (b"| <<EOF", 0),
        (b"><<EOF", 0),
        (b">x<<EOF", 0),
        // Whitespace never sets the flag: a run whose leading bytes are only
        // whitespace has no preceding token, so its `<<` is the command word
        // (`is_operator`'s `i == 0` arm). These are the runs that kill the
        // `is_token_byte -> true` / `delete !` / `&& -> ||` mutants — every
        // pin with a token before the `<<` has the flag set by that token
        // alone, so it cannot see a mutation that makes *every* byte a token
        // byte (task #185's review).
        (b" <<EOF", 0),
        (b"  <<EOF", 0),
        (b"\t<<EOF", 0),
        (b"  <<EOF x", 0),
        (b"  <<-EOF", 0),
        (b"   | <<EOF", 0),
    ] {
        let found = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(found.len(), want, "line={:?}", line);
    }

    // The `<<-` form keeps its tab-stripping marker.
    let line = b"> <<-EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"EOF");
    assert!(ops[0].strip);

    // A `<<-` behind leading whitespace and a real token keeps the marker too:
    // the whitespace sets no flag, the `>` is the preceding token.
    let line = b"  > <<-EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");
    assert!(ops[0].strip);
}

/// The io-number form of the operator: the digit run that begins the operator's
/// word is its target descriptor, and the delimiter rules (`<<-`, the bare
/// form, quoting) are unchanged by it. Every shape whose bytes only *look*
/// numbered — a digit run glued to a longer word (`x3<<T`, `>3<<T`), a quoted
/// prefix (`"3"<<T`), a quoted operator (`3"<<"T`), an escaped digit
/// (`\3<<T`), the pipeline position (`a|3<<T`) — is word data: 0 operators,
/// exactly as the token layer counts (the byte/token agreement rule).
/// `cat 99999999999999999999999<<T` counts 1 with fd 0: the count rule is
/// syntactic, and `parse::heredoc::operator_at` is what reports the fd.
#[test]
fn io_number_prefixed_operators_carry_their_fd() {
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
        (b"cat 2147483647<<T", 1),
        (b"cat 99999999999999999999999<<T", 1),
        (b"3<<T", 0),
        (b" 3<<T", 0),
        (b"\t3<<T", 0),
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
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(ops.len(), want, "line={:?}", line);
    }

    for (line, fd, delim, strip, quoted) in [
        (b"cat 3<<T".as_slice(), 3i32, b"T".as_slice(), false, false),
        (b"cat 0<<T", 0, b"T", false, false),
        (b"cat 003<<T", 3, b"T", false, false),
        (b"cat 12 34<<T", 34, b"T", false, false),
        (b"cat 3<<-T", 3, b"T", true, false),
        (b"cat 3<< T", 3, b"T", false, false),
        (b"cat 3<<'Q'", 3, b"Q", false, true),
        (b"cat ;3<<T", 3, b"T", false, false),
        (b"cat 2147483647<<T", i32::MAX, b"T", false, false),
        // Syntactically an operator with an unrepresentable fd: the byte scan
        // reports fd 0 and the parse layer names the prefix.
        (b"cat 99999999999999999999999<<T", 0, b"T", false, false),
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(ops.len(), 1, "line={:?}", line);
        assert_eq!(
            (
                ops[0].fd,
                ops[0].delim.as_slice(),
                ops[0].strip,
                ops[0].quoted
            ),
            (fd, delim, strip, quoted),
            "line={:?}",
            line
        );
    }

    // Two numbered operators on one line keep their own descriptors.
    let line = b"cat 3<<T 4<<U";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(
        ops.iter().map(|op| op.fd).collect::<alloc::vec::Vec<i32>>(),
        alloc::vec![3, 4]
    );
}
