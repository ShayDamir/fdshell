#![allow(clippy::unwrap_used)]

//! The stray-`;;` rule of task #423: a `;;` outside a `case` clause is a parse
//! error — POSIX #2.10 puts it inside a clause and nowhere else, and bash stops
//! there (fdshell's house rc 1). One predicate, two readers: the byte reader
//! answers on the physical line for everything the statement split shows the
//! byte walk (preflight — nothing runs), and the token reader answers inside
//! block bodies, where the pair survives tokenization.
//!
//! Every bash value below is `bash -c` (5.3.9), measured 2026-11-05; fdshell's
//! column is this build. Rows whose message stays another rule's are pinned
//! here too, each with the rule that owns it.

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

const STRAY: &str = "stray ';;' — it only separates 'case' clauses";

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// The byte reader's half: the statement list shows the pair to the byte walk,
/// so the report is a preflight and nothing on the line runs (the
/// `Segment::AsyncInvalid` family's shape; bash stops at rc 2 everywhere).
#[test]
fn the_byte_reader_reports_the_statement_lists() {
    for script in [
        "echo a ;; echo b",
        "echo a ; ; echo b",
        "echo a;;echo b",
        "echo a ;;",
        "; ; echo b",
        "echo a; ;; echo b",
        // The spaced pair behind a cond operator is still this rule's pair;
        // glued-behind-operator (`||;;`) is the operator rule's own, pinned by
        // `tests/cond_blocks.rs` (its message stays `expected command after`).
        "echo a || ;; echo b",
        "echo a && ;; echo b",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: bash rc 2: {err}");
        assert!(err.contains(STRAY), "{script}: {err}");
    }
}

/// The token reader's half: a body is a sub-slice the scanner never re-splits,
/// so the pair re-appears in its token list and the guard in
/// `semi::block_body`/`block_condition` answers. A pair inside the slice's
/// leading separator run (what `trim_body` absorbs) stays the accepted
/// spelling; the `case`-arm flips of this half are pinned in
/// `tests/case.rs::a_semi_inside_a_nested_arm_block_stays_as_measured`.
#[test]
fn the_token_reader_reports_the_bodies() {
    for script in [
        "{ : ;; }",
        "f() { : ;; }; f",
        "for i in a; do : ;; done",
        "while true; do echo a ; ; echo b; done",
        "if true; then\n echo a ;;\nfi",
        // The task text pinned this row as lazy (bash rc 2, body unrun, rc 0);
        // measured, the guard answers it — the body's pair is intact tokens, and
        // the lazy pins of `tests/block_closers.rs` keep their own (pair-less)
        // rows. Bash agrees on the verdict here, not the rc.
        "while false; do echo a ;; echo b; done",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: bash rc 2: {err}");
        assert!(err.contains(STRAY), "{script}: {err}");
    }
    // The leading-run spellings keep their own messages: the empty-body rule
    // owns `{ ;; }` and `f() { ;; }`, and `{ ;; :; }`-shapes run.
    let (out, err, code) = run("bar() { ;; :; }; bar");
    assert_eq!((out.as_str(), code), ("", 0), "bash rc 2: {err}");
    assert!(run("{ ;; }").1.contains("group is empty"));
    assert!(
        run("f() { ;; }")
            .1
            .contains("function definition body is empty")
    );
}

/// The families the predicate must not cross, each row owned by the rule named
/// on it; the values are the measurements, not bash's (marked where they
/// differ).
#[test]
fn the_measured_families_stay_measured() {
    // Task #247/#336: quoted and escaped pairs are word data (bash parity).
    assert_eq!(run("echo a \";\" \";\" b").0, "a ; ; b\n");
    assert_eq!(run("echo a \\;\\; echo b").0, "a ;; echo b\n");
    // Regions are opaque to the walk (parity): a heredoc body line and the
    // block-body variant.
    assert_eq!(run("cat <<T\nx;;y\nT\n").0, "x;;y\n");
    assert_eq!(run("f() { cat <<T;\nx;;y\nT\n}; f").0, "x;;y\n");
    // The `$( )` paren gate and the arithmetic `for` (parity).
    assert_eq!(run("echo $(a ;; b)").0, "\n");
    assert_eq!(run("for ((i=0;;i++)); do break; done").0, "");
    // POSIX #2.10: inside a clause the pair is the terminator (parity), all
    // three kinds, and the empty-arm spellings.
    assert_eq!(run("case a in a) echo m;; esac").0, "m\n");
    assert_eq!(run("case a in a) echo m;& esac").0, "m\n");
    assert_eq!(run("case a in a) echo m;;& esac").0, "m\n");
    assert_eq!(run("case a in b) ;; a) ;; esac").0, "");
    // Outside a clause `;&`/`;;&` are the statement split's own characters
    // (measured; the split rule owns the message).
    for script in ["echo a ;& echo b", "echo a ;;& echo b"] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: {err}");
        assert!(err.contains("unexpected character"), "{script}: {err}");
    }
    // Task #487's family, decided (b): a statement that **opens** on a `;` at a
    // scan unit's head reports (bash rejects every such row at rc 2, and every
    // other family member already reported rc 1 — one convention). A single
    // separator mid-line and the `;\n;` pair across a newline stay this rule's
    // readers' pins; the multi-line spellings arrive at the top level as one
    // ScriptText, so their `;` is mid-scan and the head-gate holds: measured
    // divergences, the `&` owners' deferral precedent.
    assert_eq!(run("echo a ; echo b").0, "a\nb\n");
    assert_eq!(run("echo a; echo b; echo c").0, "a\nb\nc\n");
    let (out, err, code) = run("; echo b");
    assert_eq!(
        (out.as_str(), code),
        ("", 1),
        "bash rc 2, names the `;`: {err}"
    );
    assert!(err.contains("expected command before ';'"), "{err}");
    assert_eq!(run("echo a ;\n; echo b").0, "a\nb\n");
    // The lone `;` (bash rc 2, nothing runs).
    let (out, err, code) = run(";");
    assert_eq!((out.as_str(), code), ("", 1), "bash rc 2: {err}");
    assert!(err.contains("expected command before ';'"), "{err}");
    // `x\n; y`: bash runs `x` (its 127) and stops at rc 2; fdshell's
    // multi-line row runs both (each its pinned not-found shape).
    let (out, err, code) = run("x\n; y");
    assert_eq!((out.as_str(), code), ("", 1), "{err}");
    assert!(
        err.contains("\"x\" not found") && err.contains("\"y\" not found"),
        "{err}"
    );
    // `# c\n; x`: bash rc 2, nothing runs; fdshell runs the post-comment `x`.
    let (out, err, code) = run("# c\n; x");
    assert_eq!((out.as_str(), code), ("", 1), "{err}");
    assert!(err.contains("\"x\" not found"), "{err}");
    // The order decision, pinned: bash names the leftmost token, so the
    // line-leading `;` reports even though the `;&` guard marks the line too.
    let (out, err, code) = run("; echo a ;& echo b");
    assert_eq!(
        (out.as_str(), code),
        ("", 1),
        "bash rc 2, names the `;`: {err}"
    );
    assert!(err.contains("expected command before ';'"), "{err}");
    // `; >f`: bash rc 2; fdshell reports and the file is **not** created —
    // the preflight answers before any command starts.
    let f = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("semi_semi_f487");
    let (out, err, code) = run(&format!("; >{}", f.display()));
    assert_eq!((out.as_str(), code), ("", 1), "bash rc 2: {err}");
    assert!(err.contains("expected command before ';'"), "{err}");
    assert!(!f.exists(), "the report must precede the redirect");
    // Task #300's trim leniency pinned from the gate's other side: a `;`
    // opening a block body is the parse layer's trim's business — bash
    // rejects this line (rc 2), fdshell stays rc 0 ("stay as they are").
    let (out, err, code) = run("if :; then\n; echo hi\nfi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "{err}");
    // Task #301's closer-claim: the token before the closer is POSIX's `;` and
    // drops out of the body, so the if-row's pair never reaches the guard
    // (bash rc 2; this row keeps its measured acceptance, the plan's risk-2
    // fork, as do the `||;;` rows of `tests/cond_blocks.rs`).
    assert_eq!(run("if true; then echo a ;; fi").0, "a\n");
    assert_eq!(run("if true; then echo a ; ; fi").0, "a\n");
    assert!(
        run("if true; then echo a ||;; fi")
            .1
            .contains("expected command after"),
        "the cond operator rule owns this row"
    );
    // Task #313/#354: the peek guard owns the `&`-behind rows and its message
    // stands (the preflight order is pinned by `tests/async.rs`).
    for script in ["echo a &;; echo b", "echo a & ;; echo b"] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: bash rc 2: {err}");
        assert!(err.contains("unexpected character"), "{script}: {err}");
    }
}
