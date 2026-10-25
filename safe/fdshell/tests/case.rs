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

/// A `case` pattern is **one word**: bash expands it and keeps only its first
/// expanded word (`execute_case_command`, `es->word->word`), so a quoted `$@`
/// in a pattern is the **first positional** — not one pattern per positional
/// word, which is what task #319's description claimed and bash 5.3.9 measures
/// false (`set -- a b; case b in "$@")` prints nothing).
#[test]
fn dollar_at_in_a_case_pattern_is_one_word() {
    let (out, err, code) = run("set -- a b; case a in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- a b; case b in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
}

/// The first word of the expansion, so the positional list is never a joined
/// pattern either: `set -- "a b" c` makes the pattern `a b`, which matches
/// neither its own join `a b c` nor the bare `a`.
#[test]
fn dollar_at_in_a_case_pattern_is_neither_the_join_nor_a_later_positional() {
    let (out, err, code) = run("set -- \"a b\" c; case \"a b c\" in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- \"a b\" c; case a in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- \"a b\" c; case \"a b\" in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- \"a b\" c; case c in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
}

/// An empty `IFS` emits no word boundary, so nothing is dropped: the quoted
/// `$@` **space-joins** and the join is the pattern (measured `IFS=;` rows).
#[test]
fn an_empty_ifs_space_joins_a_quoted_dollar_at_pattern() {
    let (out, err, code) = run("IFS=; set -- a b; case \"a b\" in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=; set -- a b; case a in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // The glue survives on both sides, because there is no cut at all.
    let (out, err, code) =
        run("IFS=; set -- \"a b\" c; case \"xa b cy\" in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// An **unquoted** `$@` in a pattern space-joins whatever the `IFS` is: with
/// `IFS=,` the pattern of `p$@q` over `a b` is `pa bq`, and the `IFS` byte
/// never appears in it.
#[test]
fn an_unquoted_dollar_at_pattern_space_joins_whatever_the_ifs_is() {
    let (out, err, code) = run("IFS=,; set -- a b; case \"pa bq\" in p$@q) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=,; set -- a b; case \"pa,bq\" in p$@q) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=,; set -- a b; case \"ab\" in p$@) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // Unquoted, the positional's glob chars stay live (the mask rule of #280).
    let (out, err, code) = run("set -- 'a*b'; case aXb in $@) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// `$*` (quoted or not) is one word joined with the **first `IFS` byte**, and
/// an empty `IFS` **concatenates** it — the one row where the pattern surface
/// must not reuse the word path's space-when-empty separator.
#[test]
fn dollar_star_in_a_case_pattern_joins_with_the_first_ifs_byte() {
    let (out, err, code) = run("IFS=,; set -- a b; case \"a,b\" in $*) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=,; set -- a b; case \"a,b\" in \"$*\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=; set -- a b; case ab in $*) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=; set -- a b; case \"a b\" in \"$*\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
}

/// No positionals is the **empty** pattern (`"$@"` is the word list, and an
/// empty list contributes no word), while glue around it survives.
#[test]
fn a_dollar_at_pattern_with_no_positionals_is_the_empty_pattern() {
    let (out, err, code) = run("set --; case \"\" in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("set --; case x in \"$@\") echo m;; *) echo star;; esac");
    assert_eq!((out.as_str(), code), ("star\n", 0), "stderr={err:?}");
    let (out, err, code) = run("set --; case xy in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// Each `|` element expands on its own, and inside one element the quoted `$@`
/// is still only its first positional.
#[test]
fn each_dollar_at_pattern_element_keeps_the_first_positional() {
    let (out, err, code) = run("set -- a b; case a in z|\"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- a b; case b in z|\"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- a b; case a in \"$@\"|z) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// The cut keeps the piece the glue belongs to: with `IFS=,` over `a b c` the
/// pattern `x"$@"y` is `xa` — the positional touching the leading `x` — and
/// `cy` (a later positional with the trailing `y`) never matches.
#[test]
fn a_glued_dollar_at_pattern_keeps_only_the_first_piece() {
    let (out, err, code) = run("IFS=,; set -- \"a b\" c; case \"xa b\" in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=,; set -- \"a b\" c; case cy in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=,; set -- a b c; case \"xa\" in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// An empty positional still carries its boundary, so the kept piece is the
/// text up to it (`set -- a ""` → `xa`, `set -- "" b` → `x`).
#[test]
fn an_empty_positional_cuts_a_glued_dollar_at_pattern() {
    let (out, err, code) = run("set -- a \"\"; case xa in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- \"\" b; case x in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("IFS=,; set -- a \"\" b; case xa in x\"$@\"y) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// The trigger's quote bit is the pattern's quoting signal (task #280) through
/// the `$@` arm: a quoted `$@` keeps an expanded `*` **literal**, an unquoted
/// one lets it glob.
#[test]
fn a_dollar_at_pattern_keeps_its_glob_chars_by_the_trigger_quote_bit() {
    let (out, err, code) = run("set -- 'a*b'; case \"a*b\" in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- 'a*b'; case aXb in \"$@\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("set -- 'a*b'; case aXb in $@) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "stderr={err:?}");
}

/// Accepted divergence, pinned (task #319): `IFS=; set -- a ""` and the pattern
/// `x"$@"y`. bash's pattern text carries its quoted-null marker (0x7f) between
/// the pieces — `xa \177y` — which no word reaches, so bash matches **nothing**
/// here; fdshell space-joins (`xa y`) and matches.
#[test]
fn an_empty_positional_with_an_empty_ifs_diverges_in_a_pattern() {
    let (out, err, code) = run("IFS=; set -- a \"\"; case \"xa y\" in x\"$@\"y) echo m;; esac");
    assert_eq!(
        (out.as_str(), code),
        ("m\n", 0),
        "stderr={err:?}, bash matches nothing"
    );
}

/// Accepted divergence, pinned (task #319, the task #177 D1 join): the braced
/// `${@}` keeps fdshell's first-`IFS`-byte join in a pattern, where bash
/// space-joins. The braced form is out of this task's scope.
#[test]
fn a_braced_dollar_at_pattern_keeps_the_first_ifs_byte_join() {
    let (out, err, code) = run("IFS=,; set -- a b; case \"a b\" in \"${@}\") echo m;; esac");
    assert_eq!(
        (out.as_str(), code),
        ("", 0),
        "stderr={err:?}, bash matches (m)"
    );
    let (out, err, code) = run("IFS=,; set -- a b; case \"a,b\" in \"${@}\") echo m;; esac");
    assert_eq!(
        (out.as_str(), code),
        ("m\n", 0),
        "stderr={err:?}, bash matches nothing"
    );
    // The default `IFS` shows the same D1 shape: bash's `"${@}"` is the **first
    // positional** there too, while fdshell joins with the space.
    let (out, err, code) = run("set -- a b; case a in \"${@}\") echo m;; esac");
    assert_eq!(
        (out.as_str(), code),
        ("", 0),
        "stderr={err:?}, bash matches (m)"
    );
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

// --- a clause closed directly by `esac`, and the missing `;;` (task #300) -----
//
// bash reads an arm as a **command list**, so two things follow, and fdshell
// reads them the same way. An arm may be **empty**, closed straight at `esac`
// (`case x in a) esac`: bash rc 0) — the opposite of the `then`/`do` bodies,
// which bash requires non-empty (task #233, `tests/block_bodies.rs`). And the
// `)` of the next clause's pattern list is a token the arm's command list cannot
// take, so clauses written without their `;;` are a syntax error: bash rc 2,
// fdshell rc 1 with `case: missing ';;' between clauses` (the repo-wide rc
// convention). Every row below is a bash 5.3.9 measurement of 2026-10-22.

#[test]
fn case_clause_closed_directly_by_esac_is_accepted() {
    for script in [
        "case x in a) esac",
        "case a in a) esac",
        "case x in a|b) esac",
        "case x in \"a\") esac",
        // An empty clause in the middle, and the last clause closed at `esac`.
        "case x in a) echo one;; b) esac",
        "case x in esac",
        // The multi-line spelling (a newline is the tokenizer's `;` token).
        "case x in\na)\nesac",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 0), "{script}: {err}");
    }
    // The arm that matches runs its body; the `esac`-closed arm runs nothing.
    let (out, err, code) = run("case x in x) echo one;; b) esac");
    assert_eq!((out.as_str(), code), ("one\n", 0), "stderr={err}");
}

/// The redirection written after an `esac` that closed an empty arm is the
/// **block's** redirect, and it opens: bash creates the file (task #298's rule,
/// unchanged by the clause walk).
#[test]
fn case_arm_closed_by_esac_takes_the_block_redirect() {
    for (script, want) in [
        ("case x in a) esac >{f}", ""),
        ("case x in x) echo one;; b) esac >{f}", "one\n"),
    ] {
        let dir = scratch_dir();
        let f = dir.join("arm.txt").to_str().unwrap().to_string();
        let (out, err, code) = run_in(&dir, &script.replace("{f}", &f));
        // The redirect takes the whole block's stdout, so the matching arm's
        // bytes land in the file, not on stdout.
        assert_eq!((out.as_str(), code), ("", 0), "{script}: {err}");
        assert!(dir.join("arm.txt").exists(), "{script}: bash opens it");
        assert_eq!(
            std::fs::read_to_string(dir.join("arm.txt")).unwrap_or_default(),
            want,
            "{script}: bash writes the block's output into the file"
        );
    }
}

#[test]
fn case_clauses_without_a_semi_separator_are_a_parse_error() {
    for script in [
        "case x in a) b) ;; esac",
        "case x in a) b) esac",
        "case x in a) *) esac",
        "case x in a) x=1) ;; esac",
        "case x in a) echo hi;; b) c) ;; esac",
        // Behind a `;` the word is in command position again.
        "case x in a) echo hi; b) ;; esac",
        // The arithmetic region ends at its own `)`, so the clause paren written
        // right behind a complete `((…))` is seen.
        "case x in a) ((1)); b) ;; esac",
        // The arm's own groups close, so the clause paren that follows a function
        // definition, a subshell, or a complete array literal is in command
        // position again (the depth rule of `parse/case_clause/stray.rs`).
        "case x in a) f() { :; }; b) ;; esac",
        "case x in a) (true); b) ;; esac",
        "case x in a) ( : ); b) ;; esac",
        "case x in a) m=(1); b) ;; esac",
        "case x in a) m=$((1)); b) ;; esac",
        // The escaped `(` of `x\(` opens no group: the mask marks the escape pair
        // unquoted, the backslash is the marker, so the `)` that follows it is the
        // clause's pattern paren.
        "case x in a) x\\() b) ;; esac",
        "case x in a) x\\() ;; esac",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: bash rc 2");
        assert!(err.contains("missing ';;'"), "{script}: {err}");
    }
}

/// A `)` that is **not** in command position is arm text, as it always was: the
/// quoted and escaped parens are word tokens, `((1))`'s second `)` follows an
/// operator, the arith `for` clause's bare `)` tokens sit inside their `((`
/// region, and a subshell-shaped arm is task #316's (its arm never matches, so
/// the pin is the parse, not the output).
///
/// The parens of a group **the arm itself opened** are arm text too: a
/// definer's parameter list (`f() { …; }`, in the arm and inside the arm's
/// `if`/`while`/`for`/`{ …; }` bodies, behind a `;` that restarts command
/// position), a subshell the arm wrote (`(cat)`, `echo hi | (cat)`,
/// `g() ( : )`, `(true) && …`), and an **array-literal assignment**, whose group
/// `(` sits mid-word in the tokenizer's word (`m=(1`, so `a) m=(1) ;;` and the
/// assignments inside the arm's bodies parse; running one is task **#422**).
/// bash parses all of these inside the arm, rc 0, and fdshell must parse them —
/// the arm's own paren nesting decides it, not the neighbouring word (`( : )`
/// closes its subshell behind a command-position word) and not the word's two
/// edges. Only the arm that **matches** runs, so the subshell rows pin the
/// parse: fdshell has no subshell yet (task #316) and the arm whose pattern
/// does not match never executes.
#[test]
fn a_paren_in_a_case_arm_stays_arm_text() {
    for script in [
        "case x in a) echo \")\";; esac",
        "case x in a) echo \\);; esac",
        "case a in a) ((1)) ;; esac",
        "case x in a) (echo sub) ;; esac",
        // A function definition is an arm command: at the arm's start, behind a
        // `;`, and inside the arm's own block bodies and brace group.
        "case x in a) f() { :; } ;; esac",
        "case x in a) echo hi; f() { :; } ;; esac",
        "case x in a) if true; then f() { :; }; fi ;; esac",
        "case x in a) while false; do f() { :; }; done ;; esac",
        "case x in a) for i in 1; do f() { :; }; done ;; esac",
        "case x in a) { f() { :; }; } ;; esac",
        // A subshell group the arm opened, and a definer whose body is one.
        "case x in a) (cat) ;; esac",
        "case x in a) echo hi | (cat) ;; esac",
        "case x in a) g() ( : ) ;; esac",
        "case x in a) ( : ) ;; esac",
        "case x in a) (true) && echo y ;; esac",
        // An **array-literal assignment** opens a group too, and its `(` sits
        // mid-word: the tokenizer's word is `m=(1` and the array's `)` is a token
        // of its own, so only a scan over the word's whole byte range sees the
        // group. The arm's pattern never matches, so these pin the **parse** —
        // fdshell has no array runtime (task #422) and the literal's `)` runs as a
        // command at top level exactly as it does here.
        "case x in a) m=(1) ;; esac",
        "case x in a) x=(1) y=2 ;; esac",
        "case x in a) echo hi; x=(1) ;; esac",
        "case x in a) m=(1 2) ;; esac",
        "case x in a) x=(1 2) ;; esac",
        "case x in a) m=() ;; esac",
        "case x in a) arr=(\"a\") ;; esac",
        "case x in a) arr=(\"$@\") ;; esac",
        "case x in a) m=($(echo a)) ;; esac",
        "case x in a) m=$((1)) ;; esac",
        // The same assignment inside the arm's own block bodies and brace group.
        "case x in a) if true; then m=(1); fi ;; esac",
        "case x in a) while false; do m=(1); done ;; esac",
        "case x in a) for i in 1; do m=(1); done ;; esac",
        "case x in a) { m=(1); } ;; esac",
        // A quoted `(` is word text and an escaped one is a literal byte, so
        // neither opens a group; these stay the accepted #419 shapes (bash rc 2).
        "case x in a) echo \"x(\" b) ;; esac",
        "case x in a) echo x\\() ;; esac",
        // A quoted backslash enters the word as a pair of quoted bytes, so the
        // `(` behind it opens its group: the arm reads as the subshell shape of
        // #316/#419 and parses (bash rejects it, rc 2).
        "case x in a) '\\'(x) ;; esac",
        "case x in a) \"\\\\\"(x) ;; esac",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 0), "{script}: {err}");
    }
    // The arith `for` row with the arm that matches runs its loop (bash prints
    // 0 and 1).
    let (out, err, code) = run("case x in x) for ((i=0;i<2;i++)); do echo $i; done;; esac");
    assert_eq!((out.as_str(), code), ("0\n1\n", 0), "stderr={err}");
    // A definition made in an arm is callable there, byte for byte as bash runs
    // it (bash prints `y`).
    let (out, err, code) = run("case x in x) f() { builtin echo y; }; f ;; esac");
    assert_eq!((out.as_str(), code), ("y\n", 0), "stderr={err}");
    // The arm that **matches** runs past its array literal (bash prints `done`).
    // fdshell has no arrays, so the literal's `)` is reported as a command on
    // stderr — byte for byte what the same assignment does at top level, and
    // task #422's runtime half. The pin here is the parse and the arm running.
    let (out, err, code) = run("case x in x) m=(1); echo done ;; esac");
    assert_eq!((out.as_str(), code), ("done\n", 0), "stderr={err}");
}

/// The `;`/`;;`/`&` family around an arm, pinned **as measured**: the accepted
/// leniencies (a lone `;`, a trailing `&`, the `;&` closer, a closer-shaped word
/// in the arm, the glued `;;b)esac` of task **#421**, a `)` with no word before
/// it — task **#419**'s family), and the rejects that stay rc 1 where bash is
/// rc 2. `case x in a) ; esac` is the accepted leniency this task deliberately
/// does not tighten (the plan's open question 2).
#[test]
fn the_semi_family_around_an_arm_stays_as_measured() {
    for script in [
        "case x in a) echo hi; esac",
        "case x in a) echo hi & esac",
        "case x in a) echo hi;& esac",
        "case x in a) echo esac;; esac",
        "case x in a) ; esac",
        // A `)` with no word before it (the arm is empty, so the paren has no
        // predecessor) is arm text here; bash rejects it, rc 2 (task #419).
        "case x in a) ) ;; esac",
        "case x in a) echo y; ) ;; esac",
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: {err}");
    }
    // The `&` row backgrounds the arm's statement: the pin is the rc only, never
    // the job's stdout (LESSONS).
    for script in [
        // The last clause has no `;;`: the block span never closes (bash rc 2).
        "case x in a) echo one;; *) echo two esac",
        // A closer-shaped pattern with no `;;` at all (bash rc 2).
        "case x in a) esac ;; esac",
        // The closer glued behind a pattern's `)` is never found by the block
        // scan (bash rc 0, task #421 owns the shape).
        "case x in a) echo hi;;b)esac",
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: {err}");
    }
}

/// A heredoc **body** written in an arm is body text, never clause structure:
/// `tokenize_statement` blanks the body region, so a `)` written there (even
/// glued to a word) is not the pattern paren of the missing `;;`. bash prints
/// the body verbatim, rc 0.
#[test]
fn a_heredoc_body_in_an_arm_keeps_its_paren() {
    let (out, err, code) = run("case x in x) cat <<T\nx)\n)\nT\nesac");
    assert_eq!((out.as_str(), code), ("x)\n)\n", 0), "stderr={err}");
}
