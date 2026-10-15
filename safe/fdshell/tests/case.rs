#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

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

// --- the `case` word's own quote mask (task #269) ---------------------------
//
// The word of `case WORD in …` is substituted with the mask of its own (joined)
// tokens, so a `\<char>` written inside double quotes keeps its backslash and a
// quoted `~` stays literal. bash 5.3.9 measurements; `m` is the clause body's
// output, so "no match" is empty stdout.
//
// fdshell matches patterns **literally** — the pattern side carries no mask and
// is never substituted (task #280 owns it), and fdshell has no optional `(`
// before a pattern. The rows where the pattern side decides the outcome are
// labelled as that divergence.

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
    // an unmasked pattern globs them (the accepted divergence of task #246).
    let (out, _err, code) = run("case \"a*b\" in \"a*b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
    let (out, _err, code) = run("case \"a*b\" in a*b) echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0));
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
    // An unquoted `~` still expands to `$HOME`, so a literal `$HOME` pattern
    // cannot match it (fdshell never substitutes patterns: task #280).
    let (out, _err, code) = run("case ~ in \"$HOME\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash matches (m)");
}

/// Accepted divergences of the **pattern** side, inherited by task #280
/// (measured against bash 5.3.9): patterns carry no quote mask and are never
/// substituted, so an escape pair written in a pattern escapes its glob
/// characters while the word keeps its bytes, and `$name` in a pattern is
/// literal text.
#[test]
fn case_pattern_side_divergences_are_pinned() {
    // bash matches (m): its word is `a\ b` and the pattern `a\ b` compares the
    // same text. fdshell's word is `a\ b` and the unmasked pattern folds the
    // pair to a space, so nothing matches.
    let (out, _err, code) = run("case \"a\\ b\" in \"a\\ b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash prints m");
    // Same class: bash matches `a\b` against the pattern `a\b`.
    let (out, _err, code) = run("case \"a\\b\" in \"a\\b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "bash prints m");
    // The word is the text `a$b` and fdshell's pattern is literal too, so they
    // match; bash expands `$b` in the pattern (to empty) and matches nothing.
    let (out, _err, code) = run("case \"a\\$b\" in \"a$b\") echo m;; esac");
    assert_eq!((out.as_str(), code), ("m\n", 0), "bash prints nothing");
}
