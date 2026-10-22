#![allow(clippy::unwrap_used)]

use super::opens_next_pattern;
use crate::parse::token::tokenize;

/// [`opens_next_pattern`] over the tokens of `line`, with `limit` as the index of
/// the `;;` window that ends the arm region (`None`: the run up to `esac`).
/// `line` is the statement's own text, so the keyword gate [`super::keyword_word`]
/// reads the spans the tokens were built from.
fn opens(line: &[u8], limit: Option<usize>) -> bool {
    opens_next_pattern(&tokenize(line).unwrap(), limit, line)
}

/// The shape of the bug: the arm is the word `b` and the `)` is the pattern paren
/// of the clause that is missing its `;;` before it. bash reads the arm as a
/// command list, takes the `)` as a token that list cannot take, and stops at
/// rc 2; fdshell answers `case: missing ';;' between clauses` at rc 1.
#[test]
fn a_paren_behind_a_command_position_word_opens_the_next_pattern() {
    assert!(opens(b"b)", None));
    // The `;` restarts command position, so the same shape behind a `;` bails
    // (`a) echo hi; b) ;; esac`).
    assert!(opens(b"echo a;b)", None));
    // An assignment word is a command word here (`a) x=1) ;; esac`, bash rc 2).
    assert!(opens(b"x=1)", None));
}

/// A paren behind an **argument** is arm text, not a pattern paren: this is the
/// family task #419 owns (bash rc 2, accepted here), kept open so task #316's
/// subshell-shaped arm `a) (echo sub) ;;` keeps parsing.
#[test]
fn a_paren_behind_an_argument_stays_arm_text() {
    assert!(!opens(b"echo hi)", None));
    assert!(!opens(b"(echo sub)", None));
    assert!(!opens(b"echo a[b)", None));
    assert!(!opens(b"echo (b)", None));
}

/// Arithmetic is data: the tokenizer emits every unquoted `)` as its own token,
/// so `((1))` is the tokens `((1`, `)`, `)` — the second `)` follows an operator
/// — and the `((` region swallows the bare `)` tokens an arith `for` clause
/// writes behind its `;` (`case x in x) for ((i=0;i<2;i++)); do …; done;; esac`
/// is bash rc 0).
#[test]
fn the_arithmetic_region_and_its_operators_are_data() {
    assert!(!opens(b"((1))", None));
    assert!(!opens(b"for ((i=0;i<2;i++))", None));
    // The region ends at its own `)`: the clause paren written right behind a
    // complete arithmetic command bails (`a) ((1)); b) ;; esac`, bash rc 2).
    assert!(opens(b"((1)); b)", None));
}

/// The keyword gate: a quoted `")"` is a word whose **text** is `)`, and an
/// escaped `\)` keeps its pair in the word text, so both stay arm text
/// (`case x in a) echo ")" ;; esac` and `a) echo \);; esac` are bash rc 0).
#[test]
fn a_quoted_or_escaped_paren_is_word_text() {
    assert!(!opens(b"echo \")\"", None));
    assert!(!opens(b"echo \\)", None));
}

/// The bound is the `;;` **window**, so the next clause's own pattern parens are
/// never in scope: the arm `echo hi` of `a) echo hi;; b) ;; esac` is clean, and
/// the same region taken past the separator is not.
#[test]
fn the_limit_is_the_semi_semi_window() {
    // `echo hi;;b)`: the window sits at index 2.
    assert!(!opens(b"echo hi;;b)", Some(2)));
    assert!(!opens(b"echo hi;;b)", Some(4)));
    // Past the window (the mutation of the bound) the clause's paren is seen.
    assert!(opens(b"echo hi;;b)", Some(6)));
    assert!(opens(b"echo hi;;b)", None));
}

/// A region that *starts* at the paren has no word before it, and an arm whose
/// paren sits in a word-list place (a nested `case` arm, behind `in`) is
/// untouched: `case a in a) case b in b) echo y;; esac;; esac` is bash rc 0
/// (task #418 owns the shape; the clause walk must not make it worse).
#[test]
fn a_paren_with_no_word_before_it_is_not_a_pattern_paren() {
    let tokens = tokenize(b"a)").unwrap();
    let region = tokens.get(1..).unwrap();
    assert!(!opens_next_pattern(region, None, b"a)"));
    assert!(!opens(b"case b in b)", None));
}

/// The operator tokens the scan reads as predecessors: a `)` behind the `;;`
/// pair, behind a pipeline `|`, or behind a `&` is never a pattern paren.
#[test]
fn a_paren_behind_an_operator_token_is_not_a_pattern_paren() {
    assert!(!opens(b";;)", None));
    assert!(!opens(b"|)", None));
    assert!(!opens(b"&)", None));
}
