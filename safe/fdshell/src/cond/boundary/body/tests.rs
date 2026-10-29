//! Unit pins for the heredoc region step: a statement's body regions stay
//! opaque (skipped whole); of a keyword block's regions, those whose `<<` sits
//! in the block's **clause** are scanned for their earliest top-level operator
//! and those claimed by a word of the block's **body** are skipped whole, the
//! regions of a `;`-terminated operator line are found on the following lines,
//! a multi-region run skips every region, a body's unbalanced quotes never fold
//! into the caller's state, the region scan and its window are bounded by the
//! region's own bytes, and the region scan's top-level test is per-state: only a
//! **pair** of `&` / `|` is an operator, and one written inside a quote, a
//! backtick or a `$( … )` is region data.

use crate::cond::boundary::block::{Mode, Modes};
use crate::cond::boundary::body::{Step, step, window};
use crate::cond::boundary::next_boundary;
use crate::keywords::Place;
use crate::scan::ScanState;
use crate::scan::heredoc::line_bodies_for_line;
use alloc::vec::Vec;

/// `next_boundary` from the start of `line`.
fn boundary_of(line: &[u8]) -> (usize, bool) {
    next_boundary(line, 0, &mut ScanState::new())
}

/// `next_boundary` from `i` — the resume point `run_cond_list` uses after it
/// takes the part that ends at the previous boundary.
fn boundary_from(line: &[u8], i: usize) -> (usize, bool) {
    next_boundary(line, i, &mut ScanState::new())
}

/// The regions of the run ending at the newline `nl`, as `step` computes them
/// from the run's own physical-line window.
fn regions_of(line: &[u8], run_start: usize, nl: usize) -> (Vec<(usize, usize)>, usize) {
    let (from, to) = window(line, run_start, nl);
    line_bodies_for_line(line, from, to)
}

/// The region step of the run ending at `line`'s first newline, taken from
/// index 0 with the `in_block` flag and an **empty** mode stack: every site is
/// unrecorded, so the scan answers with the [`Mode::Clause`] fallback.
fn step_of(line: &[u8], in_block: bool) -> Option<Step> {
    step_of_modes(line, in_block, &Modes::new())
}

/// The same step with a mode stack fed by `modes`.
fn step_of_modes(line: &[u8], in_block: bool, modes: &Modes) -> Option<Step> {
    let nl = line.iter().position(|&b| b == b'\n').unwrap_or(0);
    step(line, 0, nl, in_block, modes)
}

/// A mode stack fed with the words a scan scores, in order: each entry is the
/// word's `(byte index, place, block-depth delta)`, the very triple
/// [`Modes::scan`] takes. The block's clause keywords are what move the mode.
fn modes_of(line: &[u8], words: &[(usize, Place, i32)]) -> Modes {
    let mut modes = Modes::new();
    for &(i, place, delta) in words {
        modes.scan(line, i, place, delta);
    }
    modes
}

/// The `if` opener of a text that starts with `if`, at the block's clause.
const IF: (usize, Place, i32) = (0, Place::Command, 1);

/// A `;`-terminated operator line finds its body on the following lines: the
/// region window is anchored at the physical line start, so the regions cover
/// the body + delimiter lines. Their position in the block decides: a
/// **clause**-position site splits, a **body**-position site is opaque.
#[test]
fn semicolon_terminated_operator_line_finds_its_regions() {
    let line = b"if :; then cat <<X ;\nbody && x\nX\nfi";
    // The run starts past the `;` (13) and meets its newline at 20.
    let (regions, resume) = regions_of(&line[..], 13, 20);
    assert_eq!(
        regions.len(),
        1,
        "the `;`-terminated `<<` claims exactly its own region"
    );
    assert_eq!(
        regions.first().copied(),
        Some((21, 33)),
        "body + delimiter lines of the one operator"
    );
    assert_eq!(resume, 32, "the newline terminating the delimiter line");
    // The conservative fallback: the run's own site (15) was never recorded,
    // so the region is scanned and its `&&` (26) splits.
    assert_eq!(
        step(&line[..], 13, 20, true, &Modes::new()),
        Some(Step::Split(26, false)),
        "an unrecorded site keeps the region scanned"
    );
    // The body-position site: `if` (0) opens the clause, `then` (6) opens the
    // body, so the region is skipped whole and the scan resumes at 32.
    let body = modes_of(&line[..], &[IF, (6, Place::Command, 0)]);
    assert_eq!(
        step(&line[..], 13, 20, true, &body),
        Some(Step::Skip(32)),
        "the body-position region is opaque"
    );
    // The clause spelling: the `<<` (7) sits before the block's `then` (12),
    // so the same walk leaves the site in the clause and the body's `&&` (22)
    // is the split.
    let clause_text = b"if cat <<X; then\nbody && x\nX\nfi";
    let clause = modes_of(&clause_text[..], &[IF, (12, Place::Command, 0)]);
    assert_eq!(
        step_of_modes(&clause_text[..], true, &clause),
        Some(Step::Split(22, false)),
        "the clause-position region is scanned"
    );
}

/// Statement context (`block_depth == 0`): every region inside the text is
/// skipped whole, so the region's bytes (body lines *and* the delimiter line)
/// never split, and a multi-region run skips all of them. On the first text the
/// delimiter line never arrives, so the region is the end-of-file claim over the
/// remaining lines (task #364) and their `&&` is region data — bash prints those
/// lines as the body and runs `echo b` off the command line's `&&` (rc 0).
#[test]
fn statement_regions_are_skipped_whole() {
    let line = b"cat <<A && echo b\nq && r\nX";
    assert_eq!(boundary_of(&line[..]), (8, false), "the pre-body operator");
    // Resuming at the region start: the region is skipped whole — its `&&`
    // is region data — and the scan ends as one part.
    assert_eq!(boundary_from(&line[..], 11), (26, false), "one part");
    // Resuming inside the region: the same, opaque to the region's end.
    assert_eq!(boundary_from(&line[..], 12), (26, false), "one part");
    // Two operators on one run: the run's own `&&` (8) is the boundary, and
    // resuming after it skips the region of the *second* operator whole.
    let line = b"cat <<A && echo b\nq\nA\ntail\nB";
    assert_eq!(boundary_of(&line[..]), (8, false));
    assert_eq!(
        boundary_from(&line[..], 10),
        (28, false),
        "the run's second region is opaque"
    );
    // The step in statement context skips every region, landing on the last
    // delimiter line's newline (24).
    assert_eq!(
        step_of(b"cat <<A <<B\nq\nA\nb && c\nB\n", false),
        Some(Step::Skip(24)),
        "both regions skipped"
    );
}

/// Block context (`block_depth > 0`): the clause-position region's earliest
/// top-level `&&` / `||` is the boundary (the pinned clause-`&&` split); a
/// body-position region is skipped, and an unrecorded site falls back to the
/// scanned [`Mode::Clause`].
#[test]
fn block_regions_split_on_their_earliest_operator() {
    // The clause spelling, `||` reported as an OR boundary: the `<<` (7) is in
    // the condition, before the block's `then` (12).
    let line = b"if cat <<Q; then\nz || y\nQ\nfi";
    assert_eq!(boundary_of(&line[..]), (19, true), "the body's `||` (19)");
    // The same text as a **body** region: the `<<` sits behind `then`, the
    // region is opaque and the block closes on its `fi` — one part.
    let body = b"if :; then cat <<Q\nz || y\nQ\nfi";
    assert_eq!(boundary_of(&body[..]), (body.len(), false), "one part");
    // A body with no operator: skipped whole up to its newline (22); the `fi`
    // closes the block and the tail after it splits (26).
    let line = b"if :; then cat <<Q\nz\nQ\nfi && echo tail";
    assert_eq!(
        step_of(&line[..], true),
        Some(Step::Skip(22)),
        "the region carries no operator"
    );
    assert_eq!(boundary_of(&line[..]), (26, false), "the tail `&&` (26)");
    // Earliest-first: the second region's operator is reported only when the
    // first region carries none. No block word was ever scored, so the site is
    // unrecorded and the conservative `Clause` fallback scans it — the
    // tripwire of the one-region-per-operator site/region alignment.
    assert_eq!(
        step_of(b"cat <<A <<B\nq\nA\nb && c\nB\n", true),
        Some(Step::Split(18, false)),
        "the second body's `&&` (18)"
    );
    // The clause/body twins of the same two-region text.
    let clause = b"if cat <<A <<B; then\nq\nA\nb && c\nB\nfi";
    let scanned = modes_of(&clause[..], &[IF, (16, Place::Command, 0)]);
    assert_eq!(
        step_of_modes(&clause[..], true, &scanned),
        Some(Step::Split(27, false)),
        "both clause regions are scanned, the second carries the `&&` (27)"
    );
    let body = b"if :; then cat <<A <<B\nq\nA\nb && c\nB\nfi";
    let opaque = modes_of(&body[..], &[IF, (6, Place::Command, 0)]);
    assert_eq!(
        step_of_modes(&body[..], true, &opaque),
        Some(Step::Skip(35)),
        "both body regions are skipped"
    );
    // A region that ends at the text end is claimed in block context, and an
    // unrecorded site keeps the pre-#304 scan: the body's `&&` (26) splits.
    assert_eq!(
        step_of(b"if :; then cat <<X ;\nbody && x\nX\nfi", true),
        Some(Step::Split(26, false)),
        "the conservative fallback of an unrecorded site"
    );
}

/// A body-position region carrying an operator is the body's own data: the
/// block is never split in half, it closes on its closer, and the operator
/// *behind* the closer is the boundary (bash prints the body line and runs the
/// tail; the rows the README names as the body-position family).
#[test]
fn a_body_position_region_is_the_bodys_own_data() {
    // `if :; then cat <<Q` / `z && y` / `Q` / `fi && echo tail`
    let line = b"if :; then cat <<Q\nz && y\nQ\nfi && echo tail";
    assert_eq!(boundary_of(&line[..]), (31, false), "the tail `&&` (31)");
    // With no tail operator the whole block is one part.
    let line = b"if :; then cat <<Q\nz && y\nQ\nfi";
    assert_eq!(boundary_of(&line[..]), (line.len(), false), "one part");
    // A `case` arm's region (behind `q)`) is a body region too.
    let line = b"case q in q) cat <<Q\nz && y\nQ\n;;\nesac";
    assert_eq!(boundary_of(&line[..]), (line.len(), false), "one part");
    // The `case` *word list* stays in the scanned class: the `<<` (9) of
    // `case cat <<Q in` is a clause site (the arm's `)` is behind it), and the
    // heredoc's delimiter line never arrives, so no region exists at all.
    let list = b"case cat <<Q in\nq) echo hit;;\nesac";
    let fed = modes_of(
        &list[..],
        &[(0, Place::Command, 1), (13, Place::Command, 0)],
    );
    assert_eq!(
        fed.mode_at(9),
        Mode::Clause,
        "the word list's `<<` is a clause site"
    );
    assert_eq!(
        boundary_of(&list[..]),
        (list.len(), false),
        "no region, and the block's text carries no operator"
    );
}

/// The keyword that moves the mode, per block: `then`/`else`/`do` and a `case`
/// arm's `)` open a body, `elif` opens a clause again, the innermost open block
/// wins, and a closer hands the mode back to its outer block.
#[test]
fn the_mode_follows_the_block_keywords() {
    // An `elif` clause region splits (bash: rc 2, fdshell: rc 1 — the accepted
    // clause family).
    let clause = b"if :; then :; elif cat <<Q; then\nz && y\nQ\nfi";
    assert_eq!(
        boundary_of(&clause[..]),
        (35, false),
        "the `elif` condition's region is scanned"
    );
    // The `else` arm's region is a body region: opaque, one part (bash rc 0).
    let body = b"if :; then :; else cat <<Q\nz && y\nQ\nfi";
    assert_eq!(boundary_of(&body[..]), (body.len(), false), "one part");
    // The innermost block's mode wins: the outer `then` opened a body, the
    // inner `then` (17) opens the inner body, and the `fi` (39) hands the mode
    // back to the outer body — the region stays opaque.
    let nested = b"if :; then if :; then cat <<Q\nz && y\nQ\nfi\nfi";
    assert_eq!(boundary_of(&nested[..]), (nested.len(), false), "one part");
    // The unit-level twin of the same nesting.
    let fed = modes_of(
        &nested[..],
        &[
            IF,
            (6, Place::Command, 0),
            (11, IF.1, 1),
            (17, Place::Command, 0),
        ],
    );
    assert_eq!(fed.mode_at(26), Mode::Body, "the inner body at the site");
    assert_eq!(fed.mode_at(2), Mode::Clause, "before any keyword");
    let popped = modes_of(
        &nested[..],
        &[
            IF,
            (6, Place::Command, 0),
            (11, IF.1, 1),
            (17, Place::Command, 0),
            (39, Place::Command, -1),
        ],
    );
    assert_eq!(
        popped.mode_at(41),
        Mode::Body,
        "the closer hands the mode back to the outer body"
    );
}

/// A body legitimately carries unbalanced quotes, backticks and `$( … )`.
/// The region scan uses a fresh state and its bytes are never folded into
/// the caller's: an unbalanced body makes the region scan opaque (no split),
/// and the caller's quote state after the region is untouched.
#[test]
fn region_bytes_never_fold_into_the_callers_state() {
    // Statement context, balanced body: the region (body + delimiter line)
    // is skipped whole; the body's `&&` (10) never splits, and the scan
    // resumes on the delimiter line itself, where the tail's `&&` (17) is.
    let line = b"cat <<Q\nz && y\nQ\n&& echo tail";
    assert_eq!(
        boundary_of(&line[..]),
        (17, false),
        "the operator past the region"
    );
    // Statement context, unbalanced `'` in the body: the region scan is
    // opaque (the tail's `&&` (10) is region data), the region is skipped,
    // and the scan resumes on the delimiter line: the tail's `&&` (17)
    // splits, with the caller's own (clean) state.
    let line = b"cat <<Q\n' && y\nQ\n&& echo tail";
    assert_eq!(boundary_of(&line[..]), (17, false), "the tail's `&&` (17)");
    // Block context, unbalanced `'` in the body: the region scan is opaque,
    // the region is skipped, the `fi` closes the block, and the text after
    // the region carries no operator: one part.
    let line = b"if :; then cat <<Q\n' && y\nQ\nfi";
    assert_eq!(boundary_of(&line[..]), (30, false), "one part");
    // The caller's state survives the region walk unchanged.
    let mut state = ScanState::new();
    next_boundary(b"cat <<Q\n' && y\nQ", 0, &mut state);
    assert!(
        !state.in_quote && !state.in_single,
        "the body folded into the state"
    );
}

/// The region scan covers the region's own bytes and nothing past them: the
/// byte just after a region belongs to the line that follows it, and a **pair**
/// of `&` / `|` is what makes an operator — a lone `&` in a body line is data.
#[test]
fn region_operator_scan_is_bounded_by_the_region() {
    // The region is `(19, 23)`: body `z` + the delimiter line `Q`. The `&&` at
    // 23 is the region's *successor* line, the block's own text, and the block
    // keeps it: one part to the end of the text.
    let line = b"if :; then cat <<Q\nz\nQ\n&& echo tail";
    assert_eq!(
        boundary_of(&line[..]),
        (line.len(), false),
        "the operator past the region stays block text"
    );
    // A single `&` (21) in a body line: the scan must read the *next* byte to
    // call it an operator, so the region is opaque and the `fi` closes the
    // block — one part.
    let line = b"if :; then cat <<Q\nz & y\nQ\nfi";
    assert_eq!(
        boundary_of(&line[..]),
        (line.len(), false),
        "a lone `&` is body data"
    );
}

/// A **pair** of `&` / `|` is the operator: in a scanned region a lone `&`
/// (background, a redirect's `&>`) or a lone `|` (a pipe stage) is body data.
/// The scan reads the successor byte to decide, so a scan that compares a byte
/// with *itself* calls every single `&` / `|` an operator and splits the block.
#[test]
fn a_lone_ampersand_or_pipe_in_a_scanned_region_is_data() {
    // An unrecorded site is scanned (the conservative fallback), so the body's
    // lone `&` (10) is met by the scan: the region is opaque, the scan resumes
    // on the delimiter line's newline (15).
    assert_eq!(
        step_of(b"cat <<A\nq & r\nA\n", true),
        Some(Step::Skip(15)),
        "a lone `&` is not an operator"
    );
    // The lone `|` twin (a pipe stage written in a body line).
    assert_eq!(
        step_of(b"cat <<A\nq | r\nA\n", true),
        Some(Step::Skip(15)),
        "a lone `|` is not an operator"
    );
    // The `&&` pair twin, same text: the pair is the split (18).
    assert_eq!(
        step_of(b"cat <<A\nq && r\nA\n", true),
        Some(Step::Split(10, false)),
        "the pair is the region's operator"
    );
}

/// The region scan's top-level test is per-state: an operator written **inside**
/// a double quote, a single quote, a backtick substitution, or a `$( … )` in a
/// scanned body region is body data, never the split. Each state is one
/// conjunct of the scan's top-level test; a conjunction loosened to a
/// disjunction reports the operator inside that construct.
#[test]
fn an_operator_inside_a_quote_or_substitution_is_data() {
    // Each row: a body line whose operator sits inside one construct. The text
    // ends with the delimiter line, so its own newline — the last byte — is
    // where an opaque region leaves the scan.
    for body in [
        "\"q && r\"",
        "'q && r'",
        "`q && r`",
        "$(q && r)",
        "\"q || r\"",
    ] {
        let text = alloc::format!("cat <<A\n{body}\nA\n");
        assert_eq!(
            step_of(text.as_bytes(), true),
            Some(Step::Skip(text.len() - 1)),
            "the operator inside {body:?} is region data"
        );
    }
}

/// The region window is anchored at the run's **own physical line**, and that
/// anchor is a real rule, not a convenience: widening it to the text start
/// drags an earlier line's unterminated heredoc into the run's region list
/// (the helper's missing-delimiter path empties the list, so the body stops
/// being opaque), and shifting it one byte right stops recognizing a `<<` that
/// starts its line after a word break.
#[test]
fn region_window_is_the_runs_own_physical_line() {
    // The run `cat <<Q` (line 3) has one complete region (`b && c` + `Q`):
    // skipped whole, one part. Anchored at the text start, its window also
    // sees line 1's unterminated `<<P` and the regions collapse to none, so
    // the body's `&&` (21) splits.
    let line = b"cat <<P\np\ncat <<Q\nb && c\nQ";
    assert_eq!(
        boundary_of(&line[..]),
        (line.len(), false),
        "line 3's region is opaque"
    );
    // The run's physical line starts with `;` immediately followed by `<<`:
    // the word break *at index 0* is what makes `<<` an operator, so the
    // region exists and its `&&` (7) is region data.
    let line = b";<<Q\nz && y\nQ";
    assert_eq!(
        boundary_of(&line[..]),
        (line.len(), false),
        "the `<<` of a line-initial run is an operator"
    );
    // The run starts one byte past a leading newline: the anchor is that
    // newline's own index + 1, so the window is the run's line and its region
    // (`b && c` + `Q`) is skipped whole — one part.
    let line = b"\ncat <<Q\nb && c\nQ";
    assert_eq!(
        boundary_from(&line[..], 1),
        (line.len(), false),
        "the anchor is the byte past the leading newline"
    );
}

/// The region skip is the caller's rule, and it is what keeps a block closer
/// written **inside** a body region inert: the walk jumps from the run's
/// newline to the last delimiter line's newline, so the region's `done` never
/// reaches the block counter and the mode stack.
#[test]
fn a_block_closer_inside_a_body_region_never_reaches_the_stack() {
    // The run `cat <<Q` (12) is the whole command line of its own physical
    // line, its region ends at the `Q` line's newline (33), and the block
    // closes on its real `done`: one part, no `&&` inside it splits.
    let line = b"while :; do\ncat <<Q\n# done\nbody\nQ\necho a && echo b\nbreak\ndone";
    assert_eq!(
        boundary_of(&line[..]),
        (line.len(), false),
        "the region's bytes never reach the scan"
    );
    // The same text with the mode stack fed by hand: the loop's `do` (9) opens
    // the body, and the region's site (16) answers with it.
    let fed = modes_of(&line[..], &[(0, Place::Command, 1), (9, Place::Command, 0)]);
    assert_eq!(fed.mode_at(16), Mode::Body, "the body at the region's site");
}

/// The window's `to` bound is **one past** the run's terminating newline, and
/// that byte is load-bearing for a delimiter word with an unterminated quote:
/// `cat <<'a b` + `a b` ends the word's raw span exactly on the newline, so at
/// `to == nl` the quote region stops at the window edge, the delimiter reads as
/// `a b`, the next line matches it, and the run owns a region. One past the
/// newline, the quote swallows the newline byte, the delimiter is `a b\n`, no
/// line ever matches it — and a delimiter line that never matches is the
/// end-of-file shape (task #364): the run owns the region of its remaining
/// lines and the resume is the text end. At CLI level the unmatched quote is a
/// parse error rc 1 (bash rc 2), so this is the helper's own answer only
/// (task #466: the program-level argument binds the *program*, not this helper).
#[test]
fn window_end_is_one_past_the_runs_newline() {
    let text = alloc::string::String::from("cat <<'a b\na b");
    let line = text.as_bytes();
    let nl = line.iter().position(|&b| b == b'\n').unwrap_or(0);
    assert_eq!(nl, 10, "the run's terminating newline");
    assert_eq!(
        window(line, 0, nl),
        (0, 11),
        "the window ends one past the newline, not on it"
    );
    let (regions, resume) = regions_of(line, 0, nl);
    assert_eq!(
        (regions.as_slice(), resume),
        (&[(11usize, 14usize)][..], 14),
        "the unterminated quote swallows the newline, so the delimiter never matches"
    );
    assert_eq!(
        step_of(line, true),
        Some(Step::Skip(14)),
        "the end-of-file region is skipped whole (task #364)"
    );
    assert_eq!(
        step_of(line, false),
        Some(Step::Skip(14)),
        "the same in statement context"
    );
}
