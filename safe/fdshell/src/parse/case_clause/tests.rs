#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::CaseClause;
use super::parse_clauses;
use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::parse::token::tokenize_statement;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The clause list of a single-line `case`: `parse_clauses` from the token right
/// after the `in` to the block's last token, the closing `esac` (the shape
/// `case_block::tokens_to_case` hands it, with the word walk left out).
fn clauses_of(line: &[u8]) -> Result<Vec<CaseClause>, Report<ParseError>> {
    let tokens = tokenize_statement(line).unwrap();
    let in_idx = (1..tokens.len())
        .find(|&i| tokens.get(i).is_some_and(|t| t.0.eq_bytes(b"in")))
        .unwrap();
    let esac_idx = tokens.len() - 1;
    parse_clauses(&tokens, in_idx + 1, esac_idx, &stext(line))
}

/// The task's shape: a clause closed **directly** by `esac` is a clause with an
/// empty body, exactly as the `;;`-terminated empty clause is
/// (`parse::tests::case_empty_body_clause`). bash runs it at rc 0.
#[test]
fn a_clause_closed_by_esac_has_an_empty_body() {
    for line in [
        b"case \"x\" in a) esac".as_slice(),
        b"case x in a|b) esac".as_slice(),
        b"case x in \"a\") esac".as_slice(),
        // The lone `;` after the pattern's `)` is the leniency the plan's open
        // question 2 pins as measured (bash rc 2, fdshell rc 0): the clause walk
        // skips it as a leading `;` and the body it leaves is empty.
        b"case x in a) ; esac".as_slice(),
        // The multi-line spelling reaches the same walk through the newline's `;`
        // token, which is why `pos` lands one short of the `esac` index.
        b"case x in\na)\nesac".as_slice(),
    ] {
        let clauses =
            clauses_of(line).unwrap_or_else(|e| panic!("{line:?}: {}", e.current_context()));
        assert_eq!(clauses.len(), 1, "{line:?}");
        assert!(clauses[0].body.data.is_empty(), "{line:?}");
    }
    // The last clause of a `;;`-separated block, closed at `esac` (bash rc 0).
    let clauses = clauses_of(b"case x in a) echo one;; b) esac").unwrap();
    assert_eq!(clauses.len(), 2);
    assert_eq!(clauses[0].body.data, ShortCStr::from(c"echo one"));
    assert!(clauses[1].body.data.is_empty());
}

/// The missing-paren verdict stays the **paren** rule it always was: a pattern
/// list that never reaches its `)` (the position, which used to be the test, is
/// not the shape — `case x in a) esac` reaches `esac_idx` right after a
/// successful `)` and is a valid block).
#[test]
fn a_pattern_list_that_never_closes_reports_the_missing_paren() {
    for line in [
        b"case x in a esac".as_slice(),
        b"case \"x\" in echo yes;; esac".as_slice(),
    ] {
        let err = clauses_of(line)
            .err()
            .unwrap_or_else(|| panic!("{line:?} is ok"));
        assert_eq!(
            err.current_context().to_string(),
            "case: missing ')'",
            "{line:?}"
        );
    }
}

/// The empty-pattern checks keep their places, in front of the paren verdict for
/// the `)` that opens the list: `case x in ) esac` has a paren, and no pattern.
/// The three leading-`(` rows are the same verdict through the **token drop** of
/// task #334: the `(` is POSIX's standalone token, so taking it off leaves the
/// clause with no pattern at all — not a pattern that matches the empty word.
#[test]
fn an_empty_pattern_reports_the_empty_pattern() {
    for line in [
        b"case x in |) echo yes;; esac".as_slice(),
        b"case x in ) esac".as_slice(),
        b"case x in () echo yes;; esac".as_slice(),
        b"case x in ( ) echo yes;; esac".as_slice(),
        b"case x in (|) echo yes;; esac".as_slice(),
    ] {
        let err = clauses_of(line)
            .err()
            .unwrap_or_else(|| panic!("{line:?} is ok"));
        assert_eq!(
            err.current_context().to_string(),
            "case: empty pattern",
            "{line:?}"
        );
    }
}

/// Task #334: bash strips POSIX 2.9.4.3's optional leading `(` of a pattern
/// list, so the clause's first pattern text starts **behind** it. POSIX's
/// spaced `( pattern )` spelling is the case where the `(` is a whole token of
/// its own: it goes away, so [`try_join_masked`] inserts no leading space and
/// the text is exactly the pattern.
#[test]
fn the_leading_open_paren_is_taken_off_the_clause_s_first_pattern() {
    for line in [
        b"case x in (a*) esac".as_slice(),
        b"case x in ( a* ) esac".as_slice(),
        b"case x in (  a*  ) esac".as_slice(),
        b"case x in (a*) echo one;; esac".as_slice(),
    ] {
        let clauses =
            clauses_of(line).unwrap_or_else(|e| panic!("{line:?}: {}", e.current_context()));
        assert_eq!(clauses.len(), 1, "{line:?}");
        assert_eq!(clauses[0].patterns.len(), 1, "{line:?}");
        assert_eq!(
            clauses[0].patterns[0].text,
            ShortCStr::from(c"a*"),
            "{line:?}"
        );
    }
}

/// One token, one byte, the **first** pattern of the clause only: the `|` split
/// keeps a `(` written behind the separator as pattern text, `((a*))` keeps its
/// inner `(`, and a `(`-prefixed pattern list reaches both arms of a multi-clause
/// walk.
#[test]
fn only_the_first_byte_of_the_first_pattern_is_taken() {
    let clauses = clauses_of(b"case x in ( a*|z*) esac").unwrap();
    let texts: Vec<ShortCStr> = clauses[0].patterns.iter().map(|p| p.text.clone()).collect();
    assert_eq!(texts, [ShortCStr::from(c"a*"), ShortCStr::from(c"z*")]);
    let clauses = clauses_of(b"case x in z|(a) echo one;; esac").unwrap();
    let texts: Vec<ShortCStr> = clauses[0].patterns.iter().map(|p| p.text.clone()).collect();
    assert_eq!(texts, [ShortCStr::from(c"z"), ShortCStr::from(c"(a")]);
    let clauses = clauses_of(b"case x in ((a*)) echo one;; esac").unwrap();
    assert_eq!(clauses[0].patterns[0].text, ShortCStr::from(c"(a*"));
    // Every clause of the walk is a clause: each opens its own pattern list.
    let clauses = clauses_of(b"case x in (x) echo one;; (a*) echo two;; esac").unwrap();
    let texts: Vec<ShortCStr> = clauses.iter().map(|c| c.patterns[0].text.clone()).collect();
    assert_eq!(texts, [ShortCStr::from(c"x"), ShortCStr::from(c"a*")]);
}

/// The mask is the discriminator: a quoted `(` keeps its bit, so `"(a*` and
/// `'('a*` keep the byte as pattern text; an escaped `\(` keeps **both** bytes of
/// the pair in the text (the mask marks them unquoted), so the text test excludes
/// it. Every mask stays parallel to its text.
#[test]
fn a_quoted_or_escaped_open_paren_stays_pattern_text() {
    for (line, want) in [
        (
            b"case x in \"(\"a*) echo one;; esac".as_slice(),
            MaskBit::QuotedStart,
        ),
        (
            b"case x in '('a*) echo one;; esac".as_slice(),
            MaskBit::QuotedStart,
        ),
    ] {
        let clauses = clauses_of(line).unwrap();
        let pattern = &clauses[0].patterns[0];
        assert_eq!(pattern.text, ShortCStr::from(c"(a*"), "{line:?}");
        assert_eq!(pattern.mask.first(), Some(&want), "{line:?}");
        assert_eq!(pattern.mask.len(), pattern.text.len(), "{line:?}");
    }
    // The escape pair stays in the text, both bits unquoted.
    let clauses = clauses_of(b"case x in \\(a*) echo one;; esac").unwrap();
    let pattern = &clauses[0].patterns[0];
    assert_eq!(pattern.text, ShortCStr::from(c"\\(a*"));
    assert_eq!(pattern.mask.first(), Some(&MaskBit::Unquoted));
    assert_eq!(pattern.mask.len(), 4);
}

/// The text, the mask and the **zero-byte quote spans** (#321) move together
/// with the stripped byte: byte 0, bit 0, and every span index steps back one —
/// a pair written in front of the `(` saturates at 0, which is what keeps the
/// tilde gate seeing `~""/x` as a quoted prefix.
#[test]
fn the_mask_and_the_spans_stay_parallel_across_the_stripped_paren() {
    // The empty pair contributes no byte: text `(a*`, spans `[0]` before the
    // strip, `a*` with spans `[0]` after it (saturating).
    let clauses = clauses_of(b"case x in \"\"(a*) echo one;; esac").unwrap();
    let pattern = &clauses[0].patterns[0];
    assert_eq!(pattern.text, ShortCStr::from(c"a*"));
    assert_eq!(pattern.mask.len(), pattern.text.len());
    assert_eq!(pattern.spans, vec![0]);
    // The pair sits behind the `(`: the span index follows the text.
    let clauses = clauses_of(b"case x in (~\"\"/x) echo one;; esac").unwrap();
    let pattern = &clauses[0].patterns[0];
    assert_eq!(pattern.text, ShortCStr::from(c"~/x"));
    assert_eq!(pattern.mask.len(), pattern.text.len());
    assert_eq!(pattern.spans, vec![1]);
    // The same pattern with no `(` to strip is the control: same text, same span.
    let clauses = clauses_of(b"case x in ~\"\"/x) echo one;; esac").unwrap();
    let control = &clauses[0].patterns[0];
    assert_eq!(control.text, pattern.text);
    assert_eq!(control.spans, pattern.spans);
    assert_eq!(control.mask, pattern.mask);
}

/// Every separator word of the clause walk is [`span_word`]'s (task #336):
/// quoted, a `;` is no clause-leading skip, a `|` is no pattern separator, a `)`
/// is no pattern closer and a `;;` closes no clause — the bytes are pattern and
/// body **text**. Each row is the bash-measured shape.
#[test]
fn a_quoted_separator_word_is_pattern_or_body_text() {
    // A `|` inside a pattern word is pattern text: one pattern, quotes stripped.
    let clauses = clauses_of(b"case x in a\"|\"b) echo A;; esac").unwrap();
    assert_eq!(clauses[0].patterns.len(), 1);
    assert_eq!(clauses[0].patterns[0].text, ShortCStr::from(c"a|b"));
    // The same separator as its **own** quoted word joins into the pattern with
    // the token-joining space, and stays one pattern.
    let clauses = clauses_of(b"case x in a \"|\" b) echo A;; esac").unwrap();
    assert_eq!(clauses[0].patterns.len(), 1);
    assert_eq!(clauses[0].patterns[0].text, ShortCStr::from(c"a | b"));
    // The unquoted twin of both rows is the two-pattern split.
    let clauses = clauses_of(b"case x in a|b) echo A;; esac").unwrap();
    assert_eq!(clauses[0].patterns.len(), 2);
    // A quoted `;` is a pattern **word**, never the leading skip: the pattern
    // list it opens still needs its `)`.
    let clauses = clauses_of(b"case x in \";\" a) echo A;; esac").unwrap();
    assert_eq!(clauses[0].patterns[0].text, ShortCStr::from(c"; a"));
    assert!(clauses_of(b"case x in \";\" a esac").is_err());
    // A quoted `)` is pattern text, so the list never closes: bash's syntax
    // error, fdshell's missing paren.
    let err = clauses_of(b"case x in a\")\" esac").err().unwrap();
    assert_eq!(err.current_context().to_string(), "case: missing ')'");
    // A quoted `;;` is body text: the clause stays open until the real one.
    let clauses = clauses_of(b"case x in a) echo A \";;\" ;; esac").unwrap();
    assert_eq!(clauses.len(), 1);
    assert_eq!(clauses[0].body.data, ShortCStr::from(c"echo A \";;\""));
    // The unquoted `;;` ends the clause, and the next clause's body starts
    // behind it.
    let clauses = clauses_of(b"case x in a) echo A;; b) echo B;; esac").unwrap();
    assert_eq!(clauses.len(), 2);
    assert_eq!(clauses[0].body.data, ShortCStr::from(c"echo A"));
}
