#![allow(clippy::unwrap_used)]
//! Single-quote spans (task #141): a `'…'` span strips both quote bytes and is
//! one literal word — everywhere, in every code path.
//!
//! Each expectation below is the recorded output of `bash` (5.x on this
//! machine) for the same script, in the comment above the assertion.
//! Divergences are marked as such:
//! * `$'…'` is read as the POSIX-sh expansion, so `echo $'x'` is `$x` and not
//!   bash's `x` — tracked by task #156.
//! * An unbalanced quote is a parse error with fdshell's own message and exit
//!   code 1; bash prints its own syntax error with code 2. The pre-existing
//!   double-quote case behaves identically, so `'` follows the shell's
//!   convention (see STYLE.md §4).
//!
//! A `case` pattern takes the mask of its own span like every other word
//! surface (task #280 closed the divergence task #246 tracked); its
//! single-quoted rows are pinned in `tests/case.rs`
//! (`case_pattern_quote_mask_is_per_byte`, `case_pattern_mask_and_expansion_match_bash`).

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::str;
use std::sync::atomic::Ordering;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// `run` with an explicit child environment (task #282): `(key, value)` sets
/// the pair, and the `"\0"` value *removes* the key — the only way to give
/// the child no `HOME` at all, since the harness runs with one.
fn run_env(script: &str, env: &[(&str, &str)]) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        if *value == "\0" {
            cmd.env_remove(key);
        } else {
            cmd.env(key, value);
        }
    }
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// A scratch dir holding `names` as empty files; `run_in` runs there so globs
/// and redirects see exactly these files.
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

fn scratch(names: &[&str]) -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-quote-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    Scratch(dir)
}

fn run_in(dir: &std::path::Path, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .current_dir(dir)
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

/// A scratch dir of executable stubs used as a `PATH` prefix: a quoted command
/// name is looked up as a real file, and each stub prints its own name.
fn scratch_bin(names: &[&str]) -> Scratch {
    let dir = scratch(names);
    for name in names {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho ran-{}\n", name)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn run_with_path(dir: &std::path::Path, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
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

/// bash: `echo 'a b'` → `a b`; `echo 'a"b'` → `a"b`; `echo a'b c'd` → `ab cd`
/// (one word: the span's space survives, no split at it); `echo 'a''b'` → `ab`;
/// `echo ''` → a blank line.
#[test]
fn span_loses_both_quote_bytes_and_never_splits() {
    for (script, want) in [
        ("echo 'a b'", "a b\n"),
        ("echo 'a\"b'", "a\"b\n"),
        ("echo 'a b c'", "a b c\n"),
        ("echo a'b c'd", "ab cd\n"),
        ("echo 'a''b'", "ab\n"),
        ("echo x'a'b'y'", "xaby\n"),
        ("echo ''", "\n"),
        // A span is one word glued to an unquoted neighbour.
        ("echo 'a b'c", "a bc\n"),
        // Backslashes in a span are literal (POSIX §2.2.3): a span's `\`
        // shields nothing, and a span never forms an escape pair.
        ("echo 'a\\b'", "a\\b\n"),
        ("echo 'a\\*b'", "a\\*b\n"),
        ("echo 'a\\ b'", "a\\ b\n"),
        // The double quote is data in a span, and a span is data in `"` —
        // neither toggles the other.
        ("echo \"a'b'c\"", "a'b'c\n"),
        ("echo \"a\\b\"", "a\\b\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// bash: `echo 'a; b'`, `'a|b'`, `'a&&b'`, `'a>b'`, `'a<b'`, `'a*b'`,
/// `'{a,b}'`, `'a?b'` all print their bytes verbatim: inside a span every
/// metacharacter is data.
#[test]
fn span_metacharacters_are_data() {
    for (script, want) in [
        ("echo 'a; b'", "a; b\n"),
        ("echo 'a;b'", "a;b\n"),
        ("echo 'a|b'", "a|b\n"),
        ("echo 'a&&b'", "a&&b\n"),
        ("echo 'a>b'", "a>b\n"),
        ("echo 'a<b'", "a<b\n"),
        ("echo 'a*b'", "a*b\n"),
        ("echo 'a?b'", "a?b\n"),
        ("echo '{a,b}'", "{a,b}\n"),
        ("echo '$a'", "$a\n"),
        ("echo '`a`'", "`a`\n"),
        ("echo '$(a)'", "$(a)\n"),
        ("echo '#a'", "#a\n"),
        ("echo 'a#b'", "a#b\n"),
        ("echo '(a)'", "(a)\n"),
        ("echo '!'", "!\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// bash: with files `ca1 ca2` present, `echo 'ca?t'` prints `ca?t` and
/// `echo 'ca*'` prints `ca*` — a span is never globbed, and a word that
/// contains a span is not globbed either.
#[test]
fn span_is_never_globbered() {
    let dir = scratch(&["ca1", "ca2", "a b"]);
    for (script, want) in [
        ("echo 'ca?t'", "ca?t\n"),
        ("echo 'ca*'", "ca*\n"),
        ("echo ca'?'", "ca?\n"),
        ("echo 'ca1 ca2'", "ca1 ca2\n"),
        ("for w in 'a b' c; do echo \"$w\"; done", "a b\nc\n"),
    ] {
        let (out, err, code) = run_in(&dir, script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// bash: `set -- ''` leaves one positional parameter, the empty word (`$#` is
/// 1, `$1` is empty); `echo "[$1]"` prints `[]`.
#[test]
fn empty_span_is_an_empty_word() {
    let (out, err, code) = run("set -- ''; echo $#; echo \"[$1]\"");
    assert_eq!((out.as_str(), code), ("1\n[]\n", 0), "stderr={err:?}");
    // An empty span glued to a word contributes nothing.
    let (out, err, code) = run("set -- 'x''y'; echo $#; echo \"[$1]\"");
    assert_eq!((out.as_str(), code), ("1\n[xy]\n", 0), "stderr={err:?}");
    // The span's word survives as one argument (`set -- 'a b' c` → 2 args).
    let (out, err, code) = run("set -- 'a b' c; echo $#; echo \"[$1]\" \"[$2]\"");
    assert_eq!(
        (out.as_str(), code),
        ("2\n[a b] [c]\n", 0),
        "stderr={err:?}"
    );
}

/// bash: `echo 'x y' > 'a b'` writes `x y` into a file named `a b`;
/// `cat <<'EOF'` prints the body verbatim (no expansion) and the terminator is
/// `EOF`; `cat <<'a b'` is terminated by the line `a b`.
#[test]
fn span_in_a_redirect_target_and_a_heredoc_delimiter() {
    let dir = scratch(&[]);
    let (out, err, code) = run_in(&dir, "echo 'x y' > 'a b'; cat 'a b'");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x y\n");

    // `cat <<'EOF'` — the delimiter is `EOF` (quotes stripped) and the body is
    // literal: `$HOME`, a backslash and a glob byte are printed as written.
    let script = "cat <<'EOF'\n$HOME a\\*b `x`\nEOF\n";
    let (out, err, code) = run(script);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "$HOME a\\*b `x`\n");

    // A delimiter that contains a space: the span is one word.
    let script = "cat <<'a b'\nbody\na b\n";
    let (out, err, code) = run(script);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "body\n");

    // The unquoted form still expands the body.
    let script = "cat <<EOF\n$?x\nEOF\n";
    let (out, err, code) = run(script);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "0x\n");
}

/// bash: `'time' ls` runs the `time` **utility** (`/usr/bin/time ls`) — a
/// quoted `time` is a command word, not the reserved word. The scratch `PATH`
/// holds stubs so the assertion is hermetic: each stub prints its own name,
/// which proves the quoted word is looked up in `PATH` as a command name.
#[test]
fn quoted_keyword_words_are_command_words() {
    let bin = scratch_bin(&["time", "ls", "then"]);
    for (script, want) in [
        // The stub `time` runs, so the keyword never claims the statement.
        ("'time' ls", "ran-time\n"),
        ("\"time\" ls", "ran-time\n"),
        ("'ls' -d .", "ran-ls\n"),
    ] {
        let (out, err, code) = run_with_path(&bin, script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
    // A quoted closer in a block body is data: the block stays open for the
    // real closer (bash: `if true; then echo 'fi'; fi` prints `fi`).
    let (out, err, code) = run("if true; then echo 'fi'; fi");
    assert_eq!((out.as_str(), code), ("fi\n", 0), "stderr={err:?}");
    let (out, err, code) = run("while :; do echo 'done'; break; done");
    assert_eq!((out.as_str(), code), ("done\n", 0), "stderr={err:?}");
    // A non-keyword reserved word in a body position is a command word
    // (bash: `x | 'then' y` runs `then`).
    let bin = scratch_bin(&["then"]);
    let (out, err, code) = run_with_path(&bin, "echo hi | 'then'");
    assert_eq!((out.as_str(), code), ("ran-then\n", 0), "stderr={err:?}");
}

/// Task #247 (POSIX §2.9.1): a block-opener keyword written with quote bytes
/// is a **command word**, as bash does — the gate is the word's raw span
/// (`keywords::quoted::quoted_word`), so every spelling that carries a quote
/// byte is data: `'if'`, `"if"`, `i"f"`, `""if`, `''if`, `"i"f`. The scratch
/// `PATH` holds a stub per name, so "looked up in `PATH` as a command name" is
/// proved, not inferred from a not-found message (bash: `if: command not
/// found`, rc 127; fdshell's command-not-found code is 1, a shell-wide
/// convention, so the *name* is pinned, not the code).
///
/// The definer rows pin stdout as well as the code: on one line the quoted
/// definer's body never runs (bash's stdout too), and the multi-line forms are
/// pinned as the accepted divergence they are (fdshell runs the body group,
/// bash prints nothing).
#[test]
fn quoted_block_openers_are_command_words() {
    let bin = scratch_bin(&[
        "if", "while", "until", "for", "case", "{", "function", "then", "else",
    ]);
    for (script, want) in [
        ("'if' x", "ran-if\n"),
        ("\"if\" x", "ran-if\n"),
        ("i\"f\" x", "ran-if\n"),
        ("\"\"if x", "ran-if\n"),
        ("''if x", "ran-if\n"),
        ("\"i\"f x", "ran-if\n"),
        ("'while' x", "ran-while\n"),
        ("'until' x", "ran-until\n"),
        ("'for' x", "ran-for\n"),
        ("'case' x", "ran-case\n"),
        ("'{' x", "ran-{\n"),
        ("'function' x", "ran-function\n"),
    ] {
        let (out, err, code) = run_with_path(&bin, script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
    // No stub: the shell reports the quoted word's own name as the missing
    // command (rc 1, the shell-wide convention — bash prints 127).
    let (out, err, code) = run("echo a; 'if' true");
    assert_eq!((out.as_str(), code), ("a\n", 1), "stderr={err:?}");
    assert!(err.contains(r#""if""#), "stderr={err:?}");
    // The keyword words *inside* a block are gated too: the block reports its
    // own missing keyword (bash: a syntax error, rc 2, runs nothing; fdshell
    // reports the parse error with rc 1 — the rc-1-vs-2 family).
    for (script, needle) in [
        ("if true; 'then' echo a; fi", "missing 'then'"),
        ("if true; ''then echo a; fi", "missing 'then'"),
        ("if true; \"\"then echo a; fi", "missing 'then'"),
        ("for i 'in' a; do echo $i; done", "expected 'in'"),
        ("for i in a b; 'do' echo $i; done", "expected 'do'"),
        ("for i in a b; \"\"do echo $i; done", "expected 'do'"),
        ("case x 'in' a) echo A;; esac", "case: missing 'in'"),
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: stderr={err:?}");
        assert!(err.contains(needle), "{script}: stderr={err:?}");
    }
    // A quoted `else`/`elif` in the closer position is a command word run
    // *after* the arm that parsed: bash prints the same `a` and then its
    // `else: command not found`.
    let (out, err, code) = run("if true; then echo a; 'else' echo b; fi");
    assert_eq!((out.as_str(), code), ("a\n", 1), "stderr={err:?}");
    assert!(err.contains(r#""else""#), "stderr={err:?}");
    let (out, err, code) = run("if true; then echo a; 'elif' false; then echo b; fi");
    assert_eq!((out.as_str(), code), ("a\n", 1), "stderr={err:?}");
    assert!(err.contains(r#""elif""#), "stderr={err:?}");
    // A quoted definer word defines no function (bash: `` 'f'': not a valid
    // identifier ``), and the shape falls back to the `function` command. On one
    // line nothing of the body runs, so the stdout is bash's stdout (empty, and
    // `a` for the row that starts with a statement of its own).
    for (script, want) in [
        ("'function' f() { echo hi; }; f", ""),
        ("function 'f'() { echo hi; }; f", ""),
        ("f'oo'() { echo hi; }; f", ""),
        ("function 'f' { echo hi; }; f", ""),
        ("echo a; function 'f' { echo hi; }; f", "a\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 1), "{script}: stderr={err:?}");
        assert!(err.contains("not found"), "{script}: stderr={err:?}");
    }
    // Accepted divergence (task #247): the **multi-line** forms run their body as
    // the `{ …; }` group the definer's brace becomes, where bash aborts the whole
    // input with `'f'`: not a valid identifier (rc 127, prints nothing) — the
    // same "parse statement by statement" family as the rows below. The
    // byte-level gate walks `line` from the definer word's own byte, so the
    // verdict is indentation-invariant: every leading width prints the same.
    for indent in ["", " ", "  ", "   "] {
        for script in [
            format!("{indent}function 'f' {{\necho hi\n}}\nf"),
            format!("{indent}f'oo'() {{\necho hi\n}}\nf"),
        ] {
            let (out, err, code) = run(&script);
            assert_eq!(
                (out.as_str(), code),
                ("hi\n", 1),
                "{script}: stderr={err:?}"
            );
            assert!(err.contains("not found"), "{script}: stderr={err:?}");
        }
    }
    // Accepted divergence (plan §8): fdshell parses statement by statement, so a
    // quoted opener runs as a command and the *rest of the line* runs too; bash
    // parses the whole line first and dies on the syntax error (rc 2, runs
    // nothing). Pinning the fallout keeps the multi-statement blast radius visible.
    for script in [
        "'while' :; do echo a; break; done",
        "'if' true; then echo b; fi",
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: stderr={err:?}");
        assert!(err.contains("not found"), "{script}: stderr={err:?}");
    }
}

/// bash (`shopt -s expand_aliases`, `alias q="echo a'b'"` then `q`) prints
/// `ab`: the alias body is spliced, then its span loses its quotes.
#[test]
fn alias_body_round_trips_a_span() {
    let (out, err, code) = run("alias q=\"echo a'b'\"; q");
    assert_eq!((out.as_str(), code), ("ab\n", 0), "stderr={err:?}");
    // A quoted command word is never aliased (bash: `'q'` runs command `q`, so
    // "command not found"). fdshell's command-not-found exit code is 1, not
    // bash's 127 — a pre-existing shell-wide convention (master behaves the
    // same for unquoted names), so only the "not found" is pinned here.
    let (out, err, code) = run("alias q=\"echo aliased\"; 'q'");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("not found") || err.contains("resolve command path"),
        "stderr={err:?}"
    );
    let _ = out;
}

/// bash: `echo 'a` (unterminated) is a syntax error. fdshell reports its own
/// "unmatched quote" at the quote byte and exits 1 — the same convention as
/// the pre-existing double-quote case. The caret line must sit on the byte of
/// the `'` that opened the span.
#[test]
fn unbalanced_span_is_a_parse_error_at_the_quote() {
    // (script, byte index of the `'` that opens the unbalanced span)
    for (script, quote) in [
        ("echo 'a", 5usize),
        ("echo x 'y", 7),
        ("echo 'a' 'b", 9),
        ("echo '", 5),
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: stderr={err:?}");
        assert!(err.contains("unmatched quote"), "{script}: stderr={err:?}");
        // The report shows the script text, and a caret line whose caret is
        // the column of the opening quote on that line.
        let shown = err.lines().find(|l| l.contains(script)).unwrap();
        let text_col = shown[..shown.find(script).unwrap()].chars().count();
        let caret_line = err.lines().find(|l| l.trim_end().ends_with('^')).unwrap();
        let caret_col = caret_line[..caret_line.find('^').unwrap()].chars().count();
        assert_eq!(
            caret_col,
            text_col + quote,
            "{script}: caret must point at byte {quote}, stderr={err:?}"
        );
    }
    // Inside a block body the block scanner reports its own parse error.
    let (_out, err, code) = run("if true; then echo 'a");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("parse error"), "stderr={err:?}");
}

/// Divergence (task #156): bash's `echo $'x'` prints `x`; fdshell reads
/// `$'…'` as the POSIX-sh expansion, so the `$` survives and it prints `$x`.
/// Pinning it here keeps the divergence visible and honest.
#[test]
fn dollar_quote_keeps_the_dollar() {
    let (out, err, code) = run("echo $'x'");
    assert_eq!((out.as_str(), code), ("$x\n", 0), "stderr={err:?}");
    // `$` followed by a span is a literal `$` glued to the span's text.
    let (out, err, code) = run("echo a$'b c'd");
    assert_eq!((out.as_str(), code), ("a$b cd\n", 0), "stderr={err:?}");
    // The same family (task #223): bash's `$"…"` is a locale translation and
    // in the C locale it is the span's text; fdshell reads the `$` as the
    // POSIX-sh expansion, so `v=q; printf "[%s]" $"v"` prints `[$v]`.
    let (out, err, code) = run(r#"v=q; printf "[%s]" $"v""#);
    assert_eq!((out.as_str(), code), ("[$v]", 0), "stderr={err:?}");
}

/// The child shell parses the same source: a `'…'` span survives the quote
/// bytes of the outer command line. bash: `fdshell -c 'echo hi'` prints `hi`;
/// `echo '$(true)'` then evaluating it in the child prints `$(true)`.
#[test]
fn child_shell_sees_the_same_span() {
    let cmd = format!("{BIN} -c 'echo hi'");
    let (out, err, code) = run(&cmd);
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    // The span keeps the child's command line intact through the quotes.
    let cmd = format!("{BIN} -c 'echo \"a b\"'");
    let (out, err, code) = run(&cmd);
    assert_eq!((out.as_str(), code), ("a b\n", 0), "stderr={err:?}");
}

/// bash: `printf '%q\n' 'a b'` prints `a\ b`; `printf '%s|%s' 'a b' ''`
/// prints `a b|`: the span's text reaches the builtin as one argument.
#[test]
fn span_text_reaches_builtins_as_one_argument() {
    for (script, want) in [
        ("printf '%q\\n' 'a b'", "a\\ b\n"),
        ("printf '%s|%s' 'a b' ''", "a b|"),
        ("printf '%s\\n' 'a\\b'", "a\\b\n"),
        // A span's literal `$` and `\` reach the builtin unchanged: the same
        // text the substitution pass would produce (POSIX #4.2).
        ("printf '%s\\n' '$x'", "$x\n"),
        ("printf '%s\\n' 'a$b'", "a$b\n"),
        ("printf '%s\\n' '\\'", "\\\n"),
        ("test 'a b' = 'a b' && echo eq", "eq\n"),
        // `set -u` and an empty span: the word is empty, not missing.
        ("set -u; f=''; echo \"${f}x\"", "x\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// The span's literal backslash is what makes the byte-level scanners agree with
/// the tokenizer, and each case below is byte-identical to bash:
/// * `echo 'a\>'|b` pipes to the command `b` (the `>` is span data and the
///   span's `\` forms no escape pair, so it is not a `>|` clobber operator);
/// * `echo "a'b>"c` prints `a'b>c` (a `'` written in `"` opens no span, so the
///   `>` stays inside the double quotes);
/// * `echo {x,'a\}'}` prints `x a\}` (the span's `\` does not swallow its
///   closing quote, so the group is found and the member keeps a literal `\}`);
/// * `echo $(echo 'a\)b')` prints `a\)b` (the `)` written in the span is body
///   data: the substitution ends at the span's own quote and the real `)`).
#[test]
fn span_backslash_keeps_the_byte_level_scanners_in_step() {
    let bin = scratch_bin(&["b"]);
    let (out, err, code) = run_with_path(&bin, "echo 'a\\>'|b");
    assert_eq!((out.as_str(), code), ("ran-b\n", 0), "stderr={err:?}");
    for (script, want) in [
        ("echo \"a'b>\"c", "a'b>c\n"),
        ("echo {x,'a\\}'}", "x a\\}\n"),
        ("echo $(echo 'a\\)b')", "a\\)b\n"),
        ("echo a'x\\'b", "ax\\b\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// Documented divergence (README "Quoting" and "Nested regions inside double
/// quotes"): a quote written inside a `${ … }` body is a marker, not content —
/// the region scan drops **both** quote bytes and the body is then read as
/// expansion text, so a span's protection is lost there. bash strips `"` bytes
/// in an alternate word and keeps `'` bytes literal. The model is pre-existing
/// for `"` (task #174) and #141 made `'…'` follow it; the common unquoted case
/// matches bash exactly, and master printed the region literally.
#[test]
fn region_body_drops_the_span_quote_bytes() {
    // bash: `a b` — unquoted, the span is one word before the expansion runs.
    let (out, err, code) = run("v=; echo ${v:-'a b'}");
    assert_eq!((out.as_str(), code), ("a b\n", 0), "stderr={err:?}");
    // bash: `a b` — the `"` form is identical on every side.
    let (out, err, code) = run("v=; echo \"${v:-\"a b\"}\"");
    assert_eq!((out.as_str(), code), ("a b\n", 0), "stderr={err:?}");
    // bash: `'a b'` — fdshell drops the span's quotes from the body text.
    let (out, err, code) = run("v=; echo \"${v:-'a b'}\"");
    assert_eq!((out.as_str(), code), ("a b\n", 0), "stderr={err:?}");
    // bash: `a\}b` — a literal `}` written in an alternate word diverges too
    // (the region ends at the span's `}`, the body keeps a literal `\}`).
    let (out, err, code) = run("v=; echo ${v:-'a\\}b'}");
    assert_eq!((out.as_str(), code), ("a\\\\b}\n", 0), "stderr={err:?}");
}

/// Task #155: the shell performs no percent escaping — `%%` is data in every
/// spelling. bash prints `%%` for `echo '%%'` and for `echo "%%"`; the only
/// literal-`%` escape is the POSIX #4.1 pair outside quotes, and with fd `s`
/// bound bash's `echo [\%s]` prints `[%s]` (fdshell has no `%name` feature to
/// escape from, so it prints the same).
#[test]
fn percent_bytes_are_data_in_a_span() {
    for (script, want) in [("echo '%%'", "%%\n"), ("echo \"%%\"", "%%\n")] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
    let (out, err, code) = run("builtin openat2 --flags O_RDONLY /dev/null %>%s; echo [\\%s]");
    assert_eq!((out.as_str(), code), ("[%s]\n", 0), "stderr={err:?}");
}

/// Task #207: a here-doc delimiter word is a word, so it gets POSIX quote
/// removal — **every** `'` byte is removed, not one surrounding pair, and any
/// quoting in the word makes the body literal. Each expectation is bash's own
/// output for the same script (the terminator line is the stripped word).
#[test]
fn heredoc_delimiter_strips_every_single_quote() {
    // bash: empty body, rc 0 (`<<a'b'c` delimits `abc`, terminated by `abc`).
    let (out, err, code) = run("cat <<a'b'c\nabc");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // bash: empty body (`<<'x'y'z'` delimits `xyz`).
    let (out, err, code) = run("cat <<'x'y'z'\nxyz");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // bash: empty body (`<<ab'c'` and `<<'a'bc` both delimit `abc`).
    let (out, err, code) = run("cat <<ab'c'\nabc");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("cat <<'a'bc\nabc");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // bash: empty body, tab-indented terminator (`<<-` form, separate word).
    let (out, err, code) = run("cat <<- a'b'c\n\tabc");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // bash: `$HOME` — a quoted delimiter word makes the body literal.
    let (out, err, code) = run("cat <<a'b'c\n$HOME\nabc");
    assert_eq!((out.as_str(), code), ("$HOME\n", 0), "stderr={err:?}");
    // bash: `body $i` twice — a loop body's heredoc keeps the same rule.
    let (out, err, code) = run("for i in 1 2; do cat <<a'b'c\nbody $i\nabc\ndone");
    assert_eq!(
        (out.as_str(), code),
        ("body $i\nbody $i\n", 0),
        "stderr={err:?}"
    );
}

/// A quoted `~` is a literal byte: POSIX removes the quotes *after* tilde
/// expansion, and bash keeps `echo "~"` and `echo "~/x"` literal (task #269's
/// mask gate in `substitute/tilde.rs`). The unquoted pair expands, as before.
///
/// Task #281 widens the gate to the whole tilde-prefix (POSIX #2.6.1: a tilde
/// expands only when **no** quoted character appears in the bytes from `~`
/// through the first `/`, the `/` included). Every row below is bash's own
/// output, measured with `HOME=/h/o/m/e`, rc 0 in both shells.
#[test]
fn quoted_leading_tilde_is_literal() {
    let (out, err, code) = run(r#"printf "[%s]" "~""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[~]", "bash prints [~]");
    let (out, _err, code) = run(r#"printf "[%s]" "~/x""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~/x]", "bash prints [~/x]");
    let (home, _err, code) = run(r#"printf "%s" "$HOME""#);
    assert_eq!(code, 0);
    let (out, _err, code) = run(r#"printf "[%s]" ~"#);
    assert_eq!(code, 0);
    assert_eq!(out, format!("[{home}]"));
    let (out, _err, code) = run(r#"printf "[%s]" ~/x"#);
    assert_eq!(code, 0);
    assert_eq!(out, format!("[{home}/x]"));
    // A single-quoted span is literal too, as everywhere (`'~'` is `~`).
    let (out, _err, code) = run(r#"printf "[%s]" '~'"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~]");
    // A quoted character in the tilde-prefix suppresses the expansion, and the
    // quote's own removal is all that is left: bash prints `[~/*]` for `~"/*"`,
    // `[~/*/sub]` for `~"/*"/sub`, `[~/x]` for `~"/x"`, `~'/'x` and `~"/*"`
    // spelled with single quotes (task #281).
    let (out, _err, code) = run(r#"printf "[%s]" ~"/*""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[~/*]", "bash prints [~/*]");
    let (out, _err, code) = run(r#"printf "[%s]" ~"/*"/sub"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~/*/sub]", "bash prints [~/*/sub]");
    let (out, _err, code) = run(r#"printf "[%s]" ~'/*'"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~/*]", "bash prints [~/*]");
    for word in [r#"~"/x""#, r#"~'/'x"#] {
        let (out, _err, code) = run(&format!("printf \"[%s]\" {word}"));
        assert_eq!(code, 0, "{word}: stderr={err}");
        assert_eq!(out, "[~/x]", "{word}: bash prints [~/x]");
    }
    // The contrast: with the `/` unquoted the quote sits after the prefix, so
    // the tilde does expand (`~/"x"` is `[<HOME>/x]` in bash).
    let (out, _err, code) = run(r#"printf "[%s]" ~/"x""#);
    assert_eq!(code, 0);
    assert_eq!(out, format!("[{home}/x]"), "bash prints [{home}/x]");
    // Word-start rule and no-`~user`-lookup rows that stay agreement:
    // `~"a"b` and `~a/"x"` print `[~ab]` and `[~a/x]`; `x~"/*"` prints
    // `[x~/*]`; `~\/x` prints `[~/x]`.
    let (out, _err, code) = run(r#"printf "[%s]" ~"a"b"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~ab]", "bash prints [~ab]");
    let (out, _err, code) = run(r#"printf "[%s]" ~a/"x""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~a/x]", "bash prints [~a/x]");
    let (out, _err, code) = run(r#"printf "[%s]" x~"/*""#);
    assert_eq!(code, 0);
    assert_eq!(out, "[x~/*]", "bash prints [x~/*]");
    let (out, _err, code) = run(r#"printf "[%s]" ~\/x"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~/x]", "bash prints [~/x]");
    // Task #321: a quote pair that contributes **no bytes** leaves no mask bit
    // at all, so the mask alone cannot say the prefix was quoted — the
    // tokenizer's span list is what suppresses the expansion. Measured in bash
    // 5.3.9 with `HOME=/h/o/m/e`, rc 0 in both shells: the empty pairs keep the
    // word literal, and the same pair *after* the `/` leaves the tilde
    // expandable (`~/""` is `[<HOME>/]`).
    for (word, want) in [
        ("~\"\"", "[~]"),
        ("~''", "[~]"),
        ("~\"\"/x", "[~/x]"),
        ("~''/x", "[~/x]"),
        ("~\"\"''/x", "[~/x]"),
        ("\"\"~/x", "[~/x]"),
        ("~x\"\"", "[~x]"),
    ] {
        let (out, err, code) = run(&format!("printf \"[%s]\" {word}"));
        assert_eq!(code, 0, "{word}: stderr={err}");
        assert_eq!(out, want, "{word}: bash prints {want}");
    }
    // The empty span *after* the prefix's end leaves the tilde expandable:
    // `printf "[%s]" ~/""` is `[<HOME>/]` in bash.
    let (out, err, code) = run("printf \"[%s]\" ~/\"\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, format!("[{home}/]"), "bash prints [{home}/]");
    // The `$'…'` spelling of the empty span is a **documented divergence**, not
    // part of this rule: fdshell has no `$'…'` quoting at all (task #156), so
    // `~$''/x` is the three bytes `~$` + `/x` here, where bash prints `[~/x]`.
    // Pinned as measured so #156 has to face the row when it lands.
    let (out, _err, code) = run("printf \"[%s]\" ~$''/x");
    assert_eq!(code, 0);
    assert_eq!(
        out, "[~$/x]",
        "bash prints [~/x]: the $'…' gap is task #156"
    );
}

/// Task #282: `~` is driven by the `HOME` **shell variable** — the same
/// lookup `$HOME` uses (the shell's own variables first, the environment
/// fdshell started with as the fallback), POSIX #2.6.1's rule. Every row is
/// bash 5.3.9's own output, measured on this machine.
///
/// ```text
/// HOME=/tmp/H; printf "[%s] %s" ~ "$HOME"  → [/tmp/H] /tmp/H
/// HOME=/tmp/H; printf "[%s]" ~/x           → [/tmp/H/x]
/// HOME=/tmp/H; printf "[%s]" ~/"x"         → [/tmp/H/x]
/// HOME=/tmp/H; export HOME; printf %s ~    → [/tmp/H]
/// HOME=;       printf "[%s]" ~/x           → [/x]
/// HOME=/tmp/H printf "[%s]" ~              → the inherited HOME, § below
/// env HOME=a,b (no assignment) printf %s ~ → [a,b]
/// ```
#[test]
fn shell_variable_home_drives_tilde() {
    for (script, want) in [
        (r#"HOME=/tmp/H; printf "[%s]" ~"#, "[/tmp/H]"),
        (r#"HOME=/tmp/H; printf "[%s]" ~/x"#, "[/tmp/H/x]"),
        (
            r#"HOME=/tmp/H; printf "[%s] %s" ~ "$HOME""#,
            "[/tmp/H] /tmp/H",
        ),
        (r#"HOME=/tmp/H; printf "[%s]" ~/"x""#, "[/tmp/H/x]"),
        (r#"HOME=/tmp/H; export HOME; printf "[%s]" ~"#, "[/tmp/H]"),
        // An empty `HOME` is set, so it expands to nothing. POSIX #2.6.1 would
        // want a login directory for an empty `HOME`; bash prints `/x` — this
        // row is agreement with bash, not with POSIX.
        (r#"HOME=; printf "[%s]" ~/x"#, "[/x]"),
        (r#"HOME=; printf "[%s]" ~"#, "[]"),
        // The gate (#269 + #281) is untouched by the lookup change.
        (r#"HOME=/tmp/H; printf "[%s]" "~""#, "[~]"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
    }
    // The scoped prefix assignment: tilde expansion runs *before* the prefix
    // is applied, so `~` sees the inherited `HOME` — exactly what `$HOME`
    // sees in the same shape (measured: both print the inherited HOME).
    let (tilde, err, code) = run(r#"HOME=/tmp/H printf "[%s]" ~"#);
    assert_eq!(code, 0, "stderr={err}");
    let (dollar, err, code) = run(r#"HOME=/tmp/H printf "[%s]" "$HOME""#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(
        tilde, dollar,
        "a prefix `HOME` moves neither `~` nor `$HOME`"
    );
    assert_ne!(tilde, "[/tmp/H]", "bash: the inherited HOME, not /tmp/H");
    // Fallback: `HOME` only in the inherited environment (no assignment).
    let (out, err, code) = run_env(r#"IFS=,; builtin printf "[%s]" ~"#, &[("HOME", "a,b")]);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a,b]", "the inherited HOME is the fallback");
}

/// Task #282's accepted divergence, pinned as measured: with `HOME` set
/// nowhere, bash falls back to a login directory from the password database
/// (`/home/damir` here) and fdshell's `~` contributes nothing. `HOME` is
/// *removed* from the child's environment, so no fallback exists.
#[test]
fn tilde_without_home_contributes_nothing() {
    for (script, want) in [
        (r#"printf "[%s]" ~"#, "[]"),
        (r#"printf "[%s]" ~/x"#, "[/x]"),
        // nounset never fires on `~`: it is not a parameter reference (bash
        // prints its login directory with rc 0 in the same shape).
        (r#"set -u; printf "[%s]" ~"#, "[]"),
    ] {
        let (out, err, code) = run_env(script, &[("HOME", "\0")]);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}: bash prints the login directory");
    }
}

#[test]
fn dollar_at_a_region_end_is_a_literal_byte() {
    let (out, err, code) = run(r#"v=q; printf "[%s]" "$"v"#);
    assert_eq!((out.as_str(), code), ("[$v]", 0), "stderr={err:?}");
    let (out, err, code) = run(r#"v=q; echo a"$"v"#);
    assert_eq!((out.as_str(), code), ("a$v\n", 0), "stderr={err:?}");
    // The word is all literal (the `$` arm ran into the region cap and
    // expanded nothing), so task #222's provenance rule keeps it whole:
    // fdshell `[$v,a]` = bash (POSIX 2.6.5 splits only expansion bytes; a
    // literal `,` glues).
    let (out, err, code) = run(r#"IFS=,; v=q; printf "[%s]" "$"v,a"#);
    assert_eq!((out.as_str(), code), ("[$v,a]", 0), "stderr={err:?}");
}

/// A quote region ends a bare name (task #285): the mask spells where each
/// region starts (`MaskBit::QuotedStart`), so `$name` stops at the closing
/// quote of the region holding the `$` — bash's rule, now fdshell's.
#[test]
fn adjacent_quoted_regions_end_the_name() {
    // `"$v"x`: one region, then a literal byte.
    let (out, err, code) = run(r#"v=q; vx=VX; printf "[%s]" "$v"x"#);
    assert_eq!((out.as_str(), code), ("[qx]", 0), "stderr={err:?}");
    // `"$v""b"`: the second `"` opens a new region, so the name is `v` and `b`
    // is a literal — not the `vb` the single-region mask used to collect.
    let (out, err, code) = run(r#"v=q; vb=VB; printf "[%s]" "$v""b""#);
    assert_eq!((out.as_str(), code), ("[qb]", 0), "stderr={err:?}");
    // Two regions, no quotes on the tail: `q` + `x` + `y`.
    let (out, err, code) = run(r#"v=q; vx=VX; printf "[%s]" "$v"'x'y"#);
    assert_eq!((out.as_str(), code), ("[qxy]", 0), "stderr={err:?}");
    // A positional stops the same way: `"$1""2"` is `A` plus the literal `2`,
    // not the unset positional 12 the old mask implied.
    let (out, err, code) = run(r#"set -- A B; printf "[%s]" "$1""2""#);
    assert_eq!((out.as_str(), code), ("[A2]", 0), "stderr={err:?}");
    // The acceptance rows of the report, as positional parameters.
    let (out, err, code) = run(r#"v=q; vx=VX; set -- "$v"x; printf "[%s]\n" "$@""#);
    assert_eq!((out.as_str(), code), ("[qx]\n", 0), "stderr={err:?}");
    let (out, err, code) = run(r#"v=q; vx=VX; set -- "$v"'x'y; printf "[%s]\n" "$@""#);
    assert_eq!((out.as_str(), code), ("[qxy]\n", 0), "stderr={err:?}");
    // `$v$x` inside one region: the name is `v`, then `$x` starts a new arm.
    let (out, err, code) = run(r#"v=q; vx=VX; x=X; set -- "$v"$x; printf "[%s]\n" "$@""#);
    assert_eq!((out.as_str(), code), ("[qX]\n", 0), "stderr={err:?}");
    let (out, err, code) = run(r#"set -- A B; set -- "$1""2"; printf "[%s]\n" "$@""#);
    assert_eq!((out.as_str(), code), ("[A2]\n", 0), "stderr={err:?}");
    // The same shape with a space between the regions is two words in both.
    let (out, err, code) = run(r#"set -- A B; printf "[%s]" "$1" "2""#);
    assert_eq!((out.as_str(), code), ("[A][2]", 0), "stderr={err:?}");
}
