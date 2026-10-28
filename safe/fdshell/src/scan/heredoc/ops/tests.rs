//! Tests for the byte-level `<<` operator scan, including `#` comment
//! handling (the two counts — byte-level and token-level — must agree).

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::operator_delims;
use super::{operator_count, pipe};

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

/// A comment whose `#` sits at word-start index `i = 1` with its newline the
/// very next byte (`p = 1`): the skip lands on `i + p + 1 = 3`, the first byte
/// of the next line, and that line's operator counts 1. The row pins the
/// comment skip at `i > 0, p > 0`, which no index-0 row covers; it kills no
/// mutant, because the newline search starts *at* the `#` so `p >= 1` always,
/// and the multiplied skip computes `i * p + 1 = 2` — the comment's own
/// newline, from which the loop resumes at 3 with identical state
/// (`ScanState::advance` is state-neutral at a newline). The comment mutants
/// are killed by the index-0 rows; `ops.rs:36:38` is the accepted equivalent
/// (LESSONS).
#[test]
fn comment_at_index_one_with_next_line_newline_resumes_the_scan() {
    let line = b" #\ncat <<A\nb\nA\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"A");
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
    assert!(ops[0].quoted, "the folded pair quotes the word");
    assert!(!ops[0].strip);
    let line = b"cat <<\\!EOF\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"!EOF");
    assert!(ops[0].quoted, "the folded pair quotes the word");
}

/// A quoted delimiter keeps the pair inside the quotes (POSIX #4.2), and the
/// `<<-` marker folds the escape pairs outside the quotes — the folded pair
/// quotes the word, exactly like the plain form (task #263).
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
    assert!(ops[0].quoted);
}

/// The delimiter word is a word at the byte level too: quote removal drops
/// every `"`/`'` byte (not one surrounding pair) and any POSIX quote operator
/// marks the delimiter quoted, so the body is literal (task #207; the escaped
/// pair's own quote-operator half is task #263's).
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
        // An escaped quote is data in the *text*: the delimiter is the literal
        // `a"b`. The pair that made it is a quote operator, so the word is
        // quoted and the body literal (POSIX XCU 2.3, task #263).
        (b"cat <<a\\\"b", b"a\"b", false, true),
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(ops.len(), 1, "line={:?}", line);
        assert_eq!(ops[0].delim, want, "line={:?}", line);
        assert_eq!(ops[0].strip, strip, "line={:?}", line);
        assert_eq!(ops[0].quoted, quoted, "line={:?}", line);
    }
}

/// A delimiter word quoted only by escape pairs is ONE word at the byte level
/// (the pairs cut no word span: `<<\E\O\F` is one delimiter, not five) and is
/// quoted, because POSIX XCU 2.3 counts `\` a quote operator (task #263).
#[test]
fn delimiter_of_a_backslash_word_is_one_operator_and_quoted() {
    for (line, want, strip) in [
        (b"cat <<\\E\\O\\F" as &[u8], b"EOF" as &[u8], false),
        (b"cat <<E\\OF", b"EOF", false),
        (b"cat <<-E\\OF", b"EOF", true),
        (b"cat << E\\OF", b"EOF", false),
        (b"cat <<\\$HOME", b"$HOME", false),
        (b"cat <<\\\\EOF", b"\\EOF", false),
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(
            ops.len(),
            1,
            "a `\\` pair never cuts the word: line={:?}",
            line
        );
        assert_eq!(ops[0].delim, want, "line={:?}", line);
        assert_eq!(ops[0].strip, strip, "line={:?}", line);
        assert!(
            ops[0].quoted,
            "a folded pair quotes the delimiter: line={:?}",
            line
        );
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

    // A pipeline pipe that is not `>`-preceded is a stage boundary: it ends the
    // previous word and begins the `<<`'s own word, so the operator counts 1 —
    // the second stage is a redirections-only command (task #345).
    let line = b"a | <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);

    // The `k == 0` edge: a `|` at the run start (with only whitespace between
    // it and the `<<`) is a pipeline position, so the `>`-preceded test must
    // not underflow; the `<<` it begins counts 1.
    let line = b"| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);

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
    // terminate an operator word: the `|` of `cat "a>"| <<EOF` is a real
    // pipeline pipe, and it begins the `<<` word, so the count is 1 at both
    // levels (`echo "a>"|b` is a pipeline on master and bash, rc 1 and 127; the
    // pipeline's last stage being here-doc-only is task #345's shape). With a
    // word after the pipe the `<<` is an ordinary stage operator: 1 as well.
    let line = b"cat \"a>\"| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(
        ops.len(),
        1,
        "the quoted `>` is not an operator byte, so the `|` is a pipeline pipe and begins the `<<` word"
    );
    let line = b"echo \"a>\"|b <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // An escape pair shields its second byte, and an unquoted operator byte
    // earlier in the word disqualifies it, so neither word is `>`-terminated
    // and the `|` is a pipeline pipe that begins the `<<` word: 1 at both
    // levels (bash is a syntax error at `<<`, rc 2; fdshell measures rc 1 for
    // both lines, the operator-token-leading family).
    let line = b"cat x\\>| <<EOF";
    assert_eq!(operator_delims(line, 0, line.len()).unwrap().len(), 1);
    let line = b"cat a>b>| <<EOF";
    assert_eq!(operator_delims(line, 0, line.len()).unwrap().len(), 1);
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

/// The site gate is "**the `<<` begins a word**", and a word begins at the run
/// start, after whitespace, `;`, a newline, a top-level `)` or a pipeline `|`
/// (task #345: POSIX's `simple_command = REDIRECT… [word]` makes `<<EOF` a
/// command of its own, and a run that opens with a redirect, `;`, `&` or an
/// escape pair opens the `<<` word all the same). Mirrors
/// `parse::heredoc::is_operator` (the token's raw span, its predecessor not a
/// pipe token); the agreement is pinned in `parse/heredoc/tests.rs` (task #185
/// pinned the mirror, task #345 moved the word-0 arm to the operator side).
/// The zeros are the `<<` glued inside one word (`><<EOF`, `>x<<EOF`) and the
/// `|` absorbed into a `>|` clobber. `to` is the full run: a truncated `to`
/// makes `delimiter_span` return `None` and the scan reports no operators at
/// all.
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
        // The run start is a word start (`<<EOF` is a redirections-only
        // statement), a pipeline `|` begins a word, and so does a `;`-leading
        // run: 1. `><<EOF`/`>x<<EOF` keep the `<<` inside one word: 0.
        (b"<<EOF", 1),
        (b"<<EOF x", 1),
        (b"> | <<EOF", 1),
        (b"| <<EOF", 1),
        (b"><<EOF", 0),
        (b">x<<EOF", 0),
        // Leading whitespace is a word break, not a token: the `<<` begins a
        // word, so the count is 1. These rows are the `word_start` arm's kill
        // pins — a run whose leading bytes are only whitespace has the break
        // come from `word_start`, never from the run-start arm, so a mutation
        // of the whitespace arms is visible here (task #185's review asked for
        // exactly that separation).
        (b" <<EOF", 1),
        (b"  <<EOF", 1),
        (b"\t<<EOF", 1),
        (b"  <<EOF x", 1),
        (b"  <<-EOF", 1),
        (b"   | <<EOF", 1),
        // The glued io-number family: `x3<<EOF`/`>3<<EOF` are one word, so the
        // io-number is never read and the `<<` is word data (0 at both levels).
        (b"x3<<EOF", 0),
        (b">3<<EOF", 0),
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
/// The io-number at a **run start** (`3<<T`, ` 3<<T`) counts 1 and carries its
/// fd (task #345: word 0 of a redirections-only statement). The **glued** form
/// (`cat&&3<<T`) counts 0 on the physical line — `&` is a word byte for the
/// tokenizer, so `io_prefix`'s narrow word rule rejects the run — **but 1 in
/// its part window**: the cond splitter cuts right after the `&` of the glued
/// pair, the window opens on the operator, and the window-relative `run ==
/// from` arm counts a site the line never counted. The part then asks for a
/// body the line's region list never claimed and the parse layer reports the
/// false `missing terminating delimiter line 'T'` (rc 1, bash rc 0), pinned by
/// `tests/redirect.rs::a_heredoc_word_zero_keeps_its_owner_guards`; the real
/// fix (a word-boundary cut for the glued pair) is the glued-pair family's.
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
        (b"3<<T", 1),
        (b" 3<<T", 1),
        (b"\t3<<T", 1),
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
        // A digit run with a *quoted* space before it is word data: the run is
        // a raw-byte word start inside the quoted span, and the token-level
        // rule rejects a prefix that does not begin the span (the guard in
        // `span_operator`, pinned at the token level by the twin rows in
        // `operator_count_io_number_forms_agree`).
        (b"cat x\" 3<<T\"", 0),
        (b"cat \" 3<<T\"", 0),
        (b"cat x' 3<<T'", 0),
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(ops.len(), want, "line={:?}", line);
    }

    for (line, fd, delim, strip, quoted) in [
        (b"cat 3<<T".as_slice(), 3i32, b"T".as_slice(), false, false),
        (b"3<<T", 3, b"T", false, false),
        (b"0<<T", 0, b"T", false, false),
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

    // The glued io-number: the window opens on the digits (the cond splitter
    // consumed the `&`), and the byte rule reads the physical line, where `&`
    // is a word byte — so the count stays 0 in the window as it is on the
    // whole line. A window-scoped io arm would make the part ask for a body
    // the line's region list never claimed (task #345's review).
    let line = b"cat&&3<<T";
    assert_eq!(operator_count(line, 5, line.len()), 0, "glued io window");
    assert_eq!(operator_count(line, 0, line.len()), 0, "glued io line");
    // A window that opens mid-word keeps the digit run glued to the word:
    // `x3<<T` is word data, so the window counts 0.
    assert_eq!(operator_count(b"catx3<<T", 3, 8), 0, "mid-word window");
}

/// The bare-delimiter guard at the two levels (task #323): the byte level
/// gates on the **raw span's** first byte (`lines::delimiter_span`) and the
/// token level gates on the operand's **quote mask**
/// (`op::operator_word`) — one rule read at each level, so the counts agree.
/// A quoted delimiter (`<< "<x"`) is a plain word: the scan finds one
/// operator with the quote-removed word as its delimiter (the token level
/// counts the same 1 and accepts it —
/// `parse/heredoc/tests.rs::operator_count_byte_and_token_levels_agree` and
/// `layout_bare_quoted_operator_delimiter_is_a_delimiter`). The quoted **empty**
/// pair (`cat << ""`) is the same rule read at each level (task #330): the byte
/// gate asks "is the raw word empty?", and `""` is not; the token gate asks
/// "is the empty text a quoted span?", and `""` is
/// (`parse::word_quoted::word_quoted`) — one operator with an empty delimiter
/// at both levels. An unquoted
/// operator-shaped delimiter is the `None` arm: a bare `<<` with a rejected
/// delimiter word yields no operator scan at all, matching the token level's
/// syntax error
/// (`parse/heredoc/tests.rs::layout_bare_operator_before_redirect_word_is_invalid`)
/// — no body region is claimed on either side.
#[test]
fn bare_quoted_operator_delimiter_counts_one_at_both_levels() {
    for (line, want, delim) in [
        (b"cat << \"<x\"".as_slice(), 1usize, Some(b"<x".as_slice())),
        (b"cat << \"&x\"", 1, Some(b"&x")),
        (b"cat << \"%x\"", 1, Some(b"%x")),
        (b"cat <<- \"<x\"", 1, Some(b"<x")),
        // The quoted **empty** pair (task #330): the raw span `""` is not
        // empty and does not start with an operator byte, so the byte level
        // always counted it — the level the token layer used to disagree with.
        // Its delimiter is the empty word, and the token layer now accepts it
        // (`parse/heredoc/tests.rs::layout_bare_quoted_empty_delimiter_is_a_delimiter`).
        (b"cat << \"\"", 1, Some(b"".as_slice())),
        (b"cat <<- \"\"", 1, Some(b"")),
        (b"cat << ''", 1, Some(b"")),
    ] {
        let ops = operator_delims(line, 0, line.len()).unwrap();
        assert_eq!(ops.len(), want, "line={:?}", line);
        if let Some(delim) = delim {
            assert_eq!(&ops[0].delim, delim, "line={:?}", line);
        }
    }
    // Unquoted: the raw-byte rule rejects the delimiter word, so the scan
    // reports "no delimiter" (`None`) — the token layer reports the syntax
    // error, and neither side claims a body region.
    for line in [
        b"cat << <x".as_slice(),
        b"cat << >x",
        b"cat << &x",
        b"cat << %x",
    ] {
        assert!(
            operator_delims(line, 0, line.len()).is_none(),
            "line={:?}",
            line
        );
    }
}

/// `operator_count` counts the `<<` operators in a run (the same rule the
/// token layer applies, and the count the execution layer uses to hand out
/// body regions).
#[test]
fn operator_count_counts_operators() {
    let line = b"cat <<A && cat <<B";
    assert_eq!(operator_count(line, 0, 8), 1);
    assert_eq!(operator_count(line, 10, 18), 1);
    assert_eq!(operator_count(line, 0, 18), 2);
}

/// The pipeline-pipe predecessor rule, byte side (`ops::pipe::precedes_pipe` →
/// `parse::redirect::pipeline_pipe`): a pipeline `|` **ends** the previous
/// stage's word, so the `<<` written behind it begins a word and counts 1
/// (task #345 — the stage is a redirections-only command); an escaped `\|` and
/// a quoted `"|"`/`'|'` are word content, so the `<<` behind them counts on the
/// `word_start` arm; and a `|` absorbed into a `>|` clobber is operator
/// content, so it breaks nothing — the count comes from the whitespace word
/// break, and the glued `echo a>|<<EOF` stays word data at both levels. The
/// token-level agreement of every row is pinned in
/// `parse/heredoc/tests.rs::operator_count_pipe_predecessors_agree`.
#[test]
fn escaped_and_quoted_pipe_predecessors_count_the_operator() {
    for (line, want) in [
        // A pipeline pipe: a stage boundary that begins the operator's word.
        (b"| <<EOF".as_slice(), 1usize),
        (b"> | <<EOF", 1),
        (b"a >| b | <<EOF", 1),
        (b"cat \"a>\"| <<EOF", 1),
        (b"cat x\\>| <<EOF", 1),
        // The glued clobber twin: the `|` is operator content of the word, and
        // no word break precedes the `<<` — word data at both levels.
        (b"echo a>|<<EOF", 0),
        // An escaped `|` is one word (`\|`, `x\|`): the tokenizer emits a word
        // token, not a pipe token, so the `<<` is an operator.
        (b"\\| <<EOF", 1),
        (b"x\\| <<EOF", 1),
        (b"echo x\\| <<EOF", 1),
        (b"cat x\\| <<EOF", 1),
        // A quoted `|` is word content for the same reason.
        (b"\"|\" <<EOF", 1),
        (b"'|' <<EOF", 1),
        (b"echo \"|\" <<EOF", 1),
        // The absorbed-`|` twin of the escaped form: `x\>a>|` is `>`-terminated,
        // so the `|` is a clobber byte, and the whitespace before the `<<` is
        // the word break that counts.
        (b"cat x\\>a>| <<EOF", 1),
    ] {
        assert_eq!(operator_count(line, 0, line.len()), want, "line={:?}", line);
        // Every row above has a readable delimiter, so the region layer's scan
        // and the count agree on the same number.
        assert_eq!(
            operator_delims(line, 0, line.len()).map(|ops| ops.len()),
            Some(want),
            "line={:?}",
            line
        );
    }
}

/// The count is a shape count: delimiter validity is not a gate
/// (`parse::heredoc::is_operator` has none either), while `operator_delims`
/// keeps its `None` so no body region is claimed on a line the parse layer
/// reports as invalid. This is task #261's class 3: the counts agree on the
/// sites, the region layer stays exactly as pinned (`regions.rs`,
/// `line_bodies_for_line`), and the window-subset invariant is what makes the
/// split allocation-neutral.
#[test]
fn count_counts_sites_delims_keeps_none() {
    for (line, want) in [
        (b">> << <<EOF".as_slice(), 2usize),
        (b"cat <<- <<EOF", 2),
        (b"cat << <x", 1),
        (b"cat << >x", 1),
        (b"cat << &x", 1),
        (b"cat << %x", 1),
        (b"cat <<- <<EOF ; cat <<B", 3),
        (b"cat <<- <<EOF && cat <<B", 3),
        (b">> << <<EOF && cat <<B", 3),
        (b"cat <<A ; cat <<- <<X ; cat <<B", 4),
        (b"if true; then cat <<- <<X; fi", 2),
        (b"cat <<- <<X; fi", 2),
    ] {
        assert_eq!(operator_count(line, 0, line.len()), want, "line={:?}", line);
        assert!(
            operator_delims(line, 0, line.len()).is_none(),
            "line={:?}",
            line
        );
    }
    // The valid twins keep both readers agreeing on one operator.
    for line in [b"cat << A".as_slice(), b"cat <<A", b"cat <<- A"] {
        assert_eq!(operator_count(line, 0, line.len()), 1, "line={:?}", line);
        assert_eq!(
            operator_delims(line, 0, line.len()).map(|ops| ops.len()),
            Some(1),
            "line={:?}",
            line
        );
    }
}

/// The tokenizer's separator arm emits a `;` token for a newline (and for a
/// `)`), and no token for a space or a tab; all three are **word breaks**, and
/// so is the run start (task #345): every one of these runs opens on a `<<`
/// that begins a word, so the operator counts 1 at both levels. The comment
/// row keeps the byte scan's own rule (`#` skips its line whole, so the
/// comment's newline is read as the word break of the line behind it — 1).
#[test]
fn word_break_bytes_and_the_run_start_open_the_operator() {
    for (line, want) in [
        (b"\n<<EOF".as_slice(), 1usize),
        (b"\n<<EOF <<B", 2),
        (b"\n<<EOF\nbody\nEOF", 1),
        (b"x\n<<EOF", 1),
        (b"&&\n<<EOF", 1),
        (b"<<EOF", 1),
        (b" <<EOF", 1),
        (b"\t<<EOF", 1),
        (b" \t <<EOF", 1),
        (b"# x\n<<EOF", 1),
    ] {
        assert_eq!(operator_count(line, 0, line.len()), want, "line={:?}", line);
    }
}

/// The `(( … ))` / `$( … )` region is opaque to the byte scan
/// (`ScanState::advance` consumes `((` and `$(` as one unit and the `bare`
/// gate hides everything inside it). Counting an arithmetic `<<` would corrupt
/// the line's region stream, so this is the **pinned divergence** from the
/// token layer — `parse/heredoc/tests.rs::arith_region_count_divergence_is_pinned`
/// pins the pairs, and `((1<<2))` agrees at 0 through `io::word_start`.
#[test]
fn arithmetic_and_substitution_regions_are_opaque() {
    for (line, want) in [
        (b"(( <<EOF".as_slice(), 0usize),
        (b"(( 1 <<EOF", 0),
        (b"(( x <<EOF", 0),
        (b"((1<<2))", 0),
        (b"(( 1 << 2 ))", 0),
        (b"echo $((1<<2))", 0),
        (b"x=$(cat <<EOF\nbody\nEOF\n)", 0),
        // Only the region *after* the closed arithmetic command is counted.
        (b"(( 1 << 2 )) && cat <<B", 1),
        (b"(( 1 << 2 )); cat <<B", 1),
    ] {
        assert_eq!(operator_count(line, 0, line.len()), want, "line={:?}", line);
    }
}

/// The `precedes_pipe` walk reads bytes between the operator's word start and
/// the `|` (`ops::pipe::precedes_pipe`): whitespace is skipped, any other byte
/// ends the walk. Pinned directly because the count arms hide it.
#[test]
fn precedes_pipe_walks_only_whitespace() {
    assert!(pipe::precedes_pipe(b"| <<EOF", 2));
    assert!(pipe::precedes_pipe(b"   | <<EOF", 5));
    assert!(pipe::precedes_pipe(b"a| <<EOF", 2));
    assert!(!pipe::precedes_pipe(b"x <<EOF", 4));
    assert!(!pipe::precedes_pipe(b"a >| <<EOF", 6));
    assert!(!pipe::precedes_pipe(b"\"| <<EOF", 4));
}

/// The walk stops at `to`: a window that ends inside the line never reads the
/// bytes behind it, so an operator that *starts* at `to` is not a site. The
/// same shape kills the loop's `i <= to` mutant, which would walk one byte
/// past the window and push a phantom site (`operator_count` is called on
/// sub-windows by the segment/cond layers, so this is their real input).
#[test]
fn window_end_is_exclusive() {
    let line = b"cat <<A <<B";
    assert_eq!(
        operator_count(line, 0, 8),
        1,
        "the window ends at the space"
    );
    assert_eq!(
        operator_delims(line, 0, 8).map(|ops| ops.len()),
        Some(1),
        "line={:?}",
        line
    );
    assert_eq!(operator_count(line, 0, line.len()), 2, "the full line");
    assert_eq!(
        operator_count(line, 2, line.len()),
        2,
        "mid-token window start"
    );
    // A window that opens **on** the operator opens a word, so the operator
    // counts (task #345): the segment/cond layers hand the parse layer exactly
    // such windows, and the token layer counts the same 1 there. One byte
    // inside the operator (`from = 9` of `cat <<A <<B`) is no word start at
    // all, so the same window counts 0 — the pair pins both arms.
    assert_eq!(
        operator_count(line, 9, line.len()),
        0,
        "a window opening inside the operator's bytes opens no word",
    );
    assert_eq!(
        operator_count(line, 8, line.len()),
        1,
        "the same window from the operator's first byte",
    );
    assert_eq!(
        operator_count(line, 7, line.len()),
        1,
        "the same from the space before it",
    );
}

/// The `<<-` marker consumes exactly its one byte: the walk resumes after it,
/// so a second operator on the same line is still found and keeps its own
/// delimiter (`i + 2 + dash`, and the `dash`-offset mutants' kill pin).
#[test]
fn dash_marker_leaves_the_next_operator_visible() {
    let line = b"cat <<-A <<B";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 2, "line={:?}", line);
    assert_eq!(ops[0].delim, b"A");
    assert!(ops[0].strip);
    assert_eq!(ops[1].delim, b"B");
    assert!(!ops[1].strip);
    assert_eq!(operator_count(line, 0, line.len()), 2);
    // The bare `<<-` at the window end keeps its own site and stops there.
    assert_eq!(operator_count(b"cat <<-", 0, 7), 1);
}
