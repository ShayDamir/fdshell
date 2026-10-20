#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::process::Command;
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// A fresh empty dir: the lazy-expansion pins run there so a pattern's
/// `$(touch f)` creates a file the test then inspects.
/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it
/// runs on a failing test's unwind, which is exactly the test that used to
/// leak.
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn scratch_dir() -> Scratch {
    let dir = std::env::temp_dir().join(format!(
        "fdshell-case-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

fn run_in(dir: &Path, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

// --- the `case` word's own quote mask (task #269) ---------------------------
//
// The word of `case WORD in …` is substituted with the mask of its own (joined)
// tokens, so a `\<char>` written inside double quotes keeps its backslash and a
// quoted `~` stays literal. bash 5.3.9 measurements; `m` is the clause body's
// output, so "no match" is empty stdout.
//
// The clause's **patterns** carry the same treatment (task #280): each pattern
// word travels with its own per-byte mask and is substituted with it, so `?`
// and `[...]` glob, a quoted pattern byte is literal, and an expansion's glob
// characters are quoted by the quotes around its trigger (`"$p"`/`$p`). The
// pattern rows are pinned in the task #280 section below. fdshell has no
// optional `(` before a pattern.

#[test]
fn case_word_quoted_escape_pair_keeps_the_backslash() {
    // The word is `a\ b`, so the pattern `a b` does not match (bash agrees).
    // With the empty mask fdshell folded the pair and matched.
    let (out, _err, code) = run("case \"a\\ b\" in \"a b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    // Unquoted, the pair folds and the same pattern matches (bash: m).
    let (out, _err, code) = run("case a\\ b in \"a b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
}

#[test]
fn case_word_quoted_dollar_pair_keeps_the_dollar() {
    // The word is the text `a$b` (`$b` is never expanded: the pair protects it).
    let (out, _err, code) = run("case \"a\\$b\" in \"ab\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash: no match, word is a$b");
}

#[test]
fn case_word_quote_removal_and_globbing_of_patterns_are_unchanged() {
    // A quoted word keeps its glob characters as data (they never re-glob), and
    // the pattern side matches them the same way in both spellings: quoted `*`
    // is literal, unquoted `*` globs it (task #280's parity).
    let (out, _err, code) = run("case \"a*b\" in \"a*b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "bash prints m");
    let (out, _err, code) = run("case \"a*b\" in a*b) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "bash prints m");
    // A quoted space is one word, and the pattern carries the same space.
    let (out, _err, code) = run("case \"a b\" in \"a b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // `*` matches a word with a space.
    let (out, _err, code) = run("case \"a b\" in *) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
}

#[test]
fn case_word_expansions_are_unchanged() {
    // `$( … )`, a variable and word-0-style expansions run in the word exactly
    // as before (the mask changes folding only).
    let (out, _err, code) = run("v=\"a b\"; case $v in \"a b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case \"$(echo a\\ b)\" in \"a b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // The existing escape-in-pattern pin of `tests/backslash.rs` stays green.
    let (out, _err, code) = run("case f\\*o in f\\*o) echo y;; esac");
    assert_eq!((out.as_str(), code), ("y\n", 0));
}

#[test]
fn case_word_quoted_tilde_is_literal() {
    // `case "~"` is the literal tilde (bash matches the pattern `~`; fdshell
    // with the empty mask expanded `$HOME` and matched nothing).
    let (out, _err, code) = run("case \"~\" in \"~\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // An unquoted `~` expands to `$HOME` on **both** sides, so the word and the
    // pattern meet (bash prints m; fdshell matched nothing while a pattern was
    // never substituted).
    let (out, _err, code) = run("case ~ in \"$HOME\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // A quoted `~` in the pattern stays literal, so the expanded word misses it.
    let (out, _err, code) = run("case ~/a in \"~\"/a) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash prints nothing");
    // Task #281: a quoted character in a tilde-prefix keeps the **word** literal
    // too, and the unquoted pattern side expands, so the two never meet — both
    // rows are bash parity (`n`), not a divergence.
    let (out, _err, code) = run("case ~\"/*\" in ~/*) echo m;; *) echo n;; esac");
    assert_eq!((out.as_str(), code), ("n\n", 0), "bash prints n");
    let (out, _err, code) = run("case \"$HOME\"/* in ~\"/*\") echo m;; *) echo n;; esac");
    assert_eq!((out.as_str(), code), ("n\n", 0), "bash prints n");
}

/// Task #282: `~` on **both** sides of a `case` follows the `HOME` shell
/// variable, so a `HOME=…` assignment moves the word and the lazily expanded
/// pattern together. bash 5.3.9 prints `m` for the first two rows and nothing
/// for the third (its `~x` is an unknown user, fdshell's is a literal tilde —
/// the same text, so both miss `/tmp/H/x`).
#[test]
fn case_tilde_follows_the_shell_variable_home() {
    for (script, want) in [
        ("HOME=/tmp/H; case ~/x in ~/x) echo m;; esac", "m\n"),
        ("HOME=/tmp/H; case /tmp/H/x in ~/x) echo m;; esac", "m\n"),
        (
            "HOME=/tmp/H; case /tmp/H/x in ~x) echo m;; *) echo n;; esac",
            "n\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
    }
}

/// The pattern-side rows that were fdshell's divergences are bash parity now
/// (task #280): the pattern keeps the escape pair it was written with, and an
/// expansion inside a pattern is expanded — quoted, so its bytes stay literal.
#[test]
fn case_pattern_mask_and_expansion_match_bash() {
    // bash matches (m): its word is `a\ b` and the pattern `a\ b` compares the
    // same text. fdshell's unmasked pattern used to fold the pair to a space.
    let (out, _err, code) = run("case \"a\\ b\" in \"a\\ b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // Same class: bash matches `a\b` against the pattern `a\b`.
    let (out, _err, code) = run("case \"a\\b\" in \"a\\b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // The word is the text `a$b` and bash expands `$b` in the pattern (to
    // empty), so nothing matches; the single-quoted pattern keeps `$b` literal
    // and matches.
    let (out, _err, code) = run("case \"a\\$b\" in \"a$b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case \"a\\$b\" in 'a$b') echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // The variable and command-substitution spellings of the same word.
    let (out, _err, code) = run("v=\"a\\ b\"; case \"$v\" in \"a\\ b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case \"$(echo \"a\\ b\")\" in \"a\\ b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
}

// --- the `case` pattern's own quote mask (task #280) ------------------------
//
// `CaseClause::patterns` is a `Pattern` (text + mask) per `|`-separated
// pattern, matched by the mask-aware word matcher `glob::match_component` with
// `dotglob = true`. Every row below is a bash 5.3.9 measurement.

/// The mask is **per byte**, so quoting one byte quotes only that byte.
#[test]
fn case_pattern_quote_mask_is_per_byte() {
    // A quoted `*`/`?` is literal, in both quote kinds.
    let (out, _err, code) = run("case abc in \"a*\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case abc in 'a*') echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash prints nothing");
    let (out, _err, code) = run("case ab_c in \"a?c\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case ab_c in 'a?c') echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash prints nothing");
    // The byte outside the quotes keeps globbing (the mask boundary sits between
    // the `a` and the `*`).
    let (out, _err, code) = run("case ab in \"a\"*) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case \"a*b\" in \"a\"*b) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // A single-quoted glob character is literal against the unquoted star.
    let (out, _err, code) = run("case 'a*b' in \"a\"*b) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // Fully quoted patterns: the literal word, and a space that stays one byte.
    let (out, _err, code) = run("case abc in \"abc\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case ab in \"a\"\"b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case \"a b\" in \"a\"\"b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
}

/// An unquoted `\X` in a pattern is a literal `X`; a quoted `\X` keeps its
/// backslash (the pairs `tests/backslash.rs` pins, in the mask's light).
#[test]
fn case_pattern_escape_pairs_are_decided_by_the_mask() {
    let (out, _err, code) = run("case f*o in f\\*o) echo y;; esac");
    assert_eq!((out.as_str(), code), ("y\n", 0));
    let (out, _err, code) = run("case foobar in f\\*o) echo y;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case \"f\\*o\" in f\\*o) echo y;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case \"f*o\" in f\\*o) echo y;; esac");
    assert_eq!((out.as_str(), code), ("y\n", 0), "bash prints y");
    let (out, _err, code) = run("case \"a?x\" in a\\?x) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case abc in a\\*c) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case \"a*c\" in a\\*c) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
}

/// `?` and `[...]` are pattern characters now (fdshell matched with a `*`-only
/// matcher), with the bracket family's `!` negation, classes, ranges, and the
/// FNM-style literal fallback of an unclosed `[`.
#[test]
fn case_pattern_globs_like_a_bash_pattern() {
    let (out, _err, code) = run("case aXx in a?x) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case aac in a?c) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case abc in a[b]c) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // Quoted brackets are literal bytes, and so is the word's own `[`.
    let (out, _err, code) = run("case abc in \"a[b]c\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case a in [abc]) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case ab in a[!c]) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case aq in a[[:alpha:]]) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case ab in a[![:alpha:]]) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case a1 in a[![:alpha:]]) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // An unclosed `[` is a literal byte on both sides.
    let (out, _err, code) = run("case ab in a[b) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("case \"a[b\" in a[b) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // `[^…]` negation is the shared matcher's gap (task #320): fdshell reads
    // `^` as a member, so the row stays divergent.
    let (out, _err, code) = run("case ab in a[^c]) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash matches (m)");
}

/// A `case` pattern is a **word** pattern, not a pathname: no `FNM_PERIOD` rule,
/// no `set -f`, no IFS splitting.
#[test]
fn case_pattern_follows_the_word_pattern_rules() {
    // Leading-dot rows: they also kill the matcher's `dotglob` argument
    // (`true`→`false` would refuse both).
    let (out, _err, code) = run("case .abc in ?abc) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case .abc in *) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // `set -f` is a pathname option: bash matches a `case` pattern with noglob on.
    let (out, _err, code) = run("set -f; case abc in a*) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // The empty word matches the empty pattern; matching is case-sensitive.
    let (out, _err, code) = run("case \"\" in \"\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case ABC in abc) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    // bash does not split a pattern word: `y` is not in the two-word value.
    let (out, _err, code) = run("v=\"x y\"; case y in $v) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash prints nothing");
}

/// POSIX 2.9.4.3 expands a pattern word (tilde, parameter and command
/// substitution), and bash does it lazily: pattern by pattern, stopping at the
/// first match — so the `$(touch f)` behind a matching pattern never runs.
#[test]
fn case_pattern_words_are_expanded_lazily() {
    // `$name` in a pattern: the quotes around the trigger decide its output.
    let (out, _err, code) = run("p=\"a?x\"; case aXx in $p) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("p=\"a?x\"; case aXx in \"$p\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    let (out, _err, code) = run("p=\"a?x\"; case \"a?x\" in \"$p\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    // A leading `~` is expanded in a pattern, quoted `~` is literal.
    let (out, _err, code) = run("case \"$HOME\" in ~) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case \"~\" in ~) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0));
    // A named fd is a word surface here too (POSIX_COMPATIBILITY "%"): both
    // sides expand to the same fd number, as they do in bash's literal text.
    let (out, _err, code) = run("case %CWD in %CWD) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));

    // The lazy half: the substitution behind the first match is never run.
    let dir = scratch_dir();
    let (out, err, code) = run_in(&dir, "case a in z) echo x;; a|$(touch f)) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    assert!(
        !dir.join("f").exists(),
        "bash: the first match short-circuits the pattern expansion"
    );

    // And when no pattern matches first, the substitution runs on the way.
    let dir = scratch_dir();
    let (out, err, code) = run_in(&dir, "case q in z|$(touch f)|a) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    assert!(
        dir.join("f").exists(),
        "bash creates f while matching `$(touch f)`"
    );
}

/// `set -u` and `${x:?msg}` inside a pattern bail the clause: fdshell's rc 1 and
/// its own message, where bash prints rc 127 (the shell's family rule).
#[test]
fn case_pattern_substitution_bails() {
    let (out, err, code) = run("set -u; case a in \"$undef\") echo m;; *) echo star;; esac");
    assert_eq!(code, 1, "stderr={err:?}, bash prints rc 127");
    assert_eq!(out, "");
    assert!(err.contains("unbound variable"), "stderr={err:?}");
    assert!(err.contains("undef"), "stderr={err:?}");

    let (out, err, code) = run("case a in \"${u:?boom}\") echo m;; esac");
    assert_eq!(code, 1, "stderr={err:?}, bash prints rc 127");
    assert_eq!(out, "");
    assert!(err.contains("u: boom"), "stderr={err:?}");
}

/// Accepted divergence (task #222, to be closed by task #288): the `case` word
/// is substituted whole and compared as-is, so a **quoted** `$@` keeps task
/// #222's separator-free join and the pattern must be written glued where
/// bash's space join asks for a space.
#[test]
fn quoted_dollar_at_in_a_case_word_is_glued() {
    // bash matches the `a b c` pattern (its word is `a b c`); fdshell's word
    // is `a bc`, so only the glued pattern matches.
    let (out, err, code) = run(
        r#"set -- "a b" c; case "$@" in "a b c") echo spaced;; "a bc") echo glued;; *) echo other;; esac"#,
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "glued\n", "bash prints spaced");
    // The glued literal in front of the expansion follows the same rule.
    let (out, err, code) = run(
        r#"set -- "a b" c; case "p$@"q in "pa b cq") echo spaced;; "pa bcq") echo glued;; *) echo other;; esac"#,
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "glued\n", "bash prints spaced");
}

/// Accepted divergence (task #319): a `case` pattern is **one** word, so a
/// quoted `$@` in the pattern is fdshell's join where bash makes one pattern per
/// positional word.
#[test]
fn dollar_at_in_a_case_pattern_is_one_word() {
    let (out, err, code) = run("set -- a b; case a in \"$@\") echo m;; esac");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "", "bash matches its `a` pattern (m)");
}

/// Accepted divergence, parse layer: fdshell joins a multi-token pattern with a
/// space, where bash rejects the shape with a syntax error (rc 2). fdshell
/// accepts and the joined space is a literal byte (`case_pattern_join_marks_its_inserted_space_unquoted`).
#[test]
fn multi_token_case_pattern_is_accepted() {
    let (out, err, code) = run("case \"a b\" in a b) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("case a b in \"a b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// The plain `&` is a command-list terminator **inside a `case` arm's text**
/// (task #354): every closer form of the arm — `& ;;`, `&;;`, `& ;;esac`,
/// `& esac`, the multi-line arm with the `&` glued to the arm's line, and the
/// arm whose statement is a cond list — parses, and the arm's statement runs as
/// a background job. The pin is the job's file plus `wait`, never stdout order
/// (LESSONS).
#[test]
fn ampersand_in_a_case_arm_is_a_background_job() {
    for shape in [
        "case x in x) builtin echo a >{f} & ;; esac",
        "case x in x) builtin echo a >{f} &;; esac",
        "case x in x) builtin echo a >{f} & ;;esac",
        "case x in x) builtin echo a >{f} & esac",
        "case x in x) builtin echo a >{f} &\n;;\nesac",
        "case x in x) builtin true && builtin echo a >{f} & ;; esac",
    ] {
        let dir = scratch_dir();
        let f = dir.join("arm.txt").to_str().unwrap().to_string();
        let script = shape.replace("{f}", &f) + "\nwait";
        let (out, err, code) = run_in(&dir, &script);
        assert_eq!((out.as_str(), code), ("", 0), "shape {shape:?}: {err}");
        assert_eq!(
            std::fs::read_to_string(&f).unwrap_or_default(),
            "a\n",
            "shape {shape:?}"
        );
    }
}

/// A comment written **behind** an arm closed at the `&` is still a comment:
/// when the block-opener gate opens the run at the `&` it clears
/// `book.state.word_active` (the separator arm's rule, task #354's risk R2), so
/// the `esac` ends the block cleanly and the `#` that follows is never eaten as
/// word data. bash 5.3.9 runs the arm's job and ignores the comment, rc 0; the
/// pin is the job's file plus `wait`, not stdout (LESSONS).
#[test]
fn comment_after_a_case_block_closed_at_the_ampersand_is_a_comment() {
    let dir = scratch_dir();
    let f = dir.join("arm.txt").to_str().unwrap().to_string();
    let script = format!("case x in x) builtin echo a >{f} & ;; esac # tail\nwait");
    let (out, err, code) = run_in(&dir, &script);
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    assert_eq!(
        std::fs::read_to_string(&f).unwrap_or_default(),
        "a\n",
        "the tail comment swallowed the arm's job"
    );
}

/// A `case` block whose arm statement **and** the block itself both end in `&`
/// (the block-level `&` is task **#352**'s family): bash forks the block as a
/// job and the arm's statement as a job inside it, rc 0. Two jobs share stdout,
/// so the pin is the arm's file plus `wait` — never the stdout bytes or their
/// order (LESSONS).
#[test]
fn case_block_and_its_arm_statement_can_both_be_backgrounded() {
    let dir = scratch_dir();
    let f = dir.join("arm.txt").to_str().unwrap().to_string();
    let script = format!("case x in x) builtin echo a >{f} & ;; esac &\nwait");
    let (out, err, code) = run_in(&dir, &script);
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    assert_eq!(
        std::fs::read_to_string(&f).unwrap_or_default(),
        "a\n",
        "#352: the block and its arm are both jobs; bash writes `a`"
    );
}

/// Two `&` in one arm are two jobs; the arm whose closer is the bare `esac`
/// (no `;;`) behaves the same (bash prints both, rc 0).
#[test]
fn two_ampersands_in_one_case_arm_run_both_statements() {
    for shape in [
        "case x in x) builtin echo a >{a} & builtin echo b >{b} & ;; esac",
        "case x in x) builtin echo a >{a} & builtin echo b >{b} & esac",
    ] {
        let dir = scratch_dir();
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        let script = shape
            .replace("{a}", a.to_str().unwrap())
            .replace("{b}", b.to_str().unwrap())
            + "\nwait";
        let (out, err, code) = run_in(&dir, &script);
        assert_eq!((out.as_str(), code), ("", 0), "shape {shape:?}: {err}");
        assert_eq!(
            (
                std::fs::read_to_string(&a).unwrap_or_default(),
                std::fs::read_to_string(&b).unwrap_or_default()
            ),
            (String::from("a\n"), String::from("b\n")),
            "shape {shape:?}"
        );
    }
}

/// The arm's `&` does not loosen the clause rules: a `&` behind a `&` is
/// bash's syntax error and fdshell's invalid split (rc 1), a `;;` behind a `&`
/// **outside** a block is the same (rc 1, not the case arm's leniency), and
/// the `& ;` form inside an arm is rejected (task #355): the pair head glued
/// behind the `&` is the statement terminator, not the `;;` separator, so the
/// arm body carries `& ;` and its own scan runs nothing (bash rc 2, fdshell
/// rc 1). The **glued** `;;` forms behind the `&` (`& ;;`, `&;;`, `& ;;esac`)
/// are the clause separator bash accepts and stay rc 0 (`tests/async.rs`'s
/// `block_shapes`).
#[test]
fn ampersand_in_an_arm_keeps_the_clause_rules() {
    for script in [
        "case x in x) echo a & & echo b ;; esac",
        "case x in x) echo a & ;; echo b",
    ] {
        let (out, _err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            ("", 1),
            "script {script:?}: bash rc 2"
        );
    }
    for script in [
        "case x in x) echo a & ; ;; esac",
        "case x in x) echo a & ; echo b ;; esac",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: bash rc 2");
        assert!(err.contains("unexpected character"), "{script}: {err}");
    }
}
