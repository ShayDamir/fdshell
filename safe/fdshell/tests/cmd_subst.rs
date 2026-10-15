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

/// Scratch dir for the word-0 glob pins: an executable `ea` that echoes its
/// own argv, plus `eb`, `ed` (glob fillers) and `zz` (a plain argument).
fn word0_scratch() -> String {
    let c = WORD0_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-word0-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    let ea = dir.join("ea");
    std::fs::write(&ea, "#!/bin/sh\necho ran \"$@\"\n").unwrap();
    std::fs::set_permissions(&ea, std::fs::Permissions::from_mode(0o755)).unwrap();
    for name in ["eb", "ed", "zz"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    dir.to_str().unwrap().to_string()
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
    let dir = word0_scratch();
    let (out, err, rc) = run_in(&dir, "$(echo ./e)* zz");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "ran ./eb ./ed zz\n");
    let (out, err, rc) = run_in(&dir, "./e* zz");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(out, "ran ./eb ./ed zz\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// With `failglob` a word-0 pattern that matches nothing is a clean,
/// actionable error (no `no match`, no hang): the shell reports the pattern.
#[test]
fn word0_failglob_reports_a_clean_error() {
    let dir = word0_scratch();
    let (_out, err, rc) = run_in(&dir, "shopt -s failglob; nope* echo");
    assert_ne!(rc, 0, "failglob must fail the command");
    assert!(err.contains("no match"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
    // Without failglob the pattern stays literal and the lookup fails.
    let (_out, err, rc) = run_in(&dir, "nope* echo");
    assert_ne!(rc, 0, "an unmatched literal pattern is a missing command");
    assert!(err.contains("nope*"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
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
