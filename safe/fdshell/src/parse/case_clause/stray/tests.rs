#![allow(clippy::unwrap_used)]

use super::{Stray, opens_groups, verdict};
use crate::mask::MaskBit;
use crate::parse::token::tokenize_statement;
use alloc::vec;

/// [`verdict`] over the tokens of the statement `line`, with `limit` as the
/// index of the `;;` window that ends the arm region (`None`: the run up to
/// `esac`), naming the verdict per row. This is the token list the parse layer
/// scans (`case_clause::extract::body`), heredoc bodies blanked and byte offsets
/// preserved, and `line` is the statement's own text, so the span gates
/// [`super::span_word`] and [`super::separator_data`] read the spans the tokens
/// were built from. The seed is the pattern walk's (task #419); these regions
/// are arms, so it stays 0 and the `((` word sits inside the region.
fn verdict_of(line: &[u8], limit: Option<usize>) -> Option<Stray> {
    verdict(&tokenize_statement(line).unwrap(), limit, line, 0).map(|(stray, _)| stray)
}

/// [`opens_groups`] on a word and a mask written **directly**: the mask the
/// tokenizer produces for the real shapes is pinned through [`verdict_of`]
/// above, and these are the counter's own contract, byte by byte.
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
    assert_eq!(verdict_of(b"b)", None), Some(Stray::ClausePattern));
    // The `;` restarts command position, so the same shape behind a `;` bails
    // (`a) echo hi; b) ;; esac`).
    assert_eq!(verdict_of(b"echo a;b)", None), Some(Stray::ClausePattern));
    // An assignment word is a command word here (`a) x=1) ;; esac`, bash rc 2).
    assert_eq!(verdict_of(b"x=1)", None), Some(Stray::ClausePattern));
}

/// A paren behind an **argument** is a paren the arm's command list cannot take
/// (bash rc 2) — the family task #419 closed: the place gate leaves an
/// argument-position word with no openers, so the paren sits at depth 0 and the
/// argument arm of the classification takes it. Only the arm that opened its own
/// group stays clean, and that one is task #316's shape (bash rc 0).
#[test]
fn a_paren_behind_an_argument_is_a_stray_close_paren() {
    assert_eq!(verdict_of(b"echo hi)", None), Some(Stray::CloseParen));
    // The arm opened this group, so its `)` closes it: bash rc 0.
    assert_eq!(verdict_of(b"(echo sub)", None), None);
    assert_eq!(verdict_of(b"echo a[b)", None), Some(Stray::CloseParen));
    assert_eq!(verdict_of(b"echo (b)", None), Some(Stray::CloseParen));
}

/// Arithmetic is data: the tokenizer emits every unquoted `)` as its own token,
/// so `((1))` is the tokens `((1`, `)`, `)` — the word's bytes arm **two**
/// closers and both are data, the region's own rule owning them — and the `((`
/// region swallows the bare `)` tokens an arith `for` clause writes behind its
/// `;` (`case x in x) for ((i=0;i<2;i++)); do …; done;; esac` is bash rc 0).
#[test]
fn the_arithmetic_region_and_its_operators_are_data() {
    assert_eq!(verdict_of(b"((1))", None), None);
    assert_eq!(verdict_of(b"for ((i=0;i<2;i++))", None), None);
    // The region ends at its own two closers: the clause paren written right
    // behind a complete arithmetic command bails (`a) ((1)); b) ;; esac`,
    // bash rc 2).
    assert_eq!(
        verdict_of(b"((1)); b)", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
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
    assert_eq!(verdict_of(b"f() { :; }", None), None);
    assert_eq!(verdict_of(b"echo hi;f() { :; }; f", None), None);
    // A definer inside the arm's own block bodies (the `;` behind `then`/`do`/
    // `}` is command position, so only this gate keeps these parsing).
    assert_eq!(verdict_of(b"if true; then f() { :; }; fi", None), None);
    assert_eq!(verdict_of(b"while false; do f() { :; }; done", None), None);
    assert_eq!(verdict_of(b"for i in 1; do f() { :; }; done", None), None);
    assert_eq!(verdict_of(b"{ f() { :; }; }", None), None);
    // A subshell group the arm opened: single word, pipeline stage, the `g() ( : )`
    // definer whose body is a subshell, and a subshell behind `&&`.
    assert_eq!(verdict_of(b"(cat)", None), None);
    assert_eq!(verdict_of(b"echo hi|(cat)", None), None);
    assert_eq!(verdict_of(b"g() ( : )", None), None);
    // The same group with its closer behind a **command-position** word (`:` is
    // the subshell's command): the arm's own nesting protects it, so a rule that
    // only reads the neighbouring word is not enough.
    assert_eq!(verdict_of(b"( : )", None), None);
    assert_eq!(verdict_of(b"(true) && echo y", None), None);
    // A definer glued to its subshell body has no space between the `)` and the
    // `(`, so the subshell's `)` follows the bare `(` word.
    assert_eq!(verdict_of(b"g() (:)", None), None);
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
    assert_eq!(verdict_of(b"m=(1)", None), None);
    assert_eq!(verdict_of(b"arr=(\"$@\")", None), None);
    assert_eq!(verdict_of(b"x=(1) y=2", None), None);
    assert_eq!(verdict_of(b"m=(1 2)", None), None);
    assert_eq!(verdict_of(b"m=()", None), None);
    assert_eq!(verdict_of(b"m=($(echo a))", None), None);
    // The same assignment written inside the arm's own block bodies.
    assert_eq!(verdict_of(b"if true; then m=(1); fi", None), None);
    assert_eq!(verdict_of(b"{ m=(1); }", None), None);
    assert_eq!(verdict_of(b"while false; do m=(1); done", None), None);
    // The array's `)` closes the group, so the clause paren written behind it is
    // seen (`case x in a) m=(1); b) ;; esac` is bash rc 2).
    assert_eq!(
        verdict_of(b"m=(1); b)", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
    // `((` stays arithmetic's own, mid-word included: `m=$((1))` is one word whose
    // parens close inside it, so the arm's depth must not rise — a scan that
    // counted them would swallow the clause paren after it (`case x in a)
    // m=$((1)); b) ;; esac` is bash rc 2, and the bare arith assignment is rc 0).
    assert_eq!(verdict_of(b"m=$((1))", None), None);
    assert_eq!(verdict_of(b"m=$((m+1))", None), None);
    assert_eq!(
        verdict_of(b"m=$((1)); b)", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
}

/// The depth returns to 0 when the arm's group closes, so a clause paren behind
/// a plain word still bails after a function definition or a subshell has ended
/// (`case x in a) f() { :; }; b) ;; esac` and `case x in a) (true); b) ;; esac`
/// are bash rc 2, like every other missing `;;`).
#[test]
fn a_clause_paren_behind_a_word_after_a_closed_group_still_bails() {
    assert_eq!(
        verdict_of(b"f() { :; }; b)", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
    assert_eq!(
        verdict_of(b"(true); b)", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
    assert_eq!(
        verdict_of(b"g() ( : ); b)", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
}

/// The keyword gate: a quoted `")"` is a word whose **text** is `)`, and an
/// escaped `\)` keeps its pair in the word text, so both stay arm text
/// (`case x in a) echo ")" ;; esac` and `a) echo \);; esac` are bash rc 0).
#[test]
fn a_quoted_or_escaped_paren_is_word_text() {
    assert_eq!(verdict_of(b"echo \")\"", None), None);
    assert_eq!(verdict_of(b"echo \\)", None), None);
    // A quoted `(` opens no group, so the `)` written behind the word lands at
    // depth 0 behind an argument — task #419 closed this (bash rc 2).
    assert_eq!(
        verdict_of(b"echo \"x(\" b)", None),
        Some(Stray::CloseParen),
        "bash rc 2"
    );
    // A quoted `(` at the arm's own start is word text too, so the `)` written
    // behind that word is a depth-0 candidate: `case x in a) "(x") ;; esac` is
    // bash rc 2, and a scan that counted the quoted paren as an opener would let
    // its `)` close that group and swallow the pattern paren.
    assert_eq!(
        verdict_of(b"\"(x\")", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
    // A **quoted** backslash is word text, not an escape pair, and the tokenizer
    // hands it over as a **pair** of quoted bytes: `'\'(x)` and `"\\"(x)` are both
    // the word `\\(x` (mask: quoted, quoted, unquoted, unquoted) plus its `)`
    // token. Skipping the two quoted bytes one at a time lands on the same byte as
    // one jump of two, so the `(` behind the pair opens its group either way —
    // bash rejects the shape (rc 2) and fdshell parses it as the subshell-shaped
    // arm of #316/#419. The pin is the counted group, and it is what the
    // equivalence of the escape arm's mask test rests on.
    assert_eq!(verdict_of(b"'\\'(x)", None), None);
    assert_eq!(verdict_of(b"\"\\\\\"(x)", None), None);
}

/// An **escape pair** is one literal word byte, and the token's mask marks both
/// of its bytes `Unquoted` — the backslash is the only marker. So the escaped `(`
/// of `x\(` opens no group the arm owns, and the `)` that follows it is the
/// clause's pattern paren: `case x in a) x\() ;; esac` is bash rc 2, and a rule
/// reading the word's last byte alone reads `x\(` as an open `(` and swallows it.
#[test]
fn an_escaped_paren_opens_no_group() {
    assert_eq!(
        verdict_of(b"x\\()", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
    // Behind an argument the paren is not a *pattern* paren — the place gate
    // leaves the escape pair with no opener and the argument arm of the
    // classification takes the paren (`a) echo x\() ;; esac` is bash rc 2, and
    // task #419 closed it).
    assert_eq!(
        verdict_of(b"echo x\\()", None),
        Some(Stray::CloseParen),
        "bash rc 2"
    );
}

/// The bound is the `;;` **window**, so the next clause's own pattern parens are
/// never in scope: the arm `echo hi` of `a) echo hi;; b) ;; esac` is clean, and
/// the same region taken past the separator is not.
#[test]
fn the_limit_is_the_semi_semi_window() {
    // `echo hi;;b)`: the window sits at index 2.
    assert_eq!(verdict_of(b"echo hi;;b)", Some(2)), None);
    assert_eq!(verdict_of(b"echo hi;;b)", Some(4)), None);
    // Past the window (the mutation of the bound) the clause's paren is seen.
    assert_eq!(
        verdict_of(b"echo hi;;b)", Some(6)),
        Some(Stray::ClausePattern)
    );
    assert_eq!(verdict_of(b"echo hi;;b)", None), Some(Stray::ClausePattern));
}

/// A region that *starts* at the paren has no word before it — the `i > 0`
/// guard belongs to task #300's clause-pattern arm only, so the no-predecessor
/// arm of the classification takes it (`case x in a) ) ;; esac` is bash rc 2,
/// task #419 closed it) — and an arm whose paren sits in a word-list place (a
/// nested `case` arm, behind `in`) is untouched: `case a in a) case b in b)
/// echo y;; esac;; esac` is bash rc 0 and task #418's window walk lands it — the
/// nested clause's first pattern `)` is behind `in`, so this rule never sees it
/// as a clause paren (the pair of pins
/// [`the_window_test_reads_the_region_it_now_spans`] and the integration
/// `a_nested_block_in_an_arm_keeps_its_semi_semi`).
#[test]
fn a_paren_with_no_word_before_it_is_a_stray_close_paren() {
    let tokens = tokenize_statement(b"a)").unwrap();
    let region = tokens.get(1..).unwrap();
    assert_eq!(
        verdict(region, None, b"a)", 0),
        Some((Stray::CloseParen, 1))
    );
    assert_eq!(verdict_of(b"case b in b)", None), None);
}

/// The bound is the `;;` **window**, and task #418 made the window the arm's own
/// level — so the region the walk reads now spans a nested block. Up to that
/// window a nested pattern paren is still not a clause paren (the word behind
/// `in` is a word-list word), and the region's last window decides where the walk
/// stops; past it, the nested clause's `)` in command position is seen. The second
/// line is the nested clause list with a **second** clause: its pattern sits in
/// command position (the nested `;;` restarted it), so this rule answers
/// `case: missing ';;' between clauses` where bash rc 0 runs both clauses — the
/// divergence the plan's pin list calls parity, pinned as measured in
/// `tests/case.rs::a_nested_block_in_an_arm_keeps_its_semi_semi`. The limits are
/// the helper's, so they index the **whole** token list of each line: the outer
/// `;;` pair sits at index 15 of the first line and 21 of the second, the nested
/// clause's own window at index 12 of both.
#[test]
fn the_window_test_reads_the_region_it_now_spans() {
    // One nested clause: nothing in the region is a pattern paren.
    let line = b"case a in a) case b in b) echo y;; esac;; esac";
    // The window is the outer `;;` pair: token index 15 (the nested `case`'s own
    // window, at index 12, belongs to the nested clause).
    assert_eq!(verdict_of(line, Some(15)), None);
    assert_eq!(
        verdict_of(line, Some(12)),
        None,
        "up to the nested clause's own window"
    );

    // The two-clause nested list: the walk to the outer window (index 21) crosses
    // the nested second clause's pattern paren, and its word sits in command
    // position — the rule fires.
    let line = b"case a in a) case b in x) echo n;; *) echo inner;; esac;; esac";
    assert_eq!(verdict_of(line, Some(21)), Some(Stray::ClausePattern));
    assert_eq!(
        verdict_of(line, Some(12)),
        None,
        "the nested clause's own window bounds a walk that never reaches the `*)`"
    );
}

/// The operator tokens the scan reads as predecessors: a `)` behind the `;;`
/// pair, behind a pipeline `|`, or behind a `&` is never a *pattern* paren — it
/// is a paren the command list cannot take (task #419 closed the row; bash rc 2
/// on all three, e.g. `case x in a) echo hi;; ) ;; esac`).
#[test]
fn a_paren_behind_an_operator_token_is_a_stray_close_paren() {
    assert_eq!(verdict_of(b";;)", None), Some(Stray::CloseParen));
    assert_eq!(verdict_of(b"|)", None), Some(Stray::CloseParen));
    assert_eq!(verdict_of(b"&)", None), Some(Stray::CloseParen));
}

/// A heredoc **body** is blanked by `tokenize_statement`, so it is never in the
/// arm region: the `)` of a body line, and even a word glued to one, is body
/// text, not a pattern paren (`case x in x) cat <<T` newline `x)` newline `T`
/// newline `esac` prints `x)` verbatim in bash, rc 0).
#[test]
fn a_heredoc_body_written_in_an_arm_is_not_in_the_region() {
    assert_eq!(
        verdict_of(b"case x in x) cat <<T\nx)\n)\nT\nesac", None),
        None
    );
}

/// A **quoted** separator word is word data, so it never restarts command
/// position for the paren written behind it (task #336, [`separator_data`]): the
/// `)` follows an argument and the argument arm of the verdict takes it — the
/// family task #419 closed (bash rc 2). The unquoted twins are the rows of
/// [`a_paren_behind_a_command_position_word_opens_the_next_pattern`].
#[test]
fn a_quoted_semicolon_never_restarts_command_position() {
    // The real `;` restarts command position, so the `)` behind the next word is
    // the clause pattern of a missing `;;`.
    assert_eq!(
        verdict_of(b"echo a;b)", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
    // The quoted and escaped words are the command's **arguments**: the paren
    // follows data, and the argument arm answers with the new verdict.
    assert_eq!(
        verdict_of(b"echo a \";\" b)", None),
        Some(Stray::CloseParen),
        "bash rc 2"
    );
    assert_eq!(
        verdict_of(b"echo a ';' b)", None),
        Some(Stray::CloseParen),
        "bash rc 2"
    );
    assert_eq!(
        verdict_of(b"echo a \\; b)", None),
        Some(Stray::CloseParen),
        "bash rc 2"
    );
    assert_eq!(
        verdict_of(b"echo a \";\" b); c)", None),
        Some(Stray::CloseParen),
        "the quoted word keeps the first `)` as word data (task #336), so the walk's \
         first judged paren is that one: its predecessor sits in argument position \
         and task #419's arm answers — the real `;` and `c` are never reached"
    );
    // The same rule inside the arm's own block bodies, where the `;` of the
    // block's clauses is command position.
    assert_eq!(
        verdict_of(b"if true; then echo a \";\" b) fi", None),
        Some(Stray::CloseParen),
        "bash rc 2"
    );
    assert_eq!(
        verdict_of(b"if true; then echo a; b) fi", None),
        Some(Stray::ClausePattern),
        "bash rc 2"
    );
    // A quoted separator as the arm's **first** word is the command word, not a
    // restart; the paren behind its argument is an argument's paren (task #419
    // closed it, bash rc 2).
    assert_eq!(
        verdict_of(b"\";\" b)", None),
        Some(Stray::CloseParen),
        "bash rc 2"
    );
    assert_eq!(
        verdict_of(b"; b)", None),
        Some(Stray::ClausePattern),
        "the real `;` leaves command position"
    );
}
