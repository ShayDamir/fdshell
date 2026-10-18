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

#[test]
fn here_string_feeds_stdin() {
    let (out, _err, code) = run("cat <<<hello");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn here_string_quoted_keeps_spaces() {
    let (out, _err, code) = run("cat <<<\"a b\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_expands_variables() {
    let (out, _err, code) = run("X=hello; cat <<<$X");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn here_string_bare_form_takes_next_word() {
    let (out, _err, code) = run("cat <<< word");
    assert_eq!(code, 0);
    assert_eq!(out, "word\n");
}

#[test]
fn here_string_appends_trailing_newline() {
    let (out, _err, code) = run("wc -c <<<x");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
}

#[test]
fn here_string_counts_lines() {
    let (out, _err, code) = run("wc -l <<<x");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

#[test]
fn here_string_leaves_other_args_intact() {
    let (out, _err, code) = run("echo a <<<x b");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_empty_word_is_one_newline() {
    let (out, _err, code) = run("cat <<<\"\"");
    assert_eq!(code, 0);
    assert_eq!(out, "\n");
}

#[test]
fn here_string_empty_quoted_word_keeps_later_args() {
    // `""` is the here-string word, not a no-op: `b` stays an argument
    // (bash: `echo a <<<"" b` → `a b`).
    let (out, _err, code) = run("echo a <<<\"\" b");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_with_builtin_command() {
    // printf does not read stdin, but the redirect must apply without error.
    let (out, _err, code) = run("printf \"%s\" a <<<hi");
    assert_eq!(code, 0);
    assert_eq!(out, "a");
}

#[test]
fn here_string_expansion_failure_is_reported() {
    // Out-of-range positional index makes the word expansion fail.
    let (_out, err, code) = run("cat <<<$99999999999999999999");
    assert_ne!(code, 0);
    assert!(err.contains("here-string"), "stderr={err:?}");
}

// --- the word's own quote mask (task #269) ----------------------------------
//
// The here-string word is substituted with its own mask, so a `\<char>` pair
// written inside double quotes keeps its backslash (POSIX 2.2.2) and a quoted
// `~` stays literal. Every expectation is measured against bash 5.3.9.

#[test]
fn here_string_quoted_escape_pair_keeps_the_backslash() {
    // bash: `a\ b`; fdshell with the empty mask printed `a b`.
    let (out, _err, code) = run("cat <<<\"a\\ b\"");
    assert_eq!((out.as_str(), code), ("a\\ b\n", 0));
    // The bare form takes the next token's mask verbatim.
    let (out, _err, code) = run("cat <<< \"a\\ b\"");
    assert_eq!((out.as_str(), code), ("a\\ b\n", 0));
    // Single quotes are mask-independent: the pair is data.
    let (out, _err, code) = run("cat <<<'a\\ b'");
    assert_eq!((out.as_str(), code), ("a\\ b\n", 0));
}

#[test]
fn here_string_quoted_dollar_and_backslash_pairs_drop_the_backslash() {
    // The three quoted pairs of POSIX 2.2.2, as everywhere else in fdshell.
    let (out, _err, code) = run("cat <<<\"a\\$b\"");
    assert_eq!((out.as_str(), code), ("a$b\n", 0));
    let (out, _err, code) = run("cat <<<\"a\\\\b\"");
    assert_eq!((out.as_str(), code), ("a\\b\n", 0));
    let (out, _err, code) = run("cat <<<\"a\\`b\"");
    assert_eq!((out.as_str(), code), ("a`b\n", 0));
    // An escaped quote is data, the tokenizer resolves it (bash agrees).
    let (out, _err, code) = run("cat <<<\"a\\\"b\"");
    assert_eq!((out.as_str(), code), ("a\"b\n", 0));
}

#[test]
fn here_string_unquoted_escape_pair_folds() {
    // An unquoted escape pair stays in the token text and folds (POSIX 2.2.1),
    // and the escaped space keeps the word whole — the same word bash builds.
    let (out, _err, code) = run("cat <<<a\\ b");
    assert_eq!((out.as_str(), code), ("a b\n", 0));
    // A trailing `\` of the word is kept.
    let (out, _err, code) = run("cat <<<abc\\");
    assert_eq!((out.as_str(), code), ("abc\\\n", 0));
}

#[test]
fn here_string_quoted_word_never_splits() {
    // The mask is the splitting mask too: the quoted space is one word.
    let (out, _err, code) = run("wc -c <<<\"a b\"");
    assert_eq!((out.as_str(), code), ("4\n", 0));
    let (out, _err, code) = run("wc -c <<<\"a\\ b\"");
    assert_eq!((out.as_str(), code), ("5\n", 0));
}

#[test]
fn here_string_tilde_follows_its_mask() {
    // Unquoted `~` expands to `$HOME`; a quoted `~` is a literal byte (bash
    // keeps it, task #269's mask gate in `substitute/tilde.rs`), and so is a
    // `~` whose **tilde-prefix** carries any quoted character — the bytes
    // through the first `/` included, so bash's `cat <<<~"/*"` prints `~/*`
    // and `cat <<<~"/x"` prints `~/x` (task #281).
    let home = std::env::var("HOME").unwrap();
    let (out, _err, code) = run("cat <<<~");
    assert_eq!((out.as_str(), code), (format!("{home}\n").as_str(), 0));
    let (out, _err, code) = run("cat <<<\"~\"");
    assert_eq!((out.as_str(), code), ("~\n", 0));
    let (out, _err, code) = run("cat <<<\"~/x\"");
    assert_eq!((out.as_str(), code), ("~/x\n", 0));
    let (out, _err, code) = run("cat <<<~/x");
    assert_eq!(
        (out.as_str(), code),
        (format!("{home}/x\n").as_str(), 0),
        "bash prints {home}/x"
    );
    let (out, _err, code) = run("cat <<<~\"/*\"");
    assert_eq!(
        (out.as_str(), code),
        ("~/*\n", 0),
        "bash prints ~/x, not HOME"
    );
    let (out, _err, code) = run("cat <<<~\"/x\"");
    assert_eq!(
        (out.as_str(), code),
        ("~/x\n", 0),
        "bash prints ~/x, not HOME"
    );
    // The contrast: the quote sits after the `/`, so the prefix is unquoted.
    let (out, _err, code) = run("cat <<<~/\"x\"");
    assert_eq!(
        (out.as_str(), code),
        (format!("{home}/x\n").as_str(), 0),
        "bash prints {home}/x"
    );
}

/// Task #282: the here-string word's tilde follows the `HOME` **shell
/// variable** — `HOME=/tmp/H; cat <<<~/x` is `/tmp/H/x` in bash 5.3.9, and an
/// empty `HOME` leaves just the `/x`.
#[test]
fn here_string_tilde_follows_the_shell_variable_home() {
    for (script, want) in [
        ("HOME=/tmp/H; cat <<<~/x", "/tmp/H/x\n"),
        ("HOME=/tmp/H; cat <<<~", "/tmp/H\n"),
        ("HOME=; cat <<<~/x", "/x\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
    }
}

/// Accepted divergence (task #222, to be closed by task #288): the here-string
/// word becomes the redirect's text and is never split, so a **quoted** `$@`
/// keeps task #222's separator-free join and the positionals glue, where bash
/// joins them with a space.
#[test]
fn quoted_dollar_at_in_a_here_string_is_glued() {
    // bash prints `a b c`, then `pa b c`.
    let (out, err, code) = run(r#"set -- "a b" c; cat <<<"$@""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a bc\n", "bash prints a b c");
    let (out, _err, code) = run(r#"set -- "a b" c; cat <<<"p$@""#);
    assert_eq!(code, 0);
    assert_eq!(out, "pa bc\n", "bash prints pa b c");
}

// --- the io-number form `[N]<<<word` (task #312) ----------------------------
//
// Every row below is measured against bash 5.3.9 (`bash -c`) and this tree's
// build on 2026-10-18. A plain `cat 3<<<x` cannot show fd 3's **content** —
// reading it back needs `<&3`, which is the intra-command gap task #178 owns —
// so the content-observable rows use the `exec` form: `exec 3<<<x` leaves fd 3
// in the shell's table (the `exec 3<<T` precedent) and `cat <&3` reads it back.

/// Run with the shell's fd 3 **closed from the start** (the same helper as
/// `tests/heredoc.rs`): a pin that asserts fd 3 is *closed* in the parent must
/// not depend on what the harness inherited (cargo-mutants hands every test
/// process its jobserver pipes as open fds ≥ 3 — LESSONS).
fn run_fd3_closed(script: &str) -> (String, String, i32) {
    let output = Command::new("sh")
        .args(["-c", "exec 3>&-; exec \"$0\" -c \"$1\"", BIN, script])
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

/// `cat 3<<<str` is a here-string on fd 3: `str\n` becomes fd 3's input, stdin
/// stays the harness's empty stdin, so `cat` prints nothing (bash: rc 0, no
/// stdout). This row used to sit in
/// `tests/heredoc.rs::numbered_heredoc_prefixes_that_are_not_digits_are_word_data`
/// as word data (`cat: '3<<<str'`); task #312 made it the operator it is.
#[test]
fn here_string_on_an_io_number_feeds_that_descriptor() {
    let (out, err, code) = run_fd3_closed("cat 3<<<str");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
}

/// The body's content, read back through the shell's fd table: quoting,
/// expansion, brace protection, last-wins and the empty word all behave exactly
/// as they do on stdin (bash-measured).
#[test]
fn here_string_on_an_io_number_delivers_its_body() {
    for (script, want) in [
        ("exec 3<<<x; cat <&3", "x\n"),
        ("exec 3<<<\"a b\"; cat <&3", "a b\n"),
        ("x=\"a b\"; exec 3<<<$x; cat <&3", "a b\n"),
        ("exec 3<<<{a,b}; cat <&3", "{a,b}\n"),
        ("exec 3<<<a 3<<<b; cat <&3", "b\n"),
        ("exec 3<<<; cat <&3", "\n"),
        ("exec 3<<< \"\"; cat <&3", "\n"),
        ("exec 12<<<x; cat <&12", "x\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
    }
}

/// The word's own mask (task #269) travels with the prefixed form: the offset
/// is `prefix + 3`, so a quoted `\<char>` pair keeps its backslash, an unquoted
/// one folds, and the mask-driven tilde expands.
#[test]
fn here_string_on_an_io_number_follows_the_word_mask() {
    let home = std::env::var("HOME").unwrap();
    for (script, want) in [
        ("exec 3<<<\"a\\ b\"; cat <&3", "a\\ b\n"),
        ("exec 3<<<a\\ b; cat <&3", "a b\n"),
        ("exec 3<<<~/x; cat <&3", format!("{home}/x\n").as_str()),
        ("exec 3<<<\"~\"; cat <&3", "~\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
    }
}

/// `N` is a descriptor number, so fd 0 is the stdin form and multi-digit and
/// leading-zero runs fold (`03` is 3, as in `3>file`).
#[test]
fn here_string_io_number_forms_target_their_descriptor() {
    let (out, err, code) = run("cat 0<<<x");
    assert_eq!((out.as_str(), code), ("x\n", 0), "stderr={err}");
    for script in ["cat 12<<<x", "cat 03<<<x", "cat 99999<<<x"] {
        let (out, err, code) = run_fd3_closed(script);
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err}");
    }
}

/// The bare form with a prefix: `3<<<` is the operator token (its 4 bytes are
/// `prefix + 3`), so the word is the **next** token and exactly one word is
/// consumed — `b` stays `cat`'s argument, as bash's `cat: b: No such file`
/// shows.
#[test]
fn prefixed_here_string_bare_form_consumes_one_word() {
    let (out, err, code) = run_fd3_closed("cat 3<<< str");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    let (_out, err, code) = run_fd3_closed("cat 3<<<a b");
    assert_eq!(code, 1, "err={err}");
    assert!(err.contains("cat: b"), "err={err}");
    assert!(!err.contains("3<<<a"), "err={err}");
    let (out, err, code) = run("exec 3<<< x; cat <&3");
    assert_eq!((out.as_str(), code), ("x\n", 0), "stderr={err}");
}

/// The operator consumes only its own word: the surrounding arguments survive
/// (bash: `echo a 3<<<x b` prints `a b`).
#[test]
fn prefixed_here_string_leaves_other_args_intact() {
    for (script, want) in [("echo a 3<<<x b", "a b\n"), ("echo a 3<<< x b", "a b\n")] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
    }
}

/// `3<<<{a,b}` is attached on its word, so the word is brace-protected (the
/// shifted `word_indices`): `cat` gets no arguments, and the body is the
/// literal `{a,b}` (bash: rc 0, no stdout).
#[test]
fn prefixed_here_string_word_is_brace_protected() {
    let (out, err, code) = run_fd3_closed("cat 3<<<{a,b}");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    let (out, err, code) = run("exec 3<<< {a,b}; cat <&3");
    assert_eq!((out.as_str(), code), ("{a,b}\n", 0), "stderr={err}");
}

/// A digit run that does not fit a descriptor number is a parse error naming
/// it — the `N<<word` family's rule (`parse_fd` is the ONE fd rule). bash
/// passes the run to the command instead (`cat: 999…: No such file`), a
/// divergence documented in POSIX_COMPATIBILITY.md.
#[test]
fn prefixed_here_string_rejects_an_io_number_that_is_not_a_descriptor() {
    let (out, err, code) = run("cat 99999999999999999999999<<<x");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(
        err.contains("here-string: '99999999999999999999999' is not a valid descriptor number"),
        "{err}"
    );
    assert!(!err.contains("redirection path"), "{err}");
}

/// The io-number rule is raw-byte, exactly like the `N<<word` rule of tasks
/// #261/#274/#276: the *token text* of every row here is `3<<<x`, so a
/// token-text rule would read fd 3 where bash has word data. The whole token
/// reaches the child as one argument and no descriptor number is reported.
#[test]
fn prefixed_here_string_shapes_that_are_not_digits_are_word_data() {
    for (script, needle) in [
        ("cat 3\"<<<\"x", "cat: '3<<<x'"),
        ("cat 3'<<<'x", "cat: '3<<<x'"),
        ("cat \"3\"<<<x", "cat: '3<<<x'"),
        ("cat \\3<<<x", "cat: '3<<<x'"),
        ("cat x3<<<y", "cat: 'x3<<<y'"),
        ("FD=3; cat $FD<<<x", "cat: '3<<<x'"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "script={script:?} err={err}");
        assert_eq!(out, "", "script={script:?}");
        assert!(err.contains(needle), "script={script:?} err={err}");
        assert!(
            !err.contains("valid descriptor number"),
            "script={script:?} err={err}"
        );
    }
}

/// A prefixed operator at a run start is fdshell's **command word** (the
/// operator rule needs a preceding token): the pinned `3<<EOF` twin. bash
/// opens fd 3 and reports `3: Bad file descriptor`.
///
/// Run through `run_fd3_closed`: the trailing `cat <&3` must read a
/// deterministic fd table. An inherited readable fd 3 (cargo-mutants hands
/// every test process its jobserver pipes as fds ≥ 3) would make `cat <&3`
/// block forever and abort the whole mutants baseline — LESSONS (task #276).
#[test]
fn prefixed_here_string_at_a_run_start_is_the_command_word() {
    let (out, err, code) = run_fd3_closed("3<<<x; cat <&3");
    assert_eq!(code, 1, "err={err}");
    assert_eq!(out, "");
    assert!(err.contains("\"3<<<x\" not found"), "{err}");
    assert!(err.contains("fd 3 is not open"), "{err}");
}

/// The form composes with everything the plain here-string composes with: the
/// parse is shared, so block arms, cond lists, pipeline stages, group bodies and
/// a second here-string on another descriptor all behave (bash-measured).
#[test]
fn prefixed_here_string_composes_with_the_statement_shapes() {
    for (script, want) in [
        ("if true; then cat 3<<<x; fi", ""),
        ("true && cat 3<<<x", ""),
        ("cat 3<<<x | cat", ""),
        ("{ cat 3<<<x; }", ""),
        ("cat <<<y 3<<<x", "y\n"),
    ] {
        let (out, err, code) = run_fd3_closed(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
        assert_eq!(err, "", "{script}: stderr={err}");
    }
}

/// A sibling `>&N`/`<&N` dup takes its source from the **parent's** fd table,
/// never from the descriptor a sibling redirect of the same command opens: the
/// gap task #178/#183 owns. bash prints `str` here. Recording the measured
/// report and naming the owner is the deliverable (the #276 precedent).
#[test]
fn prefixed_here_string_sibling_dup_cannot_see_the_descriptor() {
    for script in ["cat 3<<<str <&3", "cat 3<<<x 1>&3"] {
        let (out, err, code) = run_fd3_closed(script);
        assert_eq!(code, 1, "script={script:?} out={out:?}");
        assert_eq!(out, "", "script={script:?}");
        assert!(
            err.contains("fd 3 is not open"),
            "script={script:?} err={err}"
        );
    }
}

/// `cat 3<<<<<x` is the pinned `cat <<<<<x` shape fd-ified: the word `<<x`
/// goes to fd 3, so the command prints nothing; bash rejects the shape as a
/// syntax error (rc 2) — a documented divergence, shared with the unprefixed
/// twin. A bare `3<<<` at end of input takes the empty word (bash: rc 2), and
/// `3<<< ""` is one empty quoted word.
#[test]
fn prefixed_here_string_operator_tail_and_empty_word() {
    let (out, err, code) = run_fd3_closed("cat 3<<<<<x");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    let (out, err, code) = run("exec 3<<<<<x; cat <&3");
    assert_eq!((out.as_str(), code), ("<<x\n", 0), "stderr={err}");
    let (out, err, code) = run_fd3_closed("cat 3<<<");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
}

/// Divergences pinned as measured, each owned by the task named in its
/// comment: `&` background is a parse error for every command (#313 family); a
/// redirect after a group's `}` is not attached to the group (#187/#191); the
/// shared bare-operand guard fires on the *word text* of a quoted operand
/// (#323), so `cat 3<<< "<x"` is a syntax error where bash feeds `<x\n` to fd 3.
#[test]
fn prefixed_here_string_measured_divergences_name_their_owner() {
    let (out, err, code) = run("cat 3<<<x &");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}");
    assert!(err.contains("unexpected character"), "{err}");

    let (out, err, code) = run("{ cat; } 3<<<x");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}");
    assert!(err.contains("group: unexpected word after '}'"), "{err}");

    let (out, err, code) = run("cat 3<<< \"<x\"");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}");
    assert!(err.contains("invalid redirect syntax"), "{err}");
}

/// An expansion failure in the prefixed word is reported like the stdin form's
/// (`here_string_expansion_failure_is_reported`): bash prints nothing, rc 0.
#[test]
fn prefixed_here_string_expansion_failure_is_reported() {
    let (_out, err, code) = run("cat 3<<<$99999999999999999999");
    assert_ne!(code, 0);
    assert!(err.contains("here-string"), "stderr={err}");
}
