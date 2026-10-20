#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
use super::tokenize;
use crate::parse::Token;
use alloc::{vec, vec::Vec};
use sys::ShortCStr;

/// The word text plus its quote mask of every token of `line`.
fn words(line: &[u8]) -> Vec<(ShortCStr, Vec<bool>)> {
    tokenize(line)
        .unwrap()
        .into_iter()
        .map(|(t, _, _, _, mask, _spans)| (t, mask))
        .collect()
}

/// The word text of every token of `line`.
fn text(line: &[u8]) -> Vec<ShortCStr> {
    words(line).into_iter().map(|(t, _)| t).collect()
}

// POSIX #4.1: the escape pair stays in the token text, so the byte offsets and
// the token-text syntax prefixes (`\if`, `\)`, `\<<`) keep working.
#[test]
fn escape_pair_stays_in_the_word_text() {
    let w = words(b"echo a\\*");
    assert_eq!(w[1].0, ShortCStr::from(c"a\\*"));
    assert_eq!(
        w[1].1,
        vec![false, false, false],
        "the pair bytes are unquoted"
    );
}

// A trailing `\` at end of line is a lone word byte (bash prints a literal `\`).
#[test]
fn trailing_backslash_at_eof_is_kept() {
    assert_eq!(
        text(b"echo a\\"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\")]
    );
}

// An escaped `;` is a word byte: one token, no separator token.
#[test]
fn escaped_semicolon_is_one_word() {
    assert_eq!(
        text(b"echo a\\;b"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\;b")]
    );
}

// An escaped `|` never splits a pipeline.
#[test]
fn escaped_pipe_is_one_word() {
    assert_eq!(
        text(b"echo a\\|b"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\|b")]
    );
}

// An escaped `)` does not emit the separator token the bare `)` arm pushes.
#[test]
fn escaped_paren_emits_no_separator() {
    assert_eq!(
        text(b"echo a\\)b"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a\\)b")]
    );
}

// `\<newline>` is a word byte pair (the line continuation), not a line end.
#[test]
fn escaped_newline_stays_in_the_word() {
    assert_eq!(
        text(b"echo x\\\ny"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"x\\\ny")]
    );
}

// An escaped `#` is a word byte, so it never starts a comment.
#[test]
fn escaped_hash_is_a_word_byte() {
    assert_eq!(
        text(b"echo \\#hi"),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"\\#hi")]
    );
}

// A leading escape pair makes the word un-prefixable: `\if` is one word, not a
// keyword, and `a\>b` is one word, not a redirect.
#[test]
fn escape_pair_breaks_token_text_prefixes() {
    assert_eq!(
        text(b"\\if a"),
        [ShortCStr::from(c"\\if"), ShortCStr::from(c"a")]
    );
    assert_eq!(
        text(b"echo a\\>b"),
        [ShortCStr::from(c"echo"), ShortCStr::from(c"a\\>b")]
    );
}

// Inside double quotes the POSIX #4.2 rules are unchanged: `\$`/`\\` keep both
// bytes for substitution, `\"` yields a literal `"`, any other pair keeps the
// backslash, and every byte is mask-protected.
#[test]
fn quoted_escape_rules_are_unchanged() {
    let w = words(b"echo \"a\\\\b\" \"a\\$X\" \"a\\\"b\" \"a\\nc\"");
    assert_eq!(w[1].0, ShortCStr::from(c"a\\\\b"));
    assert_eq!(w[2].0, ShortCStr::from(c"a\\$X"));
    assert_eq!(w[3].0, ShortCStr::from(c"a\"b"));
    assert_eq!(w[4].0, ShortCStr::from(c"a\\nc"));
    assert!(
        w[1].1.iter().all(|q| *q),
        "quoted bytes stay mask-protected"
    );
}

// POSIX #2.2 `>|`: a `|` right after a `>` operator byte is absorbed into the
// operator word, so `>|file`, `2>|file` and the bare `>|` are one token each.
#[test]
fn clobber_pipe_absorbed_into_operator() {
    assert_eq!(
        text(b"echo a >|b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c">|b")
        ]
    );
    assert_eq!(
        text(b"echo a >| b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c">|"),
            ShortCStr::from(c"b"),
        ]
    );
    assert_eq!(
        text(b"echo hi 2>|f"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"hi"),
            ShortCStr::from(c"2>|f"),
        ]
    );
}

// A `|` separated by whitespace from a bare `>` is a pipeline pipe: only a `|`
// whose word ends *at* the `>` byte is absorbed (`> | f` stays three tokens).
#[test]
fn spaced_pipe_after_bare_gt_stays_a_pipe() {
    assert_eq!(
        text(b"echo a > | b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c">"),
            ShortCStr::from(c"|"),
            ShortCStr::from(c"b"),
        ]
    );
}

// `&>|f` is absorbed as one token, so `&>|` reaches `parse_bg_redirect` as a
// `|` tail and is rejected there; the `&>|&pid` pidvar form stays one token.
#[test]
fn amp_clobber_is_one_token() {
    assert_eq!(
        text(b"cmd &>|f"),
        [ShortCStr::from(c"cmd"), ShortCStr::from(c"&>|f")]
    );
    assert_eq!(
        text(b"cmd &>|&pid"),
        [ShortCStr::from(c"cmd"), ShortCStr::from(c"&>|&pid")]
    );
}

// A word that *carries* the operator keeps the absorbed `|` as a word byte:
// accepted deviation (fdshell never breaks a word at operator bytes, as for
// `a>b`), pinned so the deviation is measured, not assumed.
#[test]
fn prefix_word_absorbs_pipe_is_one_word() {
    assert_eq!(
        text(b"echo x a>|b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"x"),
            ShortCStr::from(c"a>|b")
        ]
    );
    // The shared rule skips escape pairs, so `x\>a>` is `>`-terminated (the
    // pair's `>` is not the operator byte) and keeps the absorbed `|`; bash
    // 5.3.9 breaks the word at `>` (rc 0, empty stdout), fdshell prints the
    // word `x>a>|b` at rc 0.
    assert_eq!(
        text(b"echo x\\>a>|b"),
        [ShortCStr::from(c"echo"), ShortCStr::from(c"x\\>a>|b")]
    );
    // `x\>` is not: its last byte is the escape pair's second byte, so the `|`
    // stays a pipeline pipe, exactly as on master (rc 1, `"b" not found`; bash
    // rc 127).
    assert_eq!(
        text(b"echo x\\>|b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"x\\>"),
            ShortCStr::from(c"|"),
            ShortCStr::from(c"b"),
        ]
    );
}

// A quoted word is not `>`-terminated: its last byte is inside double quotes,
// so the `|` right after it is the pipeline pipe. `echo "a>"|b` is a pipeline
// on master and on bash 5.3.9 (fdshell rc 1 `"b" not found`, bash rc 127).
#[test]
fn quoted_word_is_not_terminated_by_the_operator_byte() {
    assert_eq!(
        text(b"echo \"a>\"|b"),
        [
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"a>"),
            ShortCStr::from(c"|"),
            ShortCStr::from(c"b"),
        ]
    );
}

// The absorbed `|` byte is unquoted (mask `false`), so the target's mask stays
// `true` over exactly the quoted path bytes.
#[test]
fn clobber_absorbed_pipe_is_unquoted_in_the_mask() {
    let w = words(b"echo hi >|\"my file\"");
    assert_eq!(w[2].0, ShortCStr::from(c">|my file"));
    assert!(!w[2].1[0], "the `>` byte is unquoted");
    assert!(!w[2].1[1], "the absorbed `|` byte is unquoted");
    assert_eq!(
        w[2].1[2..],
        vec![true; 7],
        "the quoted path is mask-protected"
    );
}

// --- Single quotes (task #141). Every expectation below is bash's word text.

/// The full token view of a line: word text, span, `fq`, mask, spans.
fn full(line: &[u8]) -> Vec<Token> {
    tokenize(line).unwrap()
}

// A `'…'` span is stripped and masked exactly like a double-quoted span: the
// space is word content and the bytes are protected from splitting (`x y`).
#[test]
fn single_quoted_span_is_one_masked_word() {
    let w = full(b"echo 'x y'");
    assert_eq!(
        w[1].0,
        ShortCStr::from(c"x y"),
        "both quote bytes are stripped"
    );
    assert_eq!(w[1].1, 5, "the span is the quoted word");
    assert!(w[1].3, "the word starts quoted (fq)");
    assert_eq!(w[1].4, vec![true; 3], "every span byte is masked");
}

// `'a'b'c'` is one word (`abc`): adjacent spans concatenate, as bash does.
#[test]
fn adjacent_single_quotes_make_one_word() {
    let w = full(b"echo 'a'b'c'");
    assert_eq!(w[1].0, ShortCStr::from(c"abc"));
    assert_eq!(w.len(), 2, "one word, no split");
    assert!(
        !w[1].3,
        "the bare `b` byte clears `fq`, exactly as in `\"a\"b`"
    );
}

// `''` is one empty word: `fq` set, empty text and mask, span 2 bytes
// (`word_quoted` is what makes the empty word count).
#[test]
fn empty_single_quotes_are_one_empty_word() {
    let w = full(b"echo ''");
    assert_eq!(w[1].0, ShortCStr::from(c""));
    assert!(w[1].4.is_empty(), "no bytes, so no mask bits");
    assert!(w[1].3, "fq");
    assert_eq!(w[1].2 - w[1].1, 2, "the span is the two quote bytes");
    assert!(
        super::super::word_quoted::word_quoted(&w[1].0, w[1].1, w[1].2),
        "the span is longer than the word: a word that counts"
    );
}

// A single quote inside double quotes is a literal byte (`a'b'c`).
#[test]
fn single_quotes_inside_double_quotes_are_literal() {
    assert_eq!(
        text(b"echo \"a'b'c\""),
        vec![ShortCStr::from(c"echo"), ShortCStr::from(c"a'b'c")]
    );
}

// The encoding at its origin: a single-quoted backslash is stored as the
// double-quote escape pair `\\`, which every reader of word text folds back to
// one literal `\` (`echo 'a\bc'` prints `a\bc` in bash).
#[test]
fn single_quoted_backslash_is_encoded_as_an_escape_pair() {
    let w = full(b"echo 'a\\bc'");
    assert_eq!(w[1].0, ShortCStr::from(c"a\\\\bc"));
    assert_eq!(w[1].4, vec![true; 5], "every encoded byte is masked");
}

// The `$` of a single-quoted span goes in as `\$` so the substitution pass
// yields a literal `$` and never expands a variable (`echo 'a$b'` → `a$b`).
#[test]
fn single_quoted_dollar_is_encoded_as_an_escaped_dollar() {
    let w = full(b"echo 'a$b'");
    assert_eq!(w[1].0, ShortCStr::from(c"a\\$b"));
    assert_eq!(w[1].4, vec![true; 4]);
    assert_eq!(
        super::super::super::bytes::fold::fold_mask(w[1].0.as_bytes().unwrap(), &w[1].4).0,
        b"a$b".to_vec(),
        "the quoted fold yields bash's literal text"
    );
}

// `$'x'` is the POSIX-sh reading (`$x`, mask-protected), not bash's ANSI-C
// quoting — the accepted divergence documented in the README (task #156).
#[test]
fn dollar_single_quote_is_the_posix_sh_reading() {
    let w = full(b"echo $'x'");
    assert_eq!(w[1].0, ShortCStr::from(c"\\$x"));
    assert_eq!(w[1].4, vec![false, false, true]);
    assert!(!w[1].3, "the word starts unquoted");
}

// No separator, operator or redirection byte inside a span is special.
#[test]
fn span_bytes_are_never_separators() {
    for (line, want) in [
        (b"echo 'a|b'".as_slice(), "a|b"),
        (b"echo 'a;b'", "a;b"),
        (b"echo 'a>b'", "a>b"),
        (b"echo 'a&&b'", "a&&b"),
        (b"echo 'a b'", "a b"),
        (b"echo 'a#b'", "a#b"),
    ] {
        assert_eq!(
            text(line).get(1),
            Some(&ShortCStr::from_vec(want.as_bytes().to_vec()).unwrap()),
            "{line:?}: the span is one word"
        );
    }
}

// A `"` written inside a `'…'` span does not toggle the double-quote state,
// and the span's text stays literal (`echo "a'b\"" ` is unaffected).
#[test]
fn double_quote_inside_span_does_not_toggle() {
    let w = full(b"echo 'a\"b'");
    assert_eq!(
        w[1].0,
        ShortCStr::from(c"a\"b"),
        "the span is one literal word: the inner quote is a data byte, not a toggle"
    );
    assert_eq!(w[1].4, vec![true; 3]);
}

// An unbalanced `'` is an `UnbalancedQuote` reported at the opening byte: the
// `ParsePosition` carried on the report is the index of the `'` that opened the
// span (the same byte the caret line points at), not a neighbouring byte.
#[test]
fn unbalanced_single_quote_reports_the_opening_byte() {
    for line in [
        b"echo 'abc".as_slice(),
        b"echo 'a' 'b".as_slice(),
        b"echo x 'y".as_slice(),
    ] {
        let err = tokenize(line).expect_err("unbalanced quote");
        assert!(alloc::format!("{err:?}").contains("unmatched quote"));
        let pos = err
            .downcast_ref::<crate::error::parse::ParsePosition>()
            .expect("the report carries a position")
            .pos;
        let open = line.iter().rposition(|&b| b == b'\'').unwrap();
        assert_eq!(pos, open, "{line:?}: the report must point at the open `'`");
        assert_eq!(line[pos], b'\'');
    }
}

// A trailing `''` empty span still ends the word (`echo x ''` is two words).
#[test]
fn empty_span_after_a_word_is_a_word_of_its_own() {
    assert_eq!(
        text(b"echo x ''"),
        vec![
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"x"),
            ShortCStr::from(c""),
        ]
    );
}

// Task #321: a quote pair that contributes **no bytes** leaves no mask bit, so
// the tokenizer records its opening index in the token's `spans` (field 5) —
// the only place the zero-byte span survives for the tilde gate. The index is
// the pair's position in the word text, i.e. the text length when the pair
// opened: `~""/x` records `1` (the `/` follows at index 1), and a pair at the
// word's end records the word's length (`a~""` → `2`).
fn spans(line: &[u8]) -> Vec<Vec<usize>> {
    tokenize(line)
        .unwrap()
        .into_iter()
        .map(|(_t, _s, _e, _fq, _mask, spans)| spans)
        .collect()
}

#[test]
fn empty_quote_pairs_record_their_closing_index() {
    for (line, want) in [
        // `~""/x`: the pair sits between the `~` and the `/`.
        (b"~\"\"/x".as_slice(), vec![vec![1]]),
        // `~''`: the same shape with single quotes.
        (b"~''".as_slice(), vec![vec![1]]),
        // `""~/x`: the pair is at word-text index 0 — before the `~`.
        (b"\"\"~/x".as_slice(), vec![vec![0]]),
        // `~/""`: the pair is *after* the `/`, at index 2.
        (b"~/\"\"".as_slice(), vec![vec![2]]),
        // `~""''/x`: adjacent pairs each record their own index.
        (b"~\"\"''/x".as_slice(), vec![vec![1, 1]]),
        // `~"a"/x`: the pair contributes `a`, so nothing is recorded.
        (b"~\"a\"/x".as_slice(), vec![vec![]]),
        // A bare word has no spans at all.
        (b"echo".as_slice(), vec![vec![]]),
    ] {
        assert_eq!(spans(line), want, "{line:?}");
    }
}

#[test]
fn spans_are_per_word_and_reset_between_words() {
    // `a~"" b~""`: two words, each carrying its own single index — a span
    // recorded for the first word must not leak into the second (`word_reset`).
    assert_eq!(spans(b"a~\"\" b~\"\""), vec![vec![2], vec![2]]);
    // The pair ends the first word: `x "" ~""` is three words.
    assert_eq!(spans(b"x \"\" ~\"\""), vec![vec![], vec![0], vec![1]]);
    // The assignment word keeps the pair in its own spans, indexed in the
    // whole word text (`x=~""` is the text `x=~`, so the index is 3; the
    // assignment slices it to the value's own index 1 — `envassign::value_spans`).
    assert_eq!(spans(b"x=~\"\""), vec![vec![3]]);
}

// --- Task #266: a delimiter (terminator) line is heredoc structure, never
// command bytes. `tokenize_statement` blanks its **syntax** bytes (the set
// `is_structural` defines) and keeps its **words** and its newline: a terminator
// shaped like a command line yields no operator, no pipe, no substitution and
// no `unmatched quote`, while the words the block machinery measures its text
// slices by stay, and the `;` token of the newline keeps the following statement
// separate.
// ---

/// The word texts of `tokenize_statement` (bodies blanked, terminator syntax
/// blanked, offsets preserved).
fn statement_text(line: &[u8]) -> Vec<ShortCStr> {
    super::statement::tokenize_statement(line)
        .unwrap()
        .into_iter()
        .map(|(t, _, _, _, _, _)| t)
        .collect()
}

/// The terminator's own `<<`, `|`, `$(`, backtick and quote bytes are gone from
/// the token stream; its words and the separator newline stay. Every row is a
/// shape that `master` tokenized as command bytes (rc 1: an operator count that
/// shifts the body allocation, a pipeline split, or `unmatched quote`).
#[test]
fn a_terminator_line_keeps_its_words_and_loses_its_syntax() {
    // A plain delimiter: the terminator word and the statement behind it.
    assert_eq!(
        statement_text(b"cat <<T\nbody\nT\na <<b"),
        vec![
            ShortCStr::from(c"cat"),
            ShortCStr::from(c"<<T"),
            ShortCStr::from(c";"),
            ShortCStr::from(c"T"),
            ShortCStr::from(c";"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c"<<b"),
        ],
        "the statement **behind** a terminator keeps its own operators"
    );
    for (line, want) in [
        (
            b"cat <<'a <<b'\nbody\na <<b".as_slice(),
            vec![
                ShortCStr::from(c"cat"),
                ShortCStr::from(c"<<a <<b"),
                ShortCStr::from(c";"),
                ShortCStr::from(c"a"),
                ShortCStr::from(c"b"),
            ],
        ),
        (
            b"cat <<a\\\"b\nbody\na\"b".as_slice(),
            vec![
                ShortCStr::from(c"cat"),
                ShortCStr::from(c"<<a\\\"b"),
                ShortCStr::from(c";"),
                ShortCStr::from(c"a"),
                ShortCStr::from(c"b"),
            ],
        ),
        (
            b"cat <<'a|b'\nbody\na|b".as_slice(),
            vec![
                ShortCStr::from(c"cat"),
                ShortCStr::from(c"<<a|b"),
                ShortCStr::from(c";"),
                ShortCStr::from(c"a"),
                ShortCStr::from(c"b"),
            ],
        ),
    ] {
        assert_eq!(statement_text(line), want, "{line:?}");
    }
    // A terminator plus a trailing statement: the `;` of the terminator's
    // newline keeps `echo after` a separate statement.
    assert_eq!(
        statement_text(b"cat <<'a <<b'\nbody\na <<b\necho after"),
        vec![
            ShortCStr::from(c"cat"),
            ShortCStr::from(c"<<a <<b"),
            ShortCStr::from(c";"),
            ShortCStr::from(c"a"),
            ShortCStr::from(c"b"),
            ShortCStr::from(c";"),
            ShortCStr::from(c"echo"),
            ShortCStr::from(c"after"),
        ]
    );
}

/// A terminator line carrying an unbalanced quote is opaque data, exactly like a
/// body line: tokenizing must not fail (the quote check never sees its bytes).
#[test]
fn a_terminator_line_with_an_unbalanced_quote_tokenizes() {
    for line in [
        b"cat <<a\\\"b\nbody\na\"b".as_slice(),
        b"cat <<'a\"b'\nbody\na\"b".as_slice(),
        b"cat <<'a$('b'\nbody\na$('b".as_slice(),
        b"cat <<'a`b'\nbody\na`b".as_slice(),
    ] {
        assert!(
            super::statement::tokenize_statement(line).is_ok(),
            "{line:?}: an unbalanced terminator quote is terminator data"
        );
    }
}

/// The byte set the tokenizer reads as syntax (POSIX metacharacters, quote
/// operators, expansion/escape triggers, fdshell's `%` capture marker, and the
/// blanks that keep a terminator's words apart). `{`/`}` are **not** in the set
/// (brace expansion is a pre-parse pass on the original text, with its own
/// terminator protection), and neither are the word bytes, the digits or the
/// newline: the words and the newline stay.
#[test]
fn is_structural_is_the_tokenizer_syntax_byte_set() {
    for b in b";&()<>\x7c$ \x60\"'\\#~%\t".iter() {
        assert!(super::statement::is_structural(*b), "{b:?} is syntax");
    }
    for b in b"{}abcXY079_*[].,:!?/-=+@^\n".iter() {
        assert!(!super::statement::is_structural(*b), "{b:?} is a word byte");
    }
}
