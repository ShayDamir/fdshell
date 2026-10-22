#![allow(clippy::unwrap_used)]

use super::{opens_groups, opens_next_pattern};
use crate::mask::MaskBit;
use crate::parse::token::tokenize_statement;
use alloc::vec;

/// [`opens_next_pattern`] over the tokens of the statement `line`, with `limit`
/// as the index of the `;;` window that ends the arm region (`None`: the run up
/// to `esac`). This is the token list the parse layer scans (`case_clause::
/// extract::body`), heredoc bodies blanked and byte offsets preserved, and `line`
/// is the statement's own text, so the keyword gate [`super::keyword_word`] reads
/// the spans the tokens were built from.
fn opens(line: &[u8], limit: Option<usize>) -> bool {
    opens_next_pattern(&tokenize_statement(line).unwrap(), limit, line)
}

/// [`opens_groups`] on a word and a mask written **directly**: the mask the
/// tokenizer produces for the real shapes is pinned through [`opens`] above, and
/// these are the counter's own contract, byte by byte.
fn groups(word: &sys::ShortCStr, mask: &[MaskBit]) -> usize {
    opens_groups(word.as_bytes().unwrap(), mask)
}

/// The counter's own byte tests: the paren of a definer, a subshell and an array
/// literal all count, the arithmetic `((` never does (at the word's edge nor
/// mid-word), and a quoted paren is word text.
#[test]
fn the_word_own_parens_are_counted_over_its_bytes() {
    let mask = |n: usize| vec![MaskBit::Unquoted; n];
    assert_eq!(groups(&sys::ShortCStr::from(c"f("), &mask(2)), 1);
    assert_eq!(groups(&sys::ShortCStr::from(c"(cat"), &mask(4)), 1);
    assert_eq!(groups(&sys::ShortCStr::from(c"m=(1"), &mask(4)), 1);
    assert_eq!(groups(&sys::ShortCStr::from(c"m=("), &mask(3)), 1);
    assert_eq!(groups(&sys::ShortCStr::from(c"m=($@"), &mask(5)), 1);
    // `((` is arithmetic's own pair, at the word's edge and mid-word: `m=$((1)`
    // would hold the arm's depth above 0 and swallow the next clause's paren.
    assert_eq!(groups(&sys::ShortCStr::from(c"((1"), &mask(3)), 0);
    assert_eq!(groups(&sys::ShortCStr::from(c"m=$((1"), &mask(6)), 0);
    // A quoted paren is word text.
    let quoted = vec![MaskBit::Unquoted, MaskBit::QuotedStart, MaskBit::Quoted];
    assert_eq!(groups(&sys::ShortCStr::from(c"x(b"), &quoted), 0);
}

/// The escape arm reads the **mask**, not just the byte. A quoted `\` is word
/// text and consumes nothing behind it, so the `(` right behind it opens its
/// group; an unquoted escape pair is one literal byte, so the paren it carries
/// opens nothing. The mask is written directly because the tokenizer hands a
/// quoted `\` over as a **pair** of quoted bytes (`'\'(x)` and `"\\"(x)` are both
/// the word `\\(x`, mask quoted, quoted, unquoted, unquoted), where the pair's
/// skip and the two single steps land on the same byte — no tokenized shape can
/// show the difference, and this is the mask a **derived** word carries
/// (`MaskBit::from_quoted`, the reading expansion output and a folded word use).
#[test]
fn an_escape_pair_is_read_through_the_mask() {
    let quoted_backslash = vec![MaskBit::Quoted, MaskBit::Unquoted, MaskBit::Unquoted];
    assert_eq!(groups(&sys::ShortCStr::from(c"\\(x"), &quoted_backslash), 1);
    // The unquoted pair is the other half: `\` + `(` is one literal byte.
    let unquoted = vec![MaskBit::Unquoted; 3];
    assert_eq!(groups(&sys::ShortCStr::from(c"x\\("), &unquoted), 0);
    // `x\\(x`: the pair is a literal backslash, so the `(` behind it is the
    // group's own.
    let two = vec![MaskBit::Unquoted; 5];
    assert_eq!(groups(&sys::ShortCStr::from(c"x\\\\(x"), &two), 1);
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

/// An **array-literal assignment** carries its group's `(` **mid-word**: the
/// tokenizer's word is `m=(1` (mask: every byte `Unquoted`) and the array's `)`
/// is a token of its own, so only a scan over the word's whole byte range sees
/// the group the arm opened; a test on the word's two edges misses it and judges
/// the array's `)` the next clause's pattern paren. bash parses the assignment
/// inside the arm, rc 0 — running it is task **#422**'s word-model half, the
/// parse gate is this rule's.
#[test]
fn an_array_literal_opens_a_group_the_arm_owns() {
    assert!(!opens(b"m=(1)", None));
    assert!(!opens(b"arr=(\"$@\")", None));
    assert!(!opens(b"x=(1) y=2", None));
    assert!(!opens(b"m=(1 2)", None));
    assert!(!opens(b"m=()", None));
    assert!(!opens(b"m=($(echo a))", None));
    // The same assignment written inside the arm's own block bodies.
    assert!(!opens(b"if true; then m=(1); fi", None));
    assert!(!opens(b"{ m=(1); }", None));
    assert!(!opens(b"while false; do m=(1); done", None));
    // The array's `)` closes the group, so the clause paren written behind it is
    // seen (`case x in a) m=(1); b) ;; esac` is bash rc 2).
    assert!(opens(b"m=(1); b)", None));
    // `((` stays arithmetic's own, mid-word included: `m=$((1))` is one word whose
    // parens close inside it, so the arm's depth must not rise — a scan that
    // counted them would swallow the clause paren after it (`case x in a)
    // m=$((1)); b) ;; esac` is bash rc 2, and the bare arith assignment is rc 0).
    assert!(!opens(b"m=$((1))", None));
    assert!(!opens(b"m=$((m+1))", None));
    assert!(opens(b"m=$((1)); b)", None));
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
    // A quoted `(` opens no group, so a clause paren behind it is still seen
    // through the word (`echo "x(" b) ;;` is the accepted #419 shape: the
    // predecessor sits in argument position).
    assert!(!opens(b"echo \"x(\" b)", None));
    // A quoted `(` at the arm's own start is word text too, so the `)` written
    // behind that word is a depth-0 candidate: `case x in a) "(x") ;; esac` is
    // bash rc 2, and a scan that counted the quoted paren as an opener would let
    // its `)` close that group and swallow the pattern paren.
    assert!(opens(b"\"(x\")", None));
    // A **quoted** backslash is word text, not an escape pair, and the tokenizer
    // hands it over as a **pair** of quoted bytes: `'\'(x)` and `"\\"(x)` are both
    // the word `\\(x` (mask: quoted, quoted, unquoted, unquoted) plus its `)`
    // token. Skipping the two quoted bytes one at a time lands on the same byte as
    // one jump of two, so the `(` behind the pair opens its group either way —
    // bash rejects the shape (rc 2) and fdshell parses it as the subshell-shaped
    // arm of #316/#419. The pin is the counted group, and it is what the
    // equivalence of the escape arm's mask test rests on.
    assert!(!opens(b"'\\'(x)", None));
    assert!(!opens(b"\"\\\\\"(x)", None));
}

/// An **escape pair** is one literal word byte, and the token's mask marks both
/// of its bytes `Unquoted` — the backslash is the only marker. So the escaped `(`
/// of `x\(` opens no group the arm owns, and the `)` that follows it is the
/// clause's pattern paren: `case x in a) x\() ;; esac` is bash rc 2, and a rule
/// reading the word's last byte alone reads `x\(` as an open `(` and swallows it.
#[test]
fn an_escaped_paren_opens_no_group() {
    assert!(opens(b"x\\()", None));
    // Behind an argument the paren is not a pattern paren for the other gate, and
    // nothing may claim it opened a group either (`a) echo x\() ;; esac` is the
    // accepted #419 shape, bash rc 2).
    assert!(!opens(b"echo x\\()", None));
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
