#![allow(clippy::unwrap_used)]
//! Single-quote spans (task #141): a `'…'` span strips both quote bytes and is
//! one literal word — everywhere, in every code path.
//!
//! Each expectation below is the recorded output of `bash` (5.x on this
//! machine) for the same script, in the comment above the assertion.
//! Divergences are marked as such:
//! * `$'…'` is read as the POSIX-sh expansion, so `echo $'x'` is `$x` and not
//!   bash's `x` — tracked by task #156.
//! * `case` patterns ignore the quote mask (both quote kinds), tracked by
//!   task #246 — pinned nowhere in this file.
//! * An unbalanced quote is a parse error with fdshell's own message and exit
//!   code 1; bash prints its own syntax error with code 2. The pre-existing
//!   double-quote case behaves identically, so `'` follows the shell's
//!   convention (see STYLE.md §4).

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

/// A scratch dir holding `names` as empty files; `run_in` runs there so globs
/// and redirects see exactly these files.
fn scratch(names: &[&str]) -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-quote-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    dir
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
fn scratch_bin(names: &[&str]) -> std::path::PathBuf {
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

/// Divergence (task #247): a quoted `if`/`while`/`for`/`until`/`case` at the
/// start of a statement is still parsed as the keyword, so it is a syntax
/// error; bash runs it as a command word (POSIX §2.9.1 recognizes a reserved
/// word only when it is unquoted). This is the pre-existing double-quote
/// behavior — task #141 made `'…'` behave identically, which is the point of
/// that task; the bash-parity fix is #247. Pinned so #247's fix is visible.
#[test]
fn quoted_block_openers_are_still_keywords() {
    for (script, needle) in [
        ("'if' x", "missing 'fi'"),
        ("\"if\" x", "missing 'fi'"),
        ("'while' x", "expected 'done'"),
        ("'for' x", "expected 'in'"),
        ("'case' x", "missing 'in'"),
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: stderr={err:?}");
        assert!(err.contains(needle), "{script}: stderr={err:?}");
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
}

/// Task #223: when the byte right after `$` starts a new quote region the
/// `$` is a literal byte and nothing is consumed — the region cap on the name
/// scan has no name to read. bash's `printf "[%s]" "$"v` prints `[$v]`, so
/// this is parity, not a divergence; it is the same rule `$'…'` and `$"…"`
/// already follow (see `dollar_quote_keeps_the_dollar`).
#[test]
fn dollar_at_a_region_end_is_a_literal_byte() {
    let (out, err, code) = run(r#"v=q; printf "[%s]" "$"v"#);
    assert_eq!((out.as_str(), code), ("[$v]", 0), "stderr={err:?}");
    let (out, err, code) = run(r#"v=q; echo a"$"v"#);
    assert_eq!((out.as_str(), code), ("a$v\n", 0), "stderr={err:?}");
    // The word is still an expansion (an arm ran), so the splitter runs on
    // it: fdshell cuts `[$v][a]` where bash keeps `[$v,a]` whole — the
    // "an arm ran" reading of `expanded`, task #133's rule (task #222 owns
    // the literal-delimiter join).
    let (out, err, code) = run(r#"IFS=,; v=q; printf "[%s]" "$"v,a"#);
    assert_eq!((out.as_str(), code), ("[$v][a]", 0), "stderr={err:?}");
}

/// Accepted divergence (task #285): the quote mask is one bit per byte, so two
/// *adjacent* quoted regions read as one region and a bare name keeps
/// collecting across the boundary. bash ends the name at the closing quote of
/// the region that holds the `$`: it prints `[qb]` and `[A2]`, fdshell `[VB]`
/// and `[]`. Pinning today's bytes gives #285 the expectations to delete.
#[test]
fn adjacent_quoted_regions_keep_collecting_the_name() {
    let (out, err, code) = run(r#"v=q; vb=VB; printf "[%s]" "$v""b""#);
    assert_eq!((out.as_str(), code), ("[VB]", 0), "stderr={err:?}");
    let (out, err, code) = run(r#"set -- A B; printf "[%s]" "$1""2""#);
    assert_eq!((out.as_str(), code), ("[]", 0), "stderr={err:?}");
    // The single-quote spelling of the same adjacency: two quoted regions,
    // and the name crosses the boundary into `vx`.
    let (out, err, code) = run(r#"v=q; vx=VX; printf "[%s]" "$v"'x'y"#);
    assert_eq!((out.as_str(), code), ("[VXy]", 0), "stderr={err:?}");
    // The quoted↔unquoted boundary *is* expressible in the mask, so it is
    // parity: `"$v"x` is the parameter `v` plus the literal `x` in both.
    let (out, err, code) = run(r#"v=q; vx=VX; printf "[%s]" "$v"x"#);
    assert_eq!((out.as_str(), code), ("[qx]", 0), "stderr={err:?}");
    // The same shape with a space between the regions is two words in both.
    let (out, err, code) = run(r#"set -- A B; printf "[%s]" "$1" "2""#);
    assert_eq!((out.as_str(), code), ("[A][2]", 0), "stderr={err:?}");
}
