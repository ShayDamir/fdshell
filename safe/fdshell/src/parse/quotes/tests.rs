#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use crate::error::parse::ParseError;
use crate::parse::token;
use crate::parse::token::State;
use alloc::vec;
use alloc::vec::Vec;

/// Tokens of a line, panicking on a parse error (the tests below are the
/// shapes that must parse).
fn toks(line: &[u8]) -> Vec<crate::parse::Token> {
    token::tokenize(line).unwrap()
}

fn mask_of(line: &[u8]) -> Vec<bool> {
    toks(line)[0].4.clone()
}

fn text_of(line: &[u8]) -> Vec<u8> {
    toks(line)[0].0.as_bytes().unwrap().to_vec()
}

fn span_of(line: &[u8]) -> (usize, usize) {
    let t = &toks(line)[0];
    (t.1, t.2)
}

/// The quote state and mask length the region readers must leave behind: the
/// outer quote is still open in every case, and every pushed byte has a bit.
#[test]
fn quoted_region_keeps_the_quote_open_and_the_mask_parallel() {
    for line in [
        b"\"$(echo \"x y\")\"".as_slice(),
        b"\"${u:-\"z w\"}\"".as_slice(),
        b"\"`printf \"q r\"`\"".as_slice(),
        b"\"$(echo $(echo deep))\"".as_slice(),
    ] {
        let tokens = token::tokenize(line).unwrap();
        assert_eq!(tokens.len(), 1, "{line:?}");
        let t = tokens.first().unwrap();
        assert_eq!(
            t.4.len(),
            t.0.as_bytes().unwrap().len(),
            "{line:?}: one mask bit per byte"
        );
        assert!(t.4.iter().all(|&q| q), "{line:?}: the region is quoted");
    }
}

// Row 1: a `$( … )` with inner quotes inside double quotes is one word, and
// only the leading `a` is unquoted.
#[test]
fn quoted_nested_dollar_paren_is_one_word_with_a_quoted_region() {
    let tokens = toks(b"a\"$(echo \"x y\")\"");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.as_bytes().unwrap(), b"a$(echo \"x y\")");
    assert_eq!((tokens[0].1, tokens[0].2), (0, 16));
    assert!(!tokens[0].3, "the word starts unquoted");
    let mut mask = vec![false];
    mask.extend(vec![true; 13]);
    assert_eq!(tokens[0].4, mask);
}

// Row 3: a fully quoted `$( … )` keeps `fq` and the all-true mask, so the
// expansion is not split on IFS.
#[test]
fn quoted_dollar_paren_with_inner_quotes_is_fully_quoted() {
    let tokens = toks(b"\"$(printf \"a b\")\"");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.as_bytes().unwrap(), b"$(printf \"a b\")");
    assert_eq!((tokens[0].1, tokens[0].2), (0, 17));
    assert!(tokens[0].3);
    assert_eq!(tokens[0].4, vec![true; 15]);
}

// Row 10/11: the `${ … }` body's quotes are markers, so they are dropped from
// the text (the substitution pass takes the body verbatim) and the span keeps
// them (so `word_quoted` sees the quotes).
#[test]
fn quoted_brace_region_drops_its_inner_quotes() {
    let tokens = toks(b"\"${u:-\"z w\"}\"");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.as_bytes().unwrap(), b"${u:-z w}");
    assert_eq!((tokens[0].1, tokens[0].2), (0, 13));
    assert!(tokens[0].3);
    assert_eq!(tokens[0].4, vec![true; 9]);
    assert!(
        crate::parse::word_quoted::word_quoted(&tokens[0].0, tokens[0].1, tokens[0].2),
        "the span holds the 4 dropped quote bytes"
    );
}

// The backtick region behaves the same; fdshell never expands backticks
// (task #216), so only the scanning shape is pinned.
#[test]
fn quoted_backtick_region_is_one_word() {
    assert_eq!(text_of(b"\"`printf \"q r\"`\""), b"`printf q r`".to_vec());
    assert_eq!(span_of(b"\"`printf \"q r\"`\""), (0, 16));
    assert_eq!(mask_of(b"\"`printf \"q r\"`\""), vec![true; 12]);
}

// Rows 4–8: the separators inside a quoted region are word bytes, so the
// statement, cond-list and heredoc scanners never see them.
#[test]
fn separators_inside_a_quoted_region_stay_in_the_word() {
    for line in [
        b"\"$(echo a; echo b)\"".as_slice(),
        b"\"$(printf \"x && y\")\"".as_slice(),
        b"\"$(printf \"a | b\")\"".as_slice(),
        b"\"$(printf \"a > f\")\"".as_slice(),
        b"\"$(printf \"<<EOF\")\"".as_slice(),
        b"\"${u:-\"a; b\"}\"".as_slice(),
    ] {
        let tokens = toks(line);
        assert_eq!(tokens.len(), 1, "{line:?}");
        assert!(tokens[0].3, "{line:?} is fully quoted");
    }
}

// Deep nesting: the inner `$( … )` closes before the outer one, and the whole
// thing is one quoted word.
#[test]
fn nested_quoted_regions_are_one_word() {
    assert_eq!(
        toks(b"\"$(echo \"$(echo deep)\")\"").len(),
        1,
        "the inner region closes first"
    );
    assert_eq!(span_of(b"\"$(echo \"$(echo deep)\")\""), (0, 24));
    assert_eq!(mask_of(b"\"$(echo \"$(echo deep)\")\""), vec![true; 22]);
    assert_eq!(toks(b"\"a$(printf \" b\")c\"").len(), 1, "row 14");
}

// The existing intent (`parse/tests.rs`), restated for the new code path.
#[test]
fn quoted_region_without_inner_quotes_keeps_the_all_true_mask() {
    let tokens = toks(b"\"$(true)\"");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.as_bytes().unwrap(), b"$(true)");
    assert!(tokens[0].3);
    assert_eq!(tokens[0].4, vec![true; 7]);
}

// An unterminated `$( … )` keeps the existing `unexpected eof` report, with
// the caret at the `$` (the unquoted path's rule).
#[test]
fn unterminated_quoted_dollar_paren_reports_unexpected_eof() {
    for line in [
        b"\"$(echo a".as_slice(),
        b"\"$(echo \"a\"".as_slice(),
        b"\"$(x\"".as_slice(),
    ] {
        let report = token::tokenize(line).unwrap_err();
        assert!(
            matches!(report.current_context(), ParseError::UnexpectedEof),
            "{line:?}"
        );
    }
}

// An unterminated region leaves the enclosing quote open, so the report is the
// unbalanced quote at the *outer* quote (LESSONS: always show the caret, even
// at position 0). A `$( … )` that closes normally inside an unterminated quote
// takes this path too.
#[test]
fn unterminated_quoted_region_reports_the_outer_quote() {
    for line in [
        b"\"${u".as_slice(),
        b"\"${u:-x".as_slice(),
        b"\"`a".as_slice(),
    ] {
        let report = token::tokenize(line).unwrap_err();
        assert!(
            matches!(report.current_context(), ParseError::UnbalancedQuote),
            "{line:?}"
        );
        let pos = report
            .downcast_ref::<crate::error::parse::ParsePosition>()
            .unwrap()
            .pos;
        assert_eq!(pos, 0, "{line:?}: the caret is at the outer quote");
    }
}

// An unpaired trailing quote in a `${ … }` / backtick body belongs to the
// enclosing region: the quote closes the word and the unterminated region
// stays literal text (the pinned `unclosed_brace_stays_literal` shape).
#[test]
fn unpaired_trailing_quote_closes_the_word_and_keeps_the_region_literal() {
    assert_eq!(text_of(b"\"${v#a\""), b"${v#a".to_vec());
    assert_eq!(span_of(b"\"${v#a\""), (0, 7));
    assert_eq!(mask_of(b"\"${v#a\""), vec![true; 5]);
    assert_eq!(text_of(b"\"x`y\""), b"x`y".to_vec());
    assert_eq!(text_of(b"\"a${b\""), b"a${b".to_vec());
}

// The guard of the quoted-empty-word cases: the quote closed *before* the
// region, so the region's bytes are unquoted and the word splits (bash: 2).
#[test]
fn region_after_a_closed_quote_keeps_the_unquoted_mask() {
    let tokens = toks(b"\"\"$(echo \"a b\")");
    assert_eq!(
        tokens.len(),
        1,
        "the empty quoted word and the region are one word"
    );
    assert_eq!(tokens[0].0.as_bytes().unwrap(), b"$(echo \"a b\")");
    assert_eq!((tokens[0].1, tokens[0].2), (0, 15));
    assert_eq!(
        tokens[0].4,
        vec![false; 13],
        "no byte of the region is quoted"
    );
    assert!(!tokens[0].3, "the quote closed before the region");
}

// Row 15 + brace expansion: the token spans must stay exact for the pre-parse
// rewriters (`alias_expand`, `brace_expand`), which slice the line by span.
#[test]
fn quoted_regions_keep_exact_spans_for_the_rewriters() {
    assert_eq!(span_of(b"\"$(echo {a,b})\""), (0, 15));
    let tokens = toks(b"echo \"$(echo \"a b\")\" x");
    assert_eq!(tokens.len(), 3);
    assert_eq!((tokens[1].1, tokens[1].2), (5, 20));
    assert_eq!((tokens[2].1, tokens[2].2), (21, 22));
}

// `$( … )` bodies are kept verbatim — quotes and escapes included — because
// `cmd_subst` re-parses them (the existing `cmd_subst.rs` pins, tokenizer side).
#[test]
fn dollar_paren_body_text_is_verbatim() {
    assert_eq!(
        text_of(b"\"$(echo \\\"a\\\")\""),
        b"$(echo \\\"a\\\")".to_vec()
    );
    assert_eq!(span_of(b"\"$(echo \\\"a\\\")\""), (0, 15));
    assert_eq!(text_of(b"\"$(echo \"a)\")\""), b"$(echo \"a)\")".to_vec());
    assert_eq!(span_of(b"\"$(echo \"a)\")\""), (0, 14));
}

// The quote state after a region: the outer quote stays open across it, and a
// region inside a word that already has bytes keeps the mask parallel.
#[test]
fn region_inside_a_longer_word_keeps_the_mask_parallel() {
    for line in [
        b"x\"$(echo \"a b\")\"".as_slice(),
        b"\"$(echo \"a b\")\"y".as_slice(),
        b"a\"${u:-\"z w\"}\"b".as_slice(),
        b"\"`a`\"x\"${u:-\"p q\"}\"".as_slice(),
    ] {
        let tokens = toks(line);
        assert_eq!(tokens.len(), 1, "{line:?}");
        let st = tokens[0].0.as_bytes().unwrap();
        assert_eq!(tokens[0].4.len(), st.len(), "{line:?}");
    }
}

// `State::quoted_char` returns "the quote is still open" for a region, which
// is the whole point of the change; the plain quote byte still closes.
#[test]
fn quoted_char_reports_the_quote_still_open_after_a_region() {
    let line = b"\"$(true)\"";
    let mut st = State::new();
    let mut bytes = line.iter().copied().peekable();
    // Consume the opening quote exactly as `tokenize` does.
    bytes.next();
    st.pos += 1;
    st.in_quotes = true;
    st.quote_start = Some(0);
    st.word_quoted = true;
    let mut closed = false;
    while let Some(b) = bytes.next() {
        st.pos += 1;
        closed = !st.quoted_char(b, &mut bytes, line).unwrap();
        if closed {
            break;
        }
    }
    assert!(closed, "the trailing quote closes the region's word");
    assert_eq!(st.cur.as_bytes().unwrap(), b"$(true)");
    assert_eq!(st.mask, vec![true; 7]);
    assert_eq!(st.pos, 9, "pos is the exclusive end of the consumed bytes");
}

// Task #130: the separators a byte-level scanner acts on (`;`, `|`, `&&`, `<`)
// are word data when they sit inside a quoted region. The region's word is one
// token that contains them, and only separators written outside the quotes
// become their own tokens.
#[test]
fn separators_inside_a_quoted_region_stay_word_data() {
    fn words(line: &[u8]) -> Vec<Vec<u8>> {
        toks(line)
            .iter()
            .map(|t| t.0.as_bytes().unwrap().to_vec())
            .collect()
    }
    fn want(words: &[&[u8]]) -> Vec<Vec<u8>> {
        words.iter().map(|w| w.to_vec()).collect()
    }
    let cases: &[(&[u8], &[&[u8]])] = &[
        (
            b"echo \"$(printf \"a; b\")\"",
            &[b"echo", b"$(printf \"a; b\")"],
        ),
        (
            b"echo \"$(printf \"a | b\")\" | cat",
            &[b"echo", b"$(printf \"a | b\")", b"|", b"cat"],
        ),
        (
            b"echo \"${u:-\"a && b\"}\" && echo ok",
            &[b"echo", b"${u:-a && b}", b"&&", b"echo", b"ok"],
        ),
        (
            b"echo \"`printf \"a < b\"`\"",
            &[b"echo", b"`printf a < b`"],
        ),
        (
            b"echo \"$(printf \"a; b\")\"; echo after",
            &[b"echo", b"$(printf \"a; b\")", b";", b"echo", b"after"],
        ),
    ];
    for (line, expected) in cases {
        assert_eq!(words(line), want(expected), "{line:?}");
    }
}
