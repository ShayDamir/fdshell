//! Unit pins for keyword-block opacity in the cond-list boundary scan: a
//! `&&` / `||` written inside a keyword block's text is never a boundary of
//! the outer cond list, and the shapes that must *not* take the jump
//! (closers, quoted keywords, substitution bodies, keyword-shaped words,
//! keyword-shaped *arguments*, group openers, heredoc-carrying spans) keep
//! splitting exactly as before.

use crate::cond::boundary::block::word_delta;
use crate::cond::boundary::next_boundary;
use crate::keywords::Place;
use crate::scan::ScanState;

/// `next_boundary` from the start of `line`; `line.len()` means "no split —
/// the text is one cond-list part".
fn boundary_of(line: &[u8]) -> (usize, bool) {
    next_boundary(line, 0, &mut ScanState::new())
}

/// `next_boundary` from `i` — the resume point `run_cond_list` uses after it
/// takes the part that ends at the previous boundary.
fn boundary_from(line: &[u8], i: usize) -> (usize, bool) {
    next_boundary(line, i, &mut ScanState::new())
}

/// The shapes of the reported bug: the whole block is one part, so the
/// `&&` / `||` reaches the block's own cond list (bash runs it there).
#[test]
fn operator_in_a_block_condition_is_not_a_boundary() {
    for line in [
        b"while [ 1 ] && [ 1 ]; do a; done".as_slice(),
        b"until [ 1 ] || [ 1 ]; do a; done".as_slice(),
        b"if [ 1 ] && [ 0 ]; then t; else f; fi".as_slice(),
        b"for i in a; do a && b; done".as_slice(),
        b"case x in a) false && echo b;; esac".as_slice(),
        b"while if true; then false; fi && true; do a; done".as_slice(),
        b"time while [ 1 ] && [ 1 ]; do a; done".as_slice(),
    ] {
        assert_eq!(boundary_of(line), (line.len(), false), "{:?}", line);
    }
}

/// The scan resumes just past the block's closing keyword, so a *trailing*
/// operator is still found, and a group wrapped around a block keeps its
/// split after the group's own `}` (the group rule and the jump coexist).
#[test]
fn scan_resumes_past_the_closing_keyword() {
    // `… done && echo b` — the jump lands at the space after `done` (22).
    assert_eq!(
        boundary_of(b"while true; do a; done && echo b"),
        (23, false)
    );
    // `… } && echo d` — the split is the group's operator, at 35.
    assert_eq!(
        boundary_of(b"{ while c && c; do b && c; done; } && echo d"),
        (35, false)
    );
}

/// `wait` opens an event-case block only with a pattern keyword behind it;
/// the POSIX builtin keeps its `&&` as a cond-list operator.
#[test]
fn wait_block_is_opaque_and_posix_wait_splits() {
    let block = b"wait\nreadable %rd) false && echo b;;\ndone";
    assert_eq!(boundary_of(block.as_slice()), (block.len(), false));
    // `wait $! && echo x` — the builtin: split at the `&&` (8).
    assert_eq!(boundary_of(b"wait $! && echo x"), (8, false));
    // A same-line `;` is the builtin too: the `&&` splits (after `done`, at 30).
    assert_eq!(
        boundary_of(b"wait; readable) echo r;; done && echo x"),
        (30, false)
    );
}

/// An unclosed opener yields the whole rest of the text as one part: the
/// parse layer reports the missing closer, as it does today.
#[test]
fn unclosed_opener_is_one_part() {
    let line = b"while [ 1 ] && [ 1 ]";
    assert_eq!(boundary_of(line.as_slice()), (line.len(), false));
    let line = b"if [ 1 ] && [ 0 ]; then t";
    assert_eq!(boundary_of(line.as_slice()), (line.len(), false));
}

/// A span that carries a `<<` operator keeps the block rule (task #234): its
/// command text is opaque, and only the `&&` written inside a **body** region
/// splits the part (see `tests/heredoc.rs`).
#[test]
fn heredoc_carrying_span_scans_only_its_body_regions() {
    // `while cat <<X; do a && b; X` + newline + `done`: the `&&` is in the
    // block's command text, the body region carries none, so the whole text
    // is one part — the block's own cond list runs it (bash's rule).
    let line = b"while cat <<X; do a && b; X\ndone";
    assert_eq!(boundary_of(line), (line.len(), false), "{:?}", line);
    // `if cat <<X; then` + body with `&&` + delimiter + `fi` — the body `&&`
    // (22) stays a split point: the pinned rc 1 `missing 'fi'` shape.
    assert_eq!(
        boundary_of(b"if cat <<X; then\nbody && x\nX\nfi"),
        (22, false)
    );
    // An operator in a multi-line block's *body* line that carries a heredoc:
    // the text is the block's own (extension mode, bodies inline), and the
    // region rule applies — the `&&` written on the **delimiter line** (26)
    // is inside the body region, so the region is skipped and the whole text
    // stays one part.
    let line = b"while :; do\ncat <<Q\nbody\nQ && echo x\nbreak\ndone";
    assert_eq!(boundary_of(line), (line.len(), false), "{:?}", line);
    // A `case` arm's operator in a heredoc-carrying block (append mode: the
    // bodies travel outside the text): the command text is opaque, one part.
    let line = b"case q in q) cat <<Q; echo a && echo b;; esac\nbody\nQ";
    assert_eq!(boundary_of(line), (line.len(), false), "{:?}", line);
}

/// Kill-shapes for the gate: only an *opening* keyword word at a word start
/// takes the jump. Closers, quoted and substituted keywords, keyword-shaped
/// words and group openers must keep splitting.
#[test]
fn gate_fires_only_for_an_opening_keyword_word() {
    // Closers score -1.
    assert_eq!(boundary_of(b"done && echo x"), (5, false));
    assert_eq!(boundary_of(b"fi && echo x"), (3, false));
    assert_eq!(boundary_of(b"esac && echo x"), (5, false));
    assert_eq!(boundary_of(b"} && echo b"), (2, false));
    // A quoted keyword word, and a keyword inside a `$( … )` body.
    assert_eq!(boundary_of(b"\"while\" && echo x"), (8, false));
    assert_eq!(
        boundary_of(b"echo \"$(while true; do a; done)\" && echo b"),
        (33, false)
    );
    assert_eq!(
        boundary_of(b"$( while true; do a; done ) && echo b"),
        (28, false)
    );
    // Keyword prefixes are words, not keywords.
    assert_eq!(boundary_of(b"whileish && echo x"), (9, false));
    assert_eq!(boundary_of(b"iffy && echo x"), (5, false));
    // A group opener is a cond-list part: its own rule splits after the brace.
    assert_eq!(boundary_of(b"echo { && echo b"), (7, false));
    assert_eq!(boundary_of(b"{ echo a; } && { echo b; }"), (12, false));
}

/// Task #137 regression: a keyword-shaped word in an **argument** position is
/// word data, exactly as bash, the segment scanner and the group arm above all
/// require. Without the command-position gate the word scored `+1`, the depth
/// left 0 and every later `&&` / `||` of the part disappeared — including the
/// data-corrupting `echo x > if && echo b`.
#[test]
fn keyword_shaped_argument_words_keep_the_split() {
    for (line, index, is_or) in [
        (b"echo if && echo b".as_slice(), 8, false),
        (b"echo while && echo b".as_slice(), 11, false),
        (b"echo for && echo b".as_slice(), 9, false),
        (b"echo case && echo b".as_slice(), 10, false),
        (b"echo until && echo b".as_slice(), 11, false),
        (b"echo wait && echo b".as_slice(), 10, false),
        (b"echo if || echo b".as_slice(), 8, true),
        // A pipeline stage's arguments, and a redirect target, are arguments.
        (b"echo x | echo if && echo b".as_slice(), 17, false),
        (b"echo x > if && echo b".as_slice(), 12, false),
        (b"> if && echo b".as_slice(), 5, false),
        // A keyword-shaped *closer* in an argument position stays data too.
        (b"echo done && echo b".as_slice(), 10, false),
        (b"echo fi && echo b".as_slice(), 8, false),
    ] {
        assert_eq!(
            boundary_of(line),
            (index, is_or),
            "expected the split at {index} in {line:?}"
        );
    }
    // A cond part of the run is command position, and the argument rule applies
    // to the word behind its command word: `run_cond_list` resumes just past
    // the outer `&&` (5), and there the second `&&` (16) is the split.
    assert_eq!(boundary_from(b"true && echo if && echo b", 7), (16, false));
}

/// The gate must not swallow a real block opener: a keyword in command
/// position keeps opening the block at the start of a run, behind `&&` / `||`
/// (with a `time` prefix), and inside a block body — where a nested block's own
/// closer is what brings the depth back down, so the operators written between
/// the pair stay inside the block.
#[test]
fn command_position_keywords_still_open_the_block() {
    // A block statement after `;`: its body's `&&` is the body's own.
    let line = b"echo a; if true; then false && echo x; fi";
    assert_eq!(boundary_of(&line[..]), (line.len(), false));
    // `true && if …` — the split is the outer `&&` (5): the segment scanner
    // splits the run at the block's own `;` (task #232's gap).
    assert_eq!(boundary_of(b"true && if true; then echo t; fi"), (5, false));
    // `time` is transparent in the run's rule too.
    assert_eq!(
        boundary_of(b"true && time while [ 1 ] && [ 1 ]; do a; done"),
        (5, false)
    );
    // A nested block in a body: its `fi` is scored, so the `&&` written after it
    // belongs to the outer body — the whole text stays one part.
    let line = b"while true; do if true; then false; fi && echo x; break; done; echo tail";
    assert_eq!(boundary_of(&line[..]), (line.len(), false));
}

/// Task #236: the in-block clause rule, word by word. `word_delta` scores a
/// block keyword in command position only (at top level the run's own
/// [`group_at_run`] answer, inside a block the scan's [`Place`] flag), so an
/// opener written as an argument (`echo if &&`) never opens a nested depth,
/// a clause keyword (`then`/`do`/`else`/`elif`) puts the word behind it back
/// in command position, a closer behind a command word (`echo done`) is
/// data, a glued `case` clause `)` starts the arm body in command position,
/// and a `case` list scores a closer (`esac`) but no opener (`if)` is a
/// pattern word, killing "closers and openers share one gate"). Braces are
/// scored by neither gate — the group arm counts them with the same clause
/// rule — so an argument brace (`echo {`, `echo }`) stays word data and
/// `echo { && echo b` keeps splitting (task #236).
#[test]
fn in_block_words_follow_the_clause_rule() {
    // `do echo if &&`: the `if` behind `echo` is an argument → the body's
    // `&&` stays a boundary. The transition is applied even to the unscored
    // word: an `if`-shaped argument puts the *next* word in command position
    // (the machine's shape; a condition-taking word reads its condition
    // there).
    let l = b"do echo if &&";
    assert_eq!(word_delta(l, 0, 1, 0, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 3, 1, 0, Place::Command), (0, Place::Data));
    assert_eq!(word_delta(l, 8, 1, 0, Place::Data), (0, Place::Command));
    // `do if :`: a nested block in a clause position scores.
    let l = b"do if :";
    assert_eq!(word_delta(l, 3, 1, 0, Place::Command), (1, Place::Command));
    // `then echo while`: the `while` behind `echo` is data (the transition to
    // command position is the same as for the unscored `if` above).
    let l = b"then echo while";
    assert_eq!(word_delta(l, 0, 1, 0, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 5, 1, 0, Place::Command), (0, Place::Data));
    assert_eq!(word_delta(l, 10, 1, 0, Place::Data), (0, Place::Command));
    // `echo done &&`: the argument-`done` is data, never a closer.
    let l = b"echo done &&";
    assert_eq!(word_delta(l, 0, 1, 0, Place::Command), (0, Place::Data));
    assert_eq!(word_delta(l, 5, 1, 0, Place::Data), (0, Place::Data));
    // `in esac`: a closer scores in the `case` list (`case x in esac`).
    let l = b"in esac";
    assert_eq!(word_delta(l, 0, 1, 0, Place::Command), (0, Place::Pattern));
    assert_eq!(word_delta(l, 3, 1, 0, Place::Pattern), (-1, Place::Data));
    // `in if)`: the glued form is a pattern word, not an opener.
    let l = b"in if)";
    assert_eq!(word_delta(l, 3, 1, 0, Place::Pattern), (0, Place::Command));
    // `x) echo`: the glued clause `)` starts the arm body in command position.
    let l = b"x) echo";
    assert_eq!(word_delta(l, 0, 1, 0, Place::Data), (0, Place::Command));
    // `in time if`: the `time` prefix is transparent (keeps the list's place),
    // and the `if` behind it is a pattern-list word, so it scores 0.
    let l = b"in time if";
    assert_eq!(word_delta(l, 3, 1, 0, Place::Pattern), (0, Place::Pattern));
    assert_eq!(word_delta(l, 8, 1, 0, Place::Pattern), (0, Place::Command));
    // Braces score 0 at every place (the group arm counts them); the
    // transition keeps the master behavior: an argument brace keeps data, a
    // command-position opener opens a command, and the keyword behind it
    // scores in-block nesting as before.
    let l = b"echo }";
    assert_eq!(word_delta(l, 5, 1, 0, Place::Data), (0, Place::Data));
    let l = b"echo { if";
    assert_eq!(word_delta(l, 5, 1, 0, Place::Data), (0, Place::Command));
    assert_eq!(word_delta(l, 7, 1, 0, Place::Command), (1, Place::Command));
    let l = b"{ if";
    assert_eq!(word_delta(l, 0, 1, 0, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 2, 1, 0, Place::Command), (1, Place::Command));
}

/// Task #236's S2: a glued cond pair is *two* word starts for the byte scan
/// (`is_word_break` cuts words at operator bytes), so `word_delta` meets it
/// three times and the pair's payload must be scored exactly once: the pair's
/// head word (`||if`) and tail word (`|if`) carry no keyword of their own and
/// only set the command position the payload word scores in. A background `&`
/// and a pipeline `|` are single-byte runs (no pair): their tails are
/// arguments, #292's carve-out, and the keyword behind them is unscored.
#[test]
fn glued_pair_words_score_their_payload_once() {
    // `a||if :` in a body: head at 1, tail byte at 2, payload word at 3.
    let l = b"a||if :".as_slice();
    assert_eq!(word_delta(l, 1, 1, 0, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 2, 1, 0, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 3, 1, 0, Place::Command), (1, Place::Command));
    // The `&&` spelling, and the tail byte `&if`.
    let l = b"a&&if :".as_slice();
    assert_eq!(word_delta(l, 1, 1, 0, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 2, 1, 0, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 3, 1, 0, Place::Command), (1, Place::Command));
    // Behind data the same three words carry no keyword: the in-block glued
    // divergence (`echo a&&if`), measured in `tests/glued_pair.rs`. The
    // machine's transition is unchanged — an `if`-shaped argument puts the
    // *next* word in command position (see [`in_block_words_follow_the_clause_rule`]).
    let l = b"a||if :".as_slice();
    for i in [1, 2] {
        assert_eq!(
            word_delta(l, i, 1, 0, Place::Data),
            (0, Place::Data),
            "the pair word at {i} is data behind data"
        );
    }
    assert_eq!(word_delta(l, 3, 1, 0, Place::Data), (0, Place::Command));
    // A background `&` is a single-byte run: its tail word `&if` is an
    // argument, and the `if` behind it is scored in the place that leaves.
    let l = b"&if :".as_slice();
    assert_eq!(word_delta(l, 0, 1, 0, Place::Command), (0, Place::Data));
    assert_eq!(word_delta(l, 1, 1, 0, Place::Data), (0, Place::Command));
    // A pipeline `|` is a single-byte run too (#292): the byte is an
    // argument and so is the keyword behind it.
    let l = b"a | if :".as_slice();
    assert_eq!(word_delta(l, 2, 1, 0, Place::Command), (0, Place::Data));
    assert_eq!(word_delta(l, 4, 1, 0, Place::Data), (0, Place::Command));
    // The same pair at top level is judged by the run's own rule, not the
    // flag: an empty run at `run_start` is command position, so the payload
    // `if` opens its block exactly as in-block.
    let l = b"a||if :; then :; fi".as_slice();
    assert_eq!(word_delta(l, 1, 0, 1, Place::Command), (0, Place::Command));
    assert_eq!(word_delta(l, 3, 0, 1, Place::Command), (1, Place::Command));
}
