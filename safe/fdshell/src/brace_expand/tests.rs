#![allow(clippy::unwrap_used)]
use super::expand;
use super::gobbler::{GroupType, gobble};
use super::protect::protected;
use super::seq::mkseq::element_count;
use super::seq::{MAX_WORDS, SeqKind, SeqSpec, expand_seqterm, mkseq, valid_seqterm};
use super::word::concat::{cross, cross_total};
use super::word::expand_word;
use crate::error::cmd::CmdError;
use crate::error::parse::ParseError;
use crate::parse::token::tokenize_statement;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn from(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn text(s: &str) -> ScriptText {
    ScriptText::new(from(s), Position::new(1, 1), Origin::Stdin)
}

fn expand_line(line: &str) -> Vec<u8> {
    expand(&text(line)).unwrap().as_bytes().unwrap().to_vec()
}

fn assert_too_many_words(line: &str) {
    let err = expand(&text(line)).unwrap_err();
    assert!(
        matches!(err.current_context(), CmdError::Parse),
        "expected Parse context, got: {err:?}"
    );
    assert!(
        matches!(
            err.downcast_ref::<ParseError>(),
            Some(ParseError::BraceExpansionTooManyWords)
        ),
        "expected BraceExpansionTooManyWords, got: {err:?}"
    );
}

fn words(word: &str) -> Option<Vec<Vec<u8>>> {
    expand_word(word.as_bytes()).unwrap()
}

fn seq(start: i64, end: i64, incr: i64) -> SeqSpec {
    SeqSpec {
        start,
        end,
        incr,
        kind: SeqKind::Int,
        width: 0,
    }
}

fn strings(words: Vec<Vec<u8>>) -> Vec<String> {
    words
        .into_iter()
        .map(|b| String::from_utf8(b).unwrap())
        .collect()
}

// --- word: comma groups -------------------------------------------------

#[test]
fn comma_group_cross_product() {
    assert_eq!(
        words("a{b,c}d"),
        Some(vec![b"abd".to_vec(), b"acd".to_vec()])
    );
    assert_eq!(
        words("{a,b}{c,d}"),
        Some(vec![
            b"ac".to_vec(),
            b"ad".to_vec(),
            b"bc".to_vec(),
            b"bd".to_vec()
        ])
    );
}

#[test]
fn nested_comma_group() {
    assert_eq!(
        words("{a,{b,c}}"),
        Some(vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
    );
}

#[test]
fn inner_group_in_failed_sequence() {
    // `{1..{a,b}3}`: the outer sequence is invalid, the inner comma group
    // still expands.
    assert_eq!(
        words("{1..{a,b}3}"),
        Some(vec![b"{1..a3}".to_vec(), b"{1..b3}".to_vec()])
    );
    assert_eq!(
        words("{x..1}y{d,e}"),
        Some(vec![b"{x..1}yd".to_vec(), b"{x..1}ye".to_vec()])
    );
}

#[test]
fn invalid_sequence_with_postamble_stays_literal() {
    assert_eq!(
        words("{a..b..c}x{d,e}"),
        Some(vec![b"{a..b..c}xd".to_vec(), b"{a..b..c}xe".to_vec()])
    );
}

#[test]
fn invalid_sequence_without_postamble_unchanged() {
    assert_eq!(words("{a..b..c}"), None);
    assert_eq!(words("{1..}"), None);
    assert_eq!(words("{..5}"), None);
    assert_eq!(words("{0x1..0x3}"), None);
}

#[test]
fn failed_sequence_with_plain_postamble() {
    // An overflowed bound is an invalid sequence: the literal group times
    // the (unexpanded) postamble.
    assert_eq!(
        words("{1..100000000000000000000}x"),
        Some(vec![b"{1..100000000000000000000}x".to_vec()])
    );
    // An `i64::MIN` increment is invalid (bash: literal); no postamble, so
    // the word is unchanged.
    assert_eq!(words("{1..5..-9223372036854775808}"), None);
}

#[test]
fn empty_words_kept_in_result() {
    assert_eq!(words("{,}"), Some(vec![vec![], vec![]]));
    assert_eq!(words("x{,}"), Some(vec![b"x".to_vec(), b"x".to_vec()]));
    assert_eq!(
        words("\"\"{,}"),
        Some(vec![b"\"\"".to_vec(), b"\"\"".to_vec()])
    );
    assert_eq!(
        words("{a,\"\"}"),
        Some(vec![b"a".to_vec(), b"\"\"".to_vec()])
    );
}

#[test]
fn quoted_braces_are_opaque() {
    assert_eq!(words("\"{a,b}\""), None);
    assert_eq!(words("a\"{b,c}\"d"), None);
}

#[test]
fn substitution_spans_are_opaque() {
    assert_eq!(words("$(echo {a,b})"), None);
    assert_eq!(words("`{a,b}`"), None);
    // A valid group after a substitution span still expands.
    assert_eq!(
        words("$(echo ,){a,b}"),
        Some(vec![b"$(echo ,)a".to_vec(), b"$(echo ,)b".to_vec()])
    );
}

#[test]
fn dollar_brace_counts_as_level() {
    // The `{` of `${…}` raises the level, so its `}` never closes a group.
    assert_eq!(words("${x{a,b}}"), None);
}

#[test]
fn no_group_unchanged() {
    assert_eq!(words("{a"), None);
    assert_eq!(words("a}"), None);
    assert_eq!(words("a{b"), None);
}

// --- word: sequences -----------------------------------------------------

#[test]
fn seq_terms_parse() {
    let s = expand_seqterm(b"1..5").unwrap();
    assert_eq!((s.start, s.end, s.incr, s.kind), (1, 5, 1, SeqKind::Int));
    let s = expand_seqterm(b"1..5..2").unwrap();
    assert_eq!((s.start, s.end, s.incr), (1, 5, 2));
    let s = expand_seqterm(b"1..5..-2").unwrap();
    assert_eq!((s.start, s.end, s.incr), (1, 5, -2));
    let s = expand_seqterm(b"1..10..0").unwrap();
    assert_eq!((s.start, s.end, s.incr), (1, 10, 0));
    let s = expand_seqterm(b"a..c").unwrap();
    assert_eq!((s.start, s.end, s.kind), (97, 99, SeqKind::Char));
    let s = expand_seqterm(b"01..03").unwrap();
    assert_eq!((s.kind, s.width), (SeqKind::ZInt, 2));
    let s = expand_seqterm(b"-01..1").unwrap();
    assert_eq!((s.kind, s.width), (SeqKind::ZInt, 3));
}

#[test]
fn rhs_zero_padding_drives_width() {
    // The width comes from whichever side is padded (bash: `echo {1..05}`
    // prints `01 02 03 04 05`).
    let s = expand_seqterm(b"1..05").unwrap();
    assert_eq!((s.kind, s.width), (SeqKind::ZInt, 2));
    let s = expand_seqterm(b"1..-05").unwrap();
    assert_eq!((s.kind, s.width), (SeqKind::ZInt, 3));
    let s = expand_seqterm(b"1..05").unwrap();
    assert_eq!(
        strings(mkseq(&s).unwrap().unwrap()),
        ["01", "02", "03", "04", "05"]
    );
    let s = expand_seqterm(b"1..-05").unwrap();
    assert_eq!(
        strings(mkseq(&s).unwrap().unwrap()),
        ["001", "000", "-01", "-02", "-03", "-04", "-05"]
    );
}

#[test]
fn seqterm_padding_boundaries() {
    // The padding decision is per side (leading `0` or `-0` plus at least
    // one more byte); each boundary is pinned so an off-by-one in the length
    // or sign checks changes the kind or the width. All match bash 5.3.
    let plain: &[&[u8]] = &[
        b"0..5",   // one-digit `0` lhs is not padding
        b"-0..5",  // `-0` lhs without a third byte is not padding
        b"-5..3",  // `-` prefix without a zero is not padding
        b"-15..3", // `-1` prefix is not padding
        b"105..3", // an inner `0` does not pad
        b"1..25",  // a non-zero-leading rhs does not pad
        b"5..0",   // one-digit `0` rhs is not padding
        b"5..-0",  // `-0` rhs without a third byte is not padding
        b"5..-5",  // `-` prefix without a zero is not padding
        b"5..-15", // `-1` prefix is not padding
    ];
    for s in plain {
        let Some(spec) = expand_seqterm(s) else {
            panic!("{s:?} should parse");
        };
        assert_eq!((spec.kind, spec.width), (SeqKind::Int, 0), "{s:?}");
    }
    let padded: &[(&[u8], usize)] = &[
        (b"01..5", 2),     // the lhs pads
        (b"01..03..2", 2), // the width survives the increment suffix
    ];
    for (s, width) in padded {
        let Some(spec) = expand_seqterm(s) else {
            panic!("{s:?} should parse");
        };
        assert_eq!((spec.kind, spec.width), (SeqKind::ZInt, *width), "{s:?}");
    }
}

#[test]
fn seqterm_increment_after_multi_digit_end_term() {
    // The increment `..` must be checked right after the end term (`ep + 2`);
    // a 3-byte end term is the first case where any off-by-one lands on a
    // different byte.
    let Some(spec) = expand_seqterm(b"1..250..5") else {
        panic!("should parse");
    };
    assert_eq!((spec.start, spec.end, spec.incr), (1, 250, 5));
    assert_eq!(spec.kind, SeqKind::Int);
}

#[test]
fn seq_terms_reject_invalid() {
    let bad: &[&[u8]] = &[
        b"..5",
        b"1..",
        b"..",
        b"a..",
        b"a..b..c",
        b"1..100000000000000000000",
        b"0x1..0x3",
        b"1.5..3",
        b"12x..5",
        b"a..b2",
        // Junk after the end term or the increment term.
        b"1..2x",
        b"1..5..2x",
        // A single dot after the end term is not a sequence increment.
        b"1..5.x3",
        b"a..b.x5",
        // Mixed types (one side int, the other char) are invalid.
        b"a..1",
        b"1..a",
        // A non-alphabetic single-byte lhs is not a char term.
        b"!..b",
        b"a..!",
    ];
    for bad in bad {
        assert!(expand_seqterm(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn valid_seqterm_gates_groups() {
    assert!(valid_seqterm(b"1..5"));
    assert!(valid_seqterm(b"a..c"));
    assert!(valid_seqterm(b"a..c..2"));
    assert!(valid_seqterm(b"1..100000000000000000000"));
    assert!(!valid_seqterm(b"1..x"));
    assert!(!valid_seqterm(b"..5"));
    assert!(!valid_seqterm(b"1.."));
    assert!(!valid_seqterm(b"a..b2"));
    // A two-byte lhs whose first byte is alphabetic but second is not a
    // separator is not a char term.
    assert!(!valid_seqterm(b"a5..5"));
    // No `..` at all is not a sequence term (production always passes a
    // `..`-containing amble; the gate is the contract).
    assert!(!valid_seqterm(b"abc"));
    assert!(!valid_seqterm(b""));
}

#[test]
fn mkseq_generates_elements() {
    assert_eq!(
        strings(mkseq(&seq(1, 5, 1)).unwrap().unwrap()),
        ["1", "2", "3", "4", "5"]
    );
    assert_eq!(
        strings(mkseq(&seq(10, 1, 1)).unwrap().unwrap()),
        ["10", "9", "8", "7", "6", "5", "4", "3", "2", "1"]
    );
    assert_eq!(
        strings(mkseq(&seq(1, 5, 2)).unwrap().unwrap()),
        ["1", "3", "5"]
    );
    assert_eq!(
        strings(mkseq(&seq(1, 5, -2)).unwrap().unwrap()),
        ["1", "3", "5"]
    );
    let mut spec = seq(1, 3, 1);
    spec.kind = SeqKind::ZInt;
    spec.width = 2;
    assert_eq!(strings(mkseq(&spec).unwrap().unwrap()), ["01", "02", "03"]);
}

#[test]
fn mkseq_zero_padding_with_signs() {
    let mut spec = seq(-1, 1, 1);
    spec.kind = SeqKind::ZInt;
    spec.width = 3;
    assert_eq!(
        strings(mkseq(&spec).unwrap().unwrap()),
        ["-01", "000", "001"]
    );
    let mut spec = seq(1, -1, -1);
    spec.kind = SeqKind::ZInt;
    spec.width = 2;
    assert_eq!(strings(mkseq(&spec).unwrap().unwrap()), ["01", "00", "-1"]);
}

#[test]
fn mkseq_char_sequence() {
    let mut spec = seq(97, 99, 1);
    spec.kind = SeqKind::Char;
    assert_eq!(
        mkseq(&spec).unwrap().unwrap(),
        vec![vec![97], vec![98], vec![99]]
    );
}

#[test]
fn mkseq_overflow_is_invalid() {
    // i64::MIN increment: bash treats the sequence as invalid (literal).
    assert_eq!(mkseq(&seq(1, 5, i64::MIN)).unwrap(), None);
    assert_eq!(mkseq(&seq(i64::MAX, i64::MIN, 1)).unwrap(), None);
    // `element_count` reports the same overflow as `Ok(None)`.
    assert_eq!(element_count(&seq(1, 5, i64::MIN)).unwrap(), None);
}

#[test]
fn min_incr_without_flip_is_invalid() {
    // Regression: `i64::MIN` increment when no sign flip is needed
    // (`start >= end`) must be invalid (bash: literal), not a negation
    // overflow. Covers the descending and `start == end` directions.
    assert_eq!(mkseq(&seq(5, 1, i64::MIN)).unwrap(), None);
    assert_eq!(mkseq(&seq(5, 5, i64::MIN)).unwrap(), None);
    assert_eq!(element_count(&seq(5, 1, i64::MIN)).unwrap(), None);
    assert_eq!(element_count(&seq(5, 5, i64::MIN)).unwrap(), None);
    // End-to-end: the word stays literal.
    assert_eq!(words("{5..1..-9223372036854775808}"), None);
    assert_eq!(words("{5..5..-9223372036854775808}"), None);
}

// The cap is asserted at the *count* level, not by materializing 65_536
// words: a full-materialization test sits on the edge of the 128MB
// per-test-binary VA cap once nextest's parallel test threads have claimed
// their stack VA, and aborts flakily. `element_count`/`cross_total` enforce
// the cap before any allocation, so the boundary is pinned cheaply.
#[test]
fn seq_word_cap() {
    assert!(element_count(&seq(1, 70_000, 1)).is_err());
    // Exactly MAX_WORDS is admitted.
    assert_eq!(
        element_count(&seq(1, MAX_WORDS as i64, 1)).unwrap(),
        Some(MAX_WORDS)
    );
    // One over is an error (pins the boundary).
    assert!(element_count(&seq(1, MAX_WORDS as i64 + 1, 1)).is_err());
}

#[test]
fn cross_product_cap() {
    // 256 x 256 = 65_536 = MAX_WORDS: admitted.
    assert_eq!(cross_total(256, 256).unwrap(), MAX_WORDS);
    // 257 x 257 = 66_049 > MAX_WORDS: error (pins the boundary).
    assert!(cross_total(257, 257).is_err());
    // End-to-end: an over-cap cross surfaces as a parse error.
    assert!(expand_word(b"{1..300}{1..300}").is_err());
}

#[test]
fn cross_single_empty_word_is_identity() {
    // A single empty word is the identity on either side.
    assert_eq!(
        cross(vec![vec![]], vec![b"a".to_vec()]).unwrap(),
        vec![b"a".to_vec()]
    );
    assert_eq!(
        cross(vec![b"a".to_vec(), b"b".to_vec()], vec![vec![]]).unwrap(),
        vec![b"a".to_vec(), b"b".to_vec()]
    );
}

#[test]
fn cross_two_words_with_empty_first_is_not_identity() {
    // The identity short-circuit needs a single empty word on a side; an
    // empty first word among two still crosses.
    assert_eq!(
        cross(vec![vec![], b"x".to_vec()], vec![b"a".to_vec()]).unwrap(),
        vec![b"a".to_vec(), b"xa".to_vec()]
    );
}

#[test]
fn count_overflow_is_over_cap() {
    // `0..i64::MAX` has i64::MAX + 1 elements: the count itself overflows
    // `i64` before the cap compare (bash: allocation failure + literal).
    let err = element_count(&seq(0, i64::MAX, 1)).unwrap_err();
    assert!(matches!(
        err.downcast_ref::<ParseError>(),
        Some(ParseError::BraceExpansionTooManyWords)
    ));
    // End-to-end: the same range through a word is a clean error.
    assert!(expand_word(b"{0..9223372036854775807}").is_err());
    // A usize product overflow also errors.
    assert!(cross_total(usize::MAX, 2).is_err());
}

// --- gobbler --------------------------------------------------------------

#[test]
fn gobbler_finds_groups() {
    let t = b"a{b,c}d";
    assert_eq!(gobble(t, 0, b'{'), (1, true, None));
    assert_eq!(gobble(t, 2, b'}'), (5, true, Some(GroupType::Comma)));
    let t = b"x{1..5}y";
    assert_eq!(gobble(t, 1, b'{'), (1, true, None));
    assert_eq!(gobble(t, 2, b'}'), (6, true, Some(GroupType::Seq)));
    // A comma outranks `..`.
    let t = b"{1..2,3..4}";
    assert_eq!(gobble(t, 1, b'}'), (10, true, Some(GroupType::Comma)));
    // `..` directly before `}` does not count as a separator.
    assert_eq!(gobble(b"{1..}", 1, b'}'), (5, false, None));
    // Quoted separators do not count.
    assert_eq!(gobble(b"{a\",b\"c}", 1, b'}'), (8, false, None));
    // An inner group does not close the outer one.
    assert_eq!(
        gobble(b"{a,{b,c}}", 1, b'}'),
        (8, true, Some(GroupType::Comma))
    );
    // No group:
    assert_eq!(gobble(b"{a}", 1, b'}'), (3, false, None));
    assert_eq!(gobble(b"abc", 0, b'{'), (3, false, None));
}

#[test]
fn gobbler_skips_dollar_paren() {
    let t = b"$(echo {a,b}){c,d}";
    // The `{` inside `$(…)` is skipped; the first top-level `{` is the later one.
    assert_eq!(gobble(t, 0, b'{'), (13, true, None));
    assert_eq!(gobble(t, 14, b'}'), (17, true, Some(GroupType::Comma)));
}

#[test]
fn gobbler_backslash_in_quotes_shields_next_byte() {
    // Inside a quoted span a backslash shields the next byte, so the `{`
    // there never opens a group; a group after the span still counts.
    let t = b"a\"x\\y\"{b,c}";
    assert_eq!(gobble(t, 0, b'{'), (6, true, None));
    assert_eq!(gobble(t, 7, b'}'), (10, true, Some(GroupType::Comma)));
    assert_eq!(
        words("a\"x\\y\"{b,c}"),
        Some(vec![b"a\"x\\y\"b".to_vec(), b"a\"x\\y\"c".to_vec()])
    );
}

#[test]
fn gobbler_unbalanced_dollar_paren_swallows_word() {
    // An unterminated `$(…` spans to the end of the word: no group is found.
    assert_eq!(gobble(b"a$(b{c,d}", 0, b'{'), (9, false, None));
    assert_eq!(words("a$(b{c,d}"), None);
}

#[test]
fn gobbler_ignores_blank_bounded_open_brace() {
    // The bash rule: a top-level `{` preceded by a blank and followed by a
    // blank is not a group opener, so no group is found.
    assert_eq!(gobble(b"x { b}", 0, b'{'), (6, false, None));
    // Followed by `}` instead of a blank: likewise ignored.
    assert_eq!(gobble(b"x {}", 0, b'{'), (4, false, None));
}

#[test]
fn gobbler_dollar_paren_span_resumes_after_close() {
    // A `{` inside the `$(…)` body is opaque; scanning resumes at the byte
    // just past the span's closing `)`, so the group after the span expands.
    assert_eq!(
        words("$(a{){b,c}"),
        Some(vec![b"$(a{)b".to_vec(), b"$(a{)c".to_vec()])
    );
}

#[test]
fn gobbler_dollar_without_paren_is_ordinary() {
    // A `$` not followed by `(` or `{` is an ordinary byte: the group after
    // it still expands.
    assert_eq!(
        words("$x{a,b}"),
        Some(vec![b"$xa".to_vec(), b"$xb".to_vec()])
    );
}

#[test]
fn gobbler_group_immediately_after_span() {
    // The group after a `$(…)` span is found only if scanning resumes just
    // past the span's closing `)`. With the span not at index 0, resuming
    // late skips that `{`; resuming early is corrected by the group search
    // (an inner `{` of the span never closes).
    assert_eq!(
        words("xx$(){a,b}"),
        Some(vec![b"xx$()a".to_vec(), b"xx$()b".to_vec()])
    );
}

// --- protected tokens ------------------------------------------------------

#[test]
fn heredoc_delimiter_tokens_protected() {
    // Bare form: the delimiter is the next token.
    let line = b"cat << {a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(protected(line, &tokens), vec![2]);
    // Attached form: the operator token carries the delimiter.
    let line = b"cat <<{a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(protected(line, &tokens), vec![1]);
}

#[test]
fn heredoc_terminator_line_protected() {
    // The terminating delimiter line survives tokenization as a word (after
    // the `;` separator the preceding newline emits); it must stay literal or
    // the heredoc would never terminate.
    let line = b"cat <<{a,b}\nBODY\n{a,b}";
    let tokens = tokenize_statement(line).unwrap();
    let prot = protected(line, &tokens);
    // The terminator line `{a,b}` starts at offset 17.
    let Some(ti) = tokens.iter().position(|(_, s, _, _, _)| *s == 17) else {
        panic!("no token at the terminator offset");
    };
    assert!(prot.contains(&ti), "prot={prot:?}");
}

#[test]
fn here_string_word_protected() {
    let line = b"cat <<< {a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(protected(line, &tokens), vec![2]);
    let line = b"cat <<<{a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(protected(line, &tokens), vec![1]);
}

#[test]
fn assignment_word_protected() {
    let line = b"x={a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(protected(line, &tokens), vec![0]);
    let line = b"echo x={a,b}";
    let tokens = tokenize_statement(line).unwrap();
    // Argument positions are not assignments: not protected.
    assert_eq!(protected(line, &tokens), Vec::<usize>::new());
}

#[test]
fn case_word_and_patterns_protected() {
    let line = b"case {a,b} in {x,y}) echo hi ;; esac";
    let tokens = tokenize_statement(line).unwrap();
    let prot = protected(line, &tokens);
    // word (1) and pattern (3) are protected; the body word `hi` (5) is not.
    assert!(prot.contains(&1));
    assert!(prot.contains(&3));
    assert!(!prot.contains(&5));
}

#[test]
fn ordinary_word_not_protected() {
    let line = b"echo {a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(protected(line, &tokens), Vec::<usize>::new());
}

// --- line level -------------------------------------------------------------

#[test]
fn fast_path_without_brace() {
    assert_eq!(expand_line("echo hello"), b"echo hello".to_vec());
}

#[test]
fn line_rebuild_expands_words() {
    assert_eq!(expand_line("echo {a,b}{c,d}"), b"echo ac ad bc bd".to_vec());
    assert_eq!(expand_line("echo {1..5}"), b"echo 1 2 3 4 5".to_vec());
    assert_eq!(expand_line("x={a,b}"), b"x={a,b}".to_vec());
}

#[test]
fn empty_words_drop_on_retokenization() {
    // `{,}` yields two empty words, which vanish: `echo` gets no arguments.
    let tokens = tokenize_statement(&expand_line("echo {,}")).unwrap();
    assert_eq!(tokens.len(), 1);
    // Quoted empties survive.
    let tokens = tokenize_statement(&expand_line("echo \"\"{,}")).unwrap();
    assert_eq!(tokens.len(), 3);
}

#[test]
fn comment_with_brace_is_verbatim() {
    assert_eq!(expand_line("echo a # {b,c}"), b"echo a # {b,c}".to_vec());
}

#[test]
fn heredoc_body_is_verbatim() {
    let line = "cat <<EOF\n{a,b}\nEOF";
    assert_eq!(expand_line(line), line.as_bytes().to_vec());
}

#[test]
fn heredoc_brace_delimiter_is_verbatim() {
    // A delimiter that looks like a brace group must stay literal, both in
    // the operator and in the terminating line (regression: the terminator
    // line was brace-expanded and the heredoc never terminated).
    let line = "cat <<{a,b}\nBODY\n{a,b}";
    assert_eq!(expand_line(line), line.as_bytes().to_vec());
}

#[test]
fn unclosed_group_unchanged() {
    assert_eq!(expand_line("echo {a"), b"echo {a".to_vec());
}

#[test]
fn quoted_brace_word_unchanged() {
    assert_eq!(expand_line("echo \"{a,b}\""), b"echo \"{a,b}\"".to_vec());
}

#[test]
fn cap_error_at_line_level() {
    assert_too_many_words("echo {1..300}{1..300}");
}
