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

/// A quoted backtick region is scanned as one unit too: one word, exit 0, and
/// the text stays literal (fdshell does not expand backticks at all — task #216
/// owns the expansion; #130 only makes the scanning unit-sized).
#[test]
fn quoted_backtick_region_is_one_literal_word() {
    let cases: &[(&str, &str)] = &[
        ("echo \"`echo hi`\"", "`echo hi`\n"),
        ("echo \"`printf \"q r\"`\"", "`printf q r`\n"),
        (
            "builtin printf \"[%s]\" \"`printf \"q r\"`\"",
            "[`printf q r`]",
        ),
        ("echo \"`printf \"a; b\"`\"", "`printf a; b`\n"),
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
