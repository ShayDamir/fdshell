#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A command substitution with an unbounded producer (`$(yes)`) must hit the
/// capture limit: the child is killed and a clean, actionable error is
/// reported instead of an out-of-memory kill or an infinite hang.
#[test]
fn cmd_subst_unbounded_output_is_capped() {
    let output = Command::new(BIN)
        .args(["-c", "echo $(yes)"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("capture limit"),
        "expected the capture-limit error, exit={:?} stderr={}",
        output.status.code(),
        stderr
    );
    assert!(
        !output.status.success(),
        "oversized command substitution should fail the command"
    );
}

/// A `)` inside double quotes in `$(…)` is data, not the substitution's
/// terminator. Without quote tracking the substitution ends at the first `)`
/// and the rest of the line is re-parsed as shell syntax (injection class).
#[test]
fn cmd_subst_paren_in_quotes_is_data() {
    let output = Command::new(BIN)
        .args(["-c", "x=$(echo \"a)b\"); echo got:$x"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        output.status.success(),
        "paren in quotes must not break the parse, stderr={stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "got:a)b",
        "quoted ) must stay inside the substitution, stdout={stdout:?}"
    );
}

/// Nested `$(…)` inside double quotes: the inner `)` is data for the outer
/// substitution and a real terminator for the inner one.
#[test]
fn cmd_subst_nested_with_quotes() {
    let output = Command::new(BIN)
        .args(["-c", "x=$(echo \"$(echo n)e\"); echo got:$x"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        output.status.success(),
        "nested substitution with quotes must succeed, stderr={stderr}"
    );
    assert_eq!(stdout.trim(), "got:ne");
}

/// A command substitution whose output is under the limit must still work and
/// strip trailing newlines (command substitution semantics).
#[test]
fn cmd_subst_small_output_strips_newlines() {
    let output = Command::new(BIN)
        .args(["-c", "echo [$(echo hello)]"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        "[hello]",
        "trailing newline must be stripped, stdout={stdout:?}"
    );
}

/// `set --stdout-capture-limit` lowers the cap: output above the new cap
/// fails with the same capture-limit error as the default cap.
#[test]
fn capture_limit_lowered_blocks_output() {
    let output = Command::new(BIN)
        .args([
            "-c",
            "set --stdout-capture-limit 5; echo [$(printf \"abcdefgh\")]",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("capture limit"),
        "8 bytes over a 5-byte cap must hit the limit, stderr={stderr}"
    );
    assert!(!output.status.success());
}

/// Output exactly at the new cap is allowed.
#[test]
fn capture_limit_at_cap_allows_output() {
    let output = Command::new(BIN)
        .args([
            "-c",
            "set --stdout-capture-limit 8; x=$(printf \"abcdefgh\"); echo [$x]",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(output.status.success(), "stderr={stderr}");
    assert_eq!(stdout.trim(), "[abcdefgh]");
}

/// A zero cap blocks any non-empty output but still allows empty output.
#[test]
fn capture_limit_zero_allows_empty_only() {
    let output = Command::new(BIN)
        .args(["-c", "set --stdout-capture-limit 0; x=$(true); echo ok"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(output.status.success(), "stderr={stderr}");
    assert_eq!(stdout.trim(), "ok");
}

/// Missing or non-numeric values are clean errors, not an external `set`
/// fall-through.
#[test]
fn capture_limit_bad_value_is_an_error() {
    for script in [
        "set --stdout-capture-limit",
        "set --stdout-capture-limit abc",
    ] {
        let output = Command::new(BIN)
            .args(["-c", script])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap();

        let stderr = str::from_utf8(&output.stderr).unwrap();
        assert!(
            stderr.contains("not a byte count"),
            "{script:?} must report the bad value, stderr={stderr}"
        );
        assert!(!output.status.success());
    }
}

/// `fdshell -c script` as `(stdout, stderr, exit code)`.
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

/// A `$( … )`, `${ … }` or `` ` … ` `` region inside double quotes is scanned as
/// one unit (task #130): the region's own `"` do not close the outer quote, and
/// its `;`, `&&`, `|`, `>` and `<<` are data, never statement/cond/redirect
/// separators. Every row is asserted against bash's own output, with empty
/// stderr.
#[test]
fn quoted_region_separators_are_data() {
    let cases: &[(&str, &str)] = &[
        // A `;` inside the region: the statement is not split there.
        ("echo \"$(echo \"a; b\")\"", "a; b\n"),
        // The `&&` inside the region is not a cond-list operator.
        ("echo \"$(printf \"x && y\")\" && echo ok", "x && y\nok\n"),
        // The `|` inside the region is not a pipeline.
        ("echo \"$(printf \"a | b\")\"", "a | b\n"),
        // The `>` inside the region opens no redirect.
        ("echo \"$(printf \"a > /dev/null\")\"", "a > /dev/null\n"),
        // The `<<` inside the region opens no heredoc.
        ("echo \"$(printf \"<<EOF\")\"", "<<EOF\n"),
        // A brace expansion inside the region is the child's own text.
        ("echo \"$(echo {a,b})\"", "a b\n"),
        // Region-in-region, and the same with an operator after the region.
        ("echo \"$(echo \"$(echo deep)\")\"", "deep\n"),
        (
            "echo \"$(echo \"$(printf \"x && y\")\")\" && echo ok",
            "x && y\nok\n",
        ),
        // Text around the region stays part of the same word.
        ("echo a\"$(echo \"x y\")\"", "ax y\n"),
        ("echo \"a$(printf \" b\")c\"", "a bc\n"),
        ("echo a\"$(echo \"x y\")\"z", "ax yz\n"),
        // A `${ … }` region behaves the same way.
        ("echo \"${u:-\"a; b\"}\"", "a; b\n"),
        ("echo \"${u:-\"a && b\"}\" && echo ok", "a && b\nok\n"),
        // Blocks: the region is one word of the condition's command.
        (
            "if echo \"$(echo \"a; b\")\"; then echo yes; fi",
            "a; b\nyes\n",
        ),
        (
            "if echo \"${u:-\"a && b\"}\"; then echo yes; fi",
            "a && b\nyes\n",
        ),
        ("while echo \"$(echo \"x; y\")\"; do break; done", "x; y\n"),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

/// The region's bytes are marked quoted, so its output is one word and is not
/// split on IFS. The value is read with `printf "[%s]"`.
#[test]
fn quoted_region_expansion_is_one_word() {
    let cases: &[(&str, &str)] = &[
        ("builtin printf \"[%s]\" \"$(printf \"a b\")\"", "[a b]"),
        ("builtin printf \"[%s]\" a\"$(echo \"x y\")\"", "[ax y]"),
        ("builtin printf \"[%s]\" \"a$(printf \" b\")c\"", "[a bc]"),
        ("builtin printf \"[%s]\" \"${u:-\"z w\"}\"", "[z w]"),
        (
            "builtin printf \"[%s]\" \"$(echo \"$(printf \"p q\")\")\"",
            "[p q]",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

/// A region's output keeps a heredoc structure inside it intact: the multi-line
/// `$( … )` body with a heredoc is the child's own text.
#[test]
fn quoted_region_keeps_a_heredoc_inside_its_body() {
    let (out, err, code) = run("echo \"$(cat <<EOF\nx; y\nEOF\n)\"");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "x; y\n");
}

/// A quoted backtick region is scanned as one unit *and* expanded (task #216):
/// the arm runs under the region's all-quoted mask, so its output is one word
/// and `IFS` never applies. Measured against `bash -c` 5.3.9: `hi`, `[a b]`.
///
/// Accepted divergence (README "Nested regions inside double quotes"): the
/// region scanner drops the quote bytes written *inside* a quoted body, exactly
/// as it does in a `${ … }` body, so the child re-parses the body without them.
/// `printf` shows it — bash runs `` `printf "q r"` `` with one argument,
/// fdshell's child sees `printf q r` and prints `q`. `echo` hides the same
/// difference because it rejoins its arguments with a space.
#[test]
fn quoted_backtick_region_expands_as_one_word() {
    let cases: &[(&str, &str)] = &[
        ("echo \"`echo hi`\"", "hi\n"),
        ("builtin printf \"[%s]\" \"`echo a b`\"", "[a b]"),
        ("echo \"`echo a b`\"", "a b\n"),
        ("echo \"`printf x; echo; printf y`\"", "x\ny\n"),
        ("echo \"`echo hi`x\"", "hix\n"),
        ("echo x\"`echo hi`\"", "xhi\n"),
        ("echo \"`printf \"q r\"`\"", "q\n"),
        ("builtin printf \"[%s]\" \"`printf \"q r\"`\"", "[q]"),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

/// An unterminated region is a clean parse error with the caret on the *outer*
/// quote (never a panic, never a hang): the enclosing quote stays open and the
/// report is the unbalanced quote, exactly as an unterminated plain quote.
#[test]
fn unterminated_quoted_region_is_a_clean_error() {
    for script in [
        "echo \"$(echo a\"",
        "echo \"${u:-x",
        "echo \"`a",
        "echo \"$(echo \"a)",
    ] {
        let (_out, err, code) = run(script);
        assert_ne!(code, 0, "{script:?} must fail");
        assert!(!err.is_empty(), "{script:?} must report the error");
        assert!(
            !err.contains("panicked"),
            "{script:?} must not panic, stderr={err}"
        );
    }
}

// ---- word 0: the command-name position (task #268) --------------------

use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};

static WORD0_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// Scratch dir for the word-0 glob pins: an executable `ea` that echoes its
/// own argv, plus `eb`, `ed` (glob fillers) and `zz` (a plain argument).
fn word0_scratch() -> (String, Scratch) {
    let c = WORD0_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-word0-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    let ea = dir.join("ea");
    std::fs::write(&ea, "#!/bin/sh\necho ran \"$@\"\n").unwrap();
    std::fs::set_permissions(&ea, std::fs::Permissions::from_mode(0o755)).unwrap();
    for name in ["eb", "ed", "zz"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    (dir.to_str().unwrap().to_string(), Scratch(dir))
}

/// Run with the child's cwd inside `dir`, so relative word-0 patterns resolve
/// against the scratch layout.
fn run_in(dir: &str, script: &str) -> (String, String, i32) {
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

/// Word 0 expands exactly like an argument: the whole-word form names the
/// command, a mid-word expansion concatenates with the literal bytes, and the
/// double-quoted form expands too (bash's `"` never suppresses command
/// substitution). Single quotes are the only suppression, so `'$( … )'` stays
/// the literal command name — bash reports the same name (rc 127, fdshell rc 1).
#[test]
fn word0_expansion_names_the_command() {
    for script in [
        "$(echo echo) hi",
        "$(echo \"ec\")ho hi",
        "\"$(echo echo)\" hi",
    ] {
        let (out, err, rc) = run(script);
        assert_eq!(rc, 0, "{script:?} must run echo, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(out, "hi\n", "{script:?}");
    }
    let (_out, err, rc) = run("'$(echo echo)' hi");
    assert_ne!(rc, 0, "single quotes must keep word 0 literal");
    assert!(err.contains("$(echo echo)"), "{err}");
}

/// A multi-field expansion in word 0 makes field 0 the command and fields 1..
/// the leading arguments, before the word's own args. Measured against bash:
/// `hi x`.
#[test]
fn word0_multi_field_expansion_leads_the_arguments() {
    let (out, err, rc) = run("$(echo echo hi) x");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "hi x\n");
}

/// An unquoted word 0 whose expansion is empty is not a word: the next field
/// becomes the command. A *quoted* empty expansion is a word, so it is the
/// (empty) command name and the arguments stay put; with nothing left to
/// consume, the command name is empty. fdshell reports the empty name as a
/// missing command (bash prints `: command not found`).
#[test]
fn word0_empty_expansion_consumes_the_next_field() {
    let (out, err, rc) = run("$(true) echo hi");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
    for script in ["\"$(true)\" echo hi", "$(echo )"] {
        let (_out, err, rc) = run(script);
        assert_ne!(rc, 0, "{script:?} must have an empty command name");
        assert!(
            err.contains("\"\""),
            "{script:?} must name the empty word, {err}"
        );
    }
}

/// A fully quoted whole-word `$@` with no positionals is **no word at all**
/// (task #219, bash parity: `"$@"` *is* the positional word list), so word 0
/// is empty and the next word becomes the command — the same shape as the
/// unquoted empty expansion above. Measured against bash: `hi`.
#[test]
fn empty_dollar_at_in_word_zero_leaves_the_next_word_as_command() {
    let (out, err, rc) = run(r#"set --; "$@" echo hi"#);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "hi\n");
}

/// The residual divergence of that shape: with nothing left to consume, the
/// command name is empty and fdshell reports it (rc 1 in the report, the
/// script continues at rc 0), where bash runs no command at all and prints
/// nothing. Pinned like the `$(echo )` pins above: the message, not the path.
#[test]
fn empty_dollar_at_alone_in_word_zero_reports_the_empty_name() {
    let (out, err, rc) = run(r#"set --; "$@"; echo after"#);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "after\n");
    assert!(err.contains("\"\""), "must name the empty word, {err}");
}

/// Word 0's expansion is split on the shell's `IFS` like any other word:
/// `IFS=x` splits `echoxhi` into the command `echo` and the argument `hi`,
/// which keeps the following word. Measured against bash.
#[test]
fn word0_ifs_splitting_applies() {
    let (out, err, rc) = run("IFS=x; $(echo \"echoxhi\") zz");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi zz\n");
}

/// `$name` and `$(( … ))` are expansion triggers in word 0 as well: an unset
/// variable yields the empty name (so the next field is consumed), `$((1+1))`
/// looks up the command `2`. bash reports the same names, rc 127 there.
#[test]
fn word0_parameter_and_arithmetic_expansion() {
    let (out, err, rc) = run("cmd=echo; $cmd hi");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
    let (_out, err, rc) = run("$((1+1))");
    assert_ne!(rc, 0, "the command name is the arithmetic value");
    assert!(err.contains("\"2\""), "{err}");
    let (_out, err, rc) = run("$missingvar $((0+1))");
    assert_ne!(
        rc, 0,
        "an unset variable is the empty name, so `1` is consumed"
    );
    assert!(err.contains("\"1\""), "{err}");
}

/// Escape pairs fold in word 0 (POSIX #4.1): `e\cho` is the command `echo`.
/// Inside double quotes the pair survives as the literal name (POSIX #4.2),
/// and `$`/`` ` `` survive as literal bytes. bash runs `echo`; the quoted
/// variant is a command that does not exist there too.
#[test]
fn word0_escape_pairs_fold_and_quotes_protect() {
    let (out, err, rc) = run(r"e\cho hi");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
    let (_out, err, rc) = run(r#""e\cho""#);
    assert_ne!(rc, 0, "a quoted pair stays in the command name");
    assert!(err.contains(r"e\cho"), "{err}");
    let (_out, err, rc) = run(r"$(echo '$HOME') x");
    assert_ne!(
        rc, 0,
        "the child's $HOME must not be re-expanded in the parent"
    );
    assert!(err.contains("$HOME"), "{err}");
}

/// A leading `~` in word 0 is expanded to `$HOME` (the arg pipeline's tilde
/// rule applies to the command word). The name reported must be the expanded
/// path, never the literal `~/…`.
#[test]
fn word0_tilde_expands() {
    let (_out, err, rc) = run("~/fdshell-word0-missing");
    assert_ne!(rc, 0, "the command does not exist");
    assert!(err.contains("/fdshell-word0-missing"), "{err}");
    assert!(!err.contains("~/"), "the tilde must be expanded, {err}");
}

/// Globbing applies to the expanded word: the first field of the match list is
/// the command and the rest are its leading arguments. Measured against bash
/// (`ran ./eb ./ed zz`) in the same scratch layout.
#[test]
fn word0_glob_of_the_expansion_first_field_is_the_command() {
    let (dir, _scratch) = word0_scratch();
    let (out, err, rc) = run_in(&dir, "$(echo ./e)* zz");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "ran ./eb ./ed zz\n");
    let (out, err, rc) = run_in(&dir, "./e* zz");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "ran ./eb ./ed zz\n");
}

/// With `failglob` a word-0 pattern that matches nothing is a clean,
/// actionable error (no `no match`, no hang): the shell reports the pattern.
#[test]
fn word0_failglob_reports_a_clean_error() {
    let (dir, _scratch) = word0_scratch();
    let (_out, err, rc) = run_in(&dir, "shopt -s failglob; nope* echo");
    assert_ne!(rc, 0, "failglob must fail the command");
    assert!(err.contains("no match"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
    // Without failglob the pattern stays literal and the lookup fails.
    let (_out, err, rc) = run_in(&dir, "nope* echo");
    assert_ne!(rc, 0, "an unmatched literal pattern is a missing command");
    assert!(err.contains("nope*"), "{err}");
}

/// Every pipeline stage expands its own word 0, and a `set -e` prefix is not
/// triggered by the substitution child's failure (the child's status is never
/// the command's status).
#[test]
fn word0_pipeline_stages_and_set_e() {
    let (out, err, rc) = run("$(echo echo) hi | cat");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
    let (out, err, rc) = run("set -e; $(false) echo hi");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
    // A scoped assignment prefix reaches the command found through word 0.
    let (out, err, rc) = run("X=1 $(echo echo) hi");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
}

/// An unbounded producer in word 0 (`$(yes)`) is capped by the capture limit:
/// the shell reports the actionable error and exits non-zero, in time.
#[test]
fn word0_unbounded_output_is_capped() {
    let (_out, err, rc) = run("$(yes)");
    assert_ne!(rc, 0, "an unbounded word-0 substitution must fail");
    assert!(err.contains("capture limit"), "{err}");
}

/// A chain of nested substitutions in word 0 is **bounded**: 100 levels run the
/// innermost command and report its name (`deep`, bash's `deep: command not
/// found` at rc 127 → fdshell rc 1), and one level more stops at the shell-wide
/// nesting cap with a *fixed* report count and the empty command name — the
/// chain never grows with the script, never hangs, never panics.
#[test]
fn word0_deep_nesting_is_bounded() {
    for (depth, name) in [(100, "deep"), (101, "")] {
        let script = format!("{}echo deep{}", "$(".repeat(depth), ")".repeat(depth));
        let (out, err, rc) = run(&script);
        assert_ne!(rc, 0, "depth {depth} must fail the command, stdout={out}");
        assert_eq!(out, "", "depth {depth} must print nothing");
        assert!(
            !err.contains("panicked"),
            "depth {depth} must not panic, stderr={err}"
        );
        assert!(
            err.contains(&format!("\"{name}\"")),
            "depth {depth} must name {name:?}, stderr={err}"
        );
        // 100 report blocks of 5 lines; the cap keeps the count fixed.
        assert!(
            err.lines().count() <= 520,
            "depth {depth} must stay bounded, got {} lines",
            err.lines().count()
        );
    }
}

/// The nesting cap around a word-0 expansion (README "Nesting is capped at 100
/// groups"): 99 nested `if` blocks run the expanded command exactly as bash
/// does, the 100th makes the expansion report `nesting too deep` at rc 1 (bash
/// has no cap and runs `hi`).
#[test]
fn word0_nesting_cap_reports_the_message() {
    for (depth, want_hi) in [(99usize, true), (100, false)] {
        let script = format!(
            "{} $(echo echo) hi{}",
            "if true; then ".repeat(depth),
            "; fi".repeat(depth)
        );
        let (out, err, rc) = run(&script);
        if want_hi {
            assert_eq!(rc, 0, "depth {depth} must run, stderr={err}");
            assert_eq!(out, "hi\n", "depth {depth}");
        } else {
            assert_ne!(rc, 0, "depth {depth} must hit the nesting cap");
            assert_eq!(out, "", "depth {depth} must print nothing");
            assert!(
                err.contains("nesting too deep"),
                "depth {depth} must report the cap, stderr={err}"
            );
        }
    }
}

/// `exec` and `become` command words are word-0 positions
/// (`replacer/external.rs` runs `command_word::fields` on them): the expansion
/// names the program, its extra fields are the leading argv, and the command
/// word is globbed exactly as a plain command word is. Measured against
/// `bash -c` for `exec`; bash has no `become`, and fdshell's `become` was
/// measured equal to fdshell's `exec` on every spelling here.
#[test]
fn word0_exec_and_become_expand_their_command_word() {
    let (dir, _scratch) = word0_scratch();
    for (script, want) in [
        ("exec $(echo echo) hi", "hi\n"),
        ("exec ./e* zz", "ran ./eb ./ed zz\n"),
        ("become $(echo echo) hi", "hi\n"),
        ("become ./e* zz", "ran ./eb ./ed zz\n"),
    ] {
        let (out, err, rc) = run_in(&dir, script);
        assert_eq!(rc, 0, "{script:?} must run, stderr={err}");
        assert_eq!(out, want, "{script:?}");
    }
    for prefix in ["exec", "become"] {
        let script = format!("{prefix} $(echo pwd)");
        let (out, err, rc) = run_in(&dir, &script);
        assert_eq!(rc, 0, "{script:?} must run pwd, stderr={err}");
        assert_eq!(out, format!("{dir}\n"), "{script:?} prints the cwd");
    }
}

/// `timeout`'s target word is a word-0 position: the sub-command line is built
/// from the raw target word and expanded by `command_word::expand` before it is
/// launched, so `timeout 1 $(echo echo) hi` prints `hi` (bash: coreutils
/// `timeout` runs `echo hi`).
#[test]
fn word0_timeout_target_word_expands() {
    let (out, err, rc) = run("timeout 1 $(echo echo) hi");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "", "stderr={err}");
    assert_eq!(out, "hi\n");
}

/// The `builtin` and `command` prefixes see the expanded word 0 of the command
/// they prefix: both print `hi` (bash parity).
#[test]
fn word0_builtin_and_command_prefixes_see_the_expansion() {
    for script in ["builtin $(echo echo) hi", "command $(echo echo) hi"] {
        let (out, err, rc) = run(script);
        assert_eq!(rc, 0, "{script:?} must run the expanded echo, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(out, "hi\n", "{script:?}");
    }
}

/// Function lookup runs on the expanded word 0: the expanded name finds the
/// function and its body runs (bash parity).
#[test]
fn word0_function_lookup_uses_the_expanded_name() {
    let (out, err, rc) = run("foo(){ builtin echo IN-FOO; }; $(echo foo) x");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "", "stderr={err}");
    assert_eq!(out, "IN-FOO\n");
}

/// The in-process builtins are dispatched on the expanded name: `cd` moves the
/// shell's cwd, and `exit` ends the shell with its argument's status (bash
/// parity: `cd` prints the new pwd, `exit 7` exits 7).
#[test]
fn word0_in_process_builtins_see_the_expanded_name() {
    let (dir, _scratch) = word0_scratch();
    let (out, err, rc) = run(&format!("$(echo cd) {dir}; pwd"));
    assert_eq!(rc, 0, "the expanded cd must move the shell, stderr={err}");
    assert_eq!(out, format!("{dir}\n"), "the shell's cwd must be {dir}");
    let (_out, _err, rc) = run("$(echo exit) 7");
    assert_eq!(rc, 7, "the expanded exit must end the shell with 7");
}

/// The glob rules of the expansion, pinned as words in the scratch layout
/// (`ea` executable, `eb`, `ed`, `zz`): a `nullglob` pattern that matches
/// nothing makes the word vanish, so the *next* field is the command (bash
/// names `zz`); a matched one takes its first match as the command name (bash
/// names `ea`); `set -f` disables the glob and leaves the literal name; and a
/// newline in the expansion is an `IFS` delimiter.
#[test]
fn word0_glob_rules_apply_as_words() {
    let (dir, _scratch) = word0_scratch();
    for (script, name) in [
        (r#"shopt -s nullglob; $(echo "nope*") zz"#, "zz"),
        (r#"shopt -s nullglob; $(echo "e*") zz"#, "ea"),
        (r"set -f; echo* zz", "echo*"),
    ] {
        let (_out, err, rc) = run_in(&dir, script);
        assert_ne!(
            rc, 0,
            "{script:?} must reach the command lookup, stderr={err}"
        );
        assert!(err.contains(name), "{script:?} must name {name:?}, {err}");
    }
    let (out, err, rc) = run_in(&dir, "set -f; $(echo echo) hi");
    assert_eq!(rc, 0, "set -f must not stop the expansion, stderr={err}");
    assert_eq!(out, "hi\n");
    let (out, err, rc) = run_in(&dir, r#"$(printf "echo\nhi") zz"#);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi zz\n", "newline splits the expansion");
}

/// Every statement path expands its own word 0: an `if` condition, a loop body,
/// a group, a function body, `eval`, `time`, a pipeline stage, and a nested
/// substitution — whose child expands *its own* word 0 too. Measured against
/// `bash -c`, including the two nested forms.
#[test]
fn word0_expands_in_every_context() {
    let cases: &[(&str, &str)] = &[
        ("if $(echo true); then echo yes; fi", "yes\n"),
        ("for i in 1; do $(echo echo) in; done", "in\n"),
        ("while $(echo false); do echo no; done", ""),
        ("{ $(echo echo) grp; }", "grp\n"),
        ("f(){ $(echo echo) inbody; }; f", "inbody\n"),
        ("eval '$(echo echo) ev'", "ev\n"),
        ("$(echo echo) hi | cat", "hi\n"),
        ("echo $( $(echo echo) hi )", "hi\n"),
        ("echo `$(echo echo) hi`", "hi\n"),
    ];
    for (script, want) in cases {
        let (out, err, rc) = run(script);
        assert_eq!(rc, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
    // `time` prints its own report on stderr: pin the command's stdout only.
    let (out, _err, rc) = run("time $(echo echo) tm");
    assert_eq!(rc, 0);
    assert_eq!(out, "tm\n");
}

/// Accepted divergence, pinned by name (LESSONS "word 0 is a substitution
/// surface" item 8, README "The command word (word 0)"): a scoped `NAME=value`
/// prefix is applied *around* the word-0 expansion, so a scoped `IFS` splits
/// word 0 — `IFS=: $(printf "echo:hi") zz` runs `echo hi zz` — where bash
/// splits the expansion with the shell's `IFS`, keeps `echo:hi` as one field,
/// and reports the name `echo:hi` (rc 127, fdshell rc 1).
#[test]
fn word0_scoped_ifs_divergence() {
    let (out, err, rc) = run(r#"IFS=: $(printf "echo:hi") zz"#);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi zz\n", "the scoped IFS splits word 0");
    let (out, err, rc) = run(r#"IFS=:; $(printf "echo:hi") zz"#);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi zz\n", "the shell's own IFS=: splits it too, {out}");
}

/// Word-0 leading fields are ordinary argv words, so they take the global argv
/// `%fd` substitution rule (README "Passing file descriptors to subprocess";
/// the literal form is `\%var`): with `%s` bound by `signalfd`,
/// `signalfd %s 10; w="%s"; $(echo ./ea; echo "$w")` runs `./ea` with the fd
/// *number*, where bash (no fd vars) passes the text `%s`.
#[test]
fn word0_leading_fields_take_the_argv_fd_substitution() {
    let (dir, _scratch) = word0_scratch();
    let (out, err, rc) = run_in(&dir, r#"signalfd %s 10; w="%s"; $(echo ./ea; echo "$w")"#);
    assert_eq!(rc, 0, "stderr={err}");
    let fields = out.trim_end().strip_prefix("ran ").unwrap_or_default();
    assert!(
        !fields.is_empty() && fields.chars().all(|c| c.is_ascii_digit()),
        "the leading field must be substituted with the fd number, stdout={out:?}"
    );
}

/// Task #277: a `$( … )` region is one word for every scanner, so a body that
/// starts with a space is an ordinary command word in word 0 — bash's own
/// behavior. Before the fix the byte-level word split stopped inside the region,
/// the leading `$(` read as the definer of the function `$`, and the statement
/// died as a bare `parse error` rc 1. Every row measured against `bash -c` 5.3.9.
#[test]
fn word0_substitution_body_starting_with_a_space_runs() {
    for script in [
        "$( echo true)",
        "$( echo true )",
        "$(  echo true )",
        "$( echo true ) zz",
        "$( echo true ) &",
        "true && $( echo true )",
        "$( echo true ) && $( echo true )",
        // The spellings that already worked: pinned so the fix cannot regress them.
        "$(echo true)",
        "$(echo true )",
    ] {
        let (out, err, rc) = run(script);
        assert_eq!(rc, 0, "{script:?} must run, stderr={err}");
        assert_eq!(out, "", "{script:?}");
    }
}

/// A multi-word region body expands to the command name *and* leaves the rest of
/// the line as its arguments: `$( echo echo ) hi` prints `hi` in both shells.
#[test]
fn word0_substitution_body_starting_with_a_space_multi_word_body() {
    for (script, want) in [
        ("$( echo echo ) hi", "hi\n"),
        ("$( echo echo) hi", "hi\n"),
        ("$(echo echo ) hi", "hi\n"),
        ("$(echo echo) hi", "hi\n"),
        ("$( echo echo ) $( echo hi )", "hi\n"),
        ("echo a; $( echo echo ) b", "a\nb\n"),
    ] {
        let (out, err, rc) = run(script);
        assert_eq!(rc, 0, "{script:?} must run, stderr={err}");
        assert_eq!(out, want, "{script:?}");
    }
}

/// The inner substitution names the command that is missing: `$( $(echo echo) hi )`
/// runs `hi`. fdshell's command-not-found status is the shell-wide 1, not bash's
/// 127 (README "Words and quoting").
#[test]
fn word0_substitution_of_a_substitution_reports_the_inner_command() {
    let (_out, err, rc) = run("$( $(echo echo) hi )");
    assert_eq!(rc, 1);
    assert!(err.contains("hi"), "stderr must name `hi`, got {err}");
    assert!(
        !err.contains("parse error"),
        "no bare parse error, stderr={err}"
    );
}

/// The assignment-value position is the same word walk: a space-opened body in
/// `x=$( … )` assigns, and two regions in one value concatenate.
#[test]
fn assignment_value_substitution_body_starting_with_a_space_assigns() {
    for (script, want) in [
        (r#"x=$( echo -n ab ); echo "[$x]""#, "[ab]\n"),
        (r#"x=$( echo a )$( echo b ); echo "[$x]""#, "[ab]\n"),
        (r#"x=$( echo "a  b" ); echo "[$x]""#, "[a  b]\n"),
        ("x=$( echo true )", ""),
    ] {
        let (out, err, rc) = run(script);
        assert_eq!(rc, 0, "{script:?} must run, stderr={err}");
        assert_eq!(out, want, "{script:?}");
    }
}

/// The space-opened body reaches every statement shape: a pipeline stage, a
/// redirect target, a group. All rc 0, as in bash.
#[test]
fn word0_substitution_with_a_space_body_in_a_pipeline_and_background() {
    let (out, err, rc) = run("echo $( echo hi ) | cat");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
    let (dir, _scratch) = word0_scratch();
    // Word 0 expands to `true`, so the statement runs the builtin with its
    // stdout redirected: the file exists (bash: the same, rc 0).
    let (out, err, rc) = run_in(
        &dir,
        "$( echo true ) > out277; test -f out277 && echo written",
    );
    assert_eq!((out.as_str(), rc), ("written\n", 0), "stderr={err}");
}

/// The block/statement paths in the space-opened spelling, pinned so the fix
/// cannot "help" them by accident (plan §5). The function-body row is exactly the
/// accidental help: master answered the bare `parse error` rc 1 there, the fix
/// runs the body at rc 0 — bash's own answer. Every row measured against
/// `bash -c` 5.3.9 on this branch.
#[test]
fn space_body_substitution_runs_in_a_function_body_and_block_statements() {
    let cases: &[(&str, &str)] = &[
        ("f() { $( echo true ); }; f", ""),
        ("f() { echo $( echo hi ); }; f", "hi\n"),
        ("if $( echo true ); then echo yes; fi", "yes\n"),
        (
            "for i in $( echo a ) $( echo b ); do echo \"$i\"; done",
            "a\nb\n",
        ),
    ];
    for (script, want) in cases {
        let (out, err, rc) = run(script);
        assert_eq!(rc, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(err, "", "{script:?} must report nothing, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

/// Pinned as-is, the task that owns it filed: a `$( … )` word in **definer**
/// position (before a `{` body) is not a function definition, so fdshell runs
/// the words (`hi`, `}` — not found, rc 1) where bash rejects the line at parse
/// time with a syntax error rc 2. The definer gate now rejects every word that
/// carries a substitution region, exactly as the tokenizer's word split does.
#[test]
fn definer_position_substitution_word_is_not_a_definition() {
    for script in ["$( echo hi ) { :; }", "x$( echo hi ) { :; }", "$() { :; }"] {
        let (_out, err, rc) = run(script);
        assert_eq!(rc, 1, "{script:?}, stderr={err}");
        assert!(
            err.contains("not found"),
            "{script:?} must look the words up, stderr={err}"
        );
        assert!(
            !err.contains("parse error"),
            "{script:?} is not a parse error, stderr={err}"
        );
    }
}

/// An unterminated region is the tokenizer's clean report, not the statement
/// layer's: `unexpected end of input` with the caret, rc 1 (bash rc 2). The
/// byte-level gate must claim the rest of the word behind it, never pair it with
/// a `{`.
#[test]
fn unterminated_substitution_reports_cleanly() {
    for script in ["$( echo true", "$( echo true && echo b"] {
        let (_out, err, rc) = run(script);
        assert_eq!(rc, 1, "{script:?}");
        assert!(
            err.contains("unexpected end of input"),
            "{script:?} must report the EOF, stderr={err}"
        );
        assert!(
            !err.contains("is missing its closing"),
            "{script:?} must not report an unclosed block, stderr={err}"
        );
    }
}

/// Word 0 and the redirect target are the **same** substitution surface: one
/// script expands both positions (task #268 built the pipeline, task #278 gave
/// it to the target), so the command name comes from word 0 and the file name
/// from the target. Measured against bash 5.3.9, which writes `hi` into `f`.
#[test]
fn word0_and_the_redirect_target_expand_in_one_command() {
    let (dir, _scratch) = word0_scratch();
    let (out, err, rc) = run_in(&dir, "$(echo echo) hi > $(echo f); cat f");
    assert_eq!((out.as_str(), rc), ("hi\n", 0), "stderr={err}");

    // The quoted-target rule is word 0's rule too: `"` does not suppress the
    // region, `'` does, so the literal name becomes the file.
    let (out, err, rc) = run_in(&dir, "echo hi > '$(echo f)'");
    assert_eq!((out.as_str(), rc), ("", 0), "stderr={err}");
    assert!(
        std::path::Path::new(&dir).join("$(echo f)").exists(),
        "the single-quoted target is the literal name"
    );

    // A target expanding to two fields is bash's `ambiguous redirect` (rc 1),
    // while word 0 takes the first field and keeps the rest as arguments.
    let (out, err, rc) = run_in(&dir, "echo hi > $(echo a; echo b)");
    assert_eq!((out.as_str(), rc), ("", 1), "stderr={err}");
    assert!(err.contains("ambiguous redirect"), "stderr={err}");
}
