#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::CaseClause;
use super::parse_clauses;
use crate::error::parse::ParseError;
use crate::parse::token::tokenize_statement;
use alloc::string::ToString;
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
        // The multi-line spelling reaches the same walk: the tokenizer turns a
        // newline into a `;` token, so `pos` already landed one short of `esac`.
        b"case x in a) ; esac".as_slice(),
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
#[test]
fn an_empty_pattern_reports_the_empty_pattern() {
    for line in [
        b"case x in |) echo yes;; esac".as_slice(),
        b"case x in ) esac".as_slice(),
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
