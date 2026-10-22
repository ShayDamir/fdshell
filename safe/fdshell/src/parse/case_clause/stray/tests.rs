#![allow(clippy::unwrap_used)]

use super::opens_next_pattern;
use crate::parse::token::tokenize_statement;

/// [`opens_next_pattern`] over the tokens of the statement `line`, with `limit`
/// as the index of the `;;` window that ends the arm region (`None`: the run up
/// to `esac`). This is the token list the parse layer scans (`case_clause::
/// extract::body`), heredoc bodies blanked and byte offsets preserved, and `line`
/// is the statement's own text, so the keyword gate [`super::keyword_word`] reads
/// the spans the tokens were built from.
fn opens(line: &[u8], limit: Option<usize>) -> bool {
    opens_next_pattern(&tokenize_statement(line).unwrap(), limit, line)
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

/// The parens of groups the **arm itself opened** are arm text, never the next
/// clause's pattern paren: a definer's parameter list `f(` (bash parses the
/// function definition inside the arm, rc 0), and the `)` of a subshell group
/// the arm wrote (`(cat`, a bare `(`). The arm's own paren nesting is what
/// distinguishes them from a clause pattern, which sits behind a plain word at
/// depth 0.
#[test]
fn a_paren_closing_a_group_the_arm_opened_is_not_a_pattern_paren() {
    // A definer's parameter list, at the arm's start and behind a `;` that
    // restarts command position.
    assert!(!opens(b"f() { :; }", None));
    assert!(!opens(b"echo hi;f() { :; }; f", None));
    // A definer inside the arm's own block bodies (the `;` behind `then`/`do`/
    // `}` is command position, so only this gate keeps these parsing).
    assert!(!opens(b"if true; then f() { :; }; fi", None));
    assert!(!opens(b"while false; do f() { :; }; done", None));
    assert!(!opens(b"for i in 1; do f() { :; }; done", None));
    assert!(!opens(b"{ f() { :; }; }", None));
    // A subshell group the arm opened: single word, pipeline stage, the `g() ( : )`
    // definer whose body is a subshell, and a subshell behind `&&`.
    assert!(!opens(b"(cat)", None));
    assert!(!opens(b"echo hi|(cat)", None));
    assert!(!opens(b"g() ( : )", None));
    // The same group with its closer behind a **command-position** word (`:` is
    // the subshell's command): the arm's own nesting protects it, so a rule that
    // only reads the neighbouring word is not enough.
    assert!(!opens(b"( : )", None));
    assert!(!opens(b"(true) && echo y", None));
    // A definer glued to its subshell body has no space between the `)` and the
    // `(`, so the subshell's `)` follows the bare `(` word.
    assert!(!opens(b"g() (:)", None));
}

/// The depth returns to 0 when the arm's group closes, so a clause paren behind
/// a plain word still bails after a function definition or a subshell has ended
/// (`case x in a) f() { :; }; b) ;; esac` and `case x in a) (true); b) ;; esac`
/// are bash rc 2, like every other missing `;;`).
#[test]
fn a_clause_paren_behind_a_word_after_a_closed_group_still_bails() {
    assert!(opens(b"f() { :; }; b)", None));
    assert!(opens(b"(true); b)", None));
    assert!(opens(b"g() ( : ); b)", None));
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
    let tokens = tokenize_statement(b"a)").unwrap();
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

/// A heredoc **body** is blanked by `tokenize_statement`, so it is never in the
/// arm region: the `)` of a body line, and even a word glued to one, is body
/// text, not a pattern paren (`case x in x) cat <<T` newline `x)` newline `T`
/// newline `esac` prints `x)` verbatim in bash, rc 0).
#[test]
fn a_heredoc_body_written_in_an_arm_is_not_in_the_region() {
    assert!(!opens(b"case x in x) cat <<T\nx)\n)\nT\nesac", None));
}
