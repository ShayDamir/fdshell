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

/// A simple quoted backtick substitution expands to its child's output.
/// Measured against `bash -c` 5.3.9: `hi`.
#[test]
fn backtick_simple_quoted_expands() {
    let (out, err, rc) = run(r#"echo "`echo hi`""#);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "hi\n");
}

/// A mid-word backtick with spaces splits its output into separate words.
/// Measured against `bash -c` 5.3.9: `x a b y` (four words).
#[test]
fn backtick_mid_word_with_spaces_splits() {
    let (out, err, rc) = run(r#"echo x `printf "a b"` y"#);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "x a b y\n");
}

/// A backtick sitting inside a `$( … )` body runs in the child.
/// Measured against `bash -c` 5.3.9: `a`.
#[test]
fn backtick_inside_dollar_paren() {
    let (out, err, rc) = run("echo \"$(echo `echo a`)\"");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "a\n");
}

/// A backtick wrapping a `$( … )` runs the inner substitution as the child's text.
/// Measured against `bash -c` 5.3.9: empty (the child runs `a`, missing).
#[test]
fn dollar_paren_inside_backtick() {
    let (out, _err, rc) = run(r#"echo "`$(echo a)`""#);
    assert_eq!(rc, 0);
    assert_eq!(out, "\n");
}

/// Nesting in the other order, unquoted: `$(echo `echo a`)` → `a`.
#[test]
fn backtick_nested_inside_dollar_paren_wrapping() {
    let (out, err, rc) = run("echo $(echo `echo a`)");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "a\n");
}

/// An unquoted-delimiter heredoc runs backtick expansion in its body.
#[test]
fn backtick_in_heredoc_unquoted_delimiter() {
    let script = "cat <<EOF\n`echo hi` $(echo yo)\nEOF";
    let (out, err, rc) = run(script);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "hi yo\n");
}

/// A quoted-delimiter heredoc keeps a backtick body literal.
#[test]
fn backtick_in_heredoc_quoted_delimiter() {
    let script = "cat <<'EOF'\n`echo hi`\nEOF";
    let (out, err, rc) = run(script);
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "`echo hi`\n");
}

/// An unterminated backtick region is a clean parse error, not a hang.
#[test]
fn backtick_unterminated_is_clean_error() {
    let (out, err, rc) = run(r#"echo "`echo hi"#);
    assert_ne!(rc, 0, "unterminated backtick must fail");
    assert!(
        err.contains("unmatched quote") || err.contains("syntax error"),
        "{err}"
    );
    assert!(out.is_empty() || !out.contains("hi"), "{out}");
}

/// Word 0 is a substitution surface (task #268), so the backtick form expands
/// there too: `` `echo echo` hi `` runs `echo echo`, its output field `echo` is
/// the command name and `hi` stays the argument. The old literal-word-0
/// divergence is gone. Measured against bash 5.3.9, which prints `hi` for the
/// first case and reports the *output* name `hi` for the second (rc 127 there,
/// fdshell reports rc 1).
#[test]
fn backtick_word0_expands() {
    let (out, err, rc) = run("`echo echo` hi");
    assert_eq!(rc, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "hi\n");

    let (_out, err, rc) = run("`echo hi`");
    assert_ne!(rc, 0, "the output field `hi` is the command name");
    assert!(err.contains("\"hi\""), "{err}");
}

/// A backtick substitution with an unbounded producer hits the capture limit.
#[test]
fn backtick_unbounded_output_is_capped() {
    let output = Command::new(BIN)
        .args(["-c", "echo `yes`"])
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

/// A per-test scratch dir (pid + counter), removed on drop — the house pattern
/// (LESSONS, task #255): pids are recycled, so the dir must die with its
/// process, and removal is `.ok()`-tolerant so teardown never panics.
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

/// A scratch dir for the target pins, with the child's cwd inside it.
fn scratch() -> (String, Scratch) {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-backtick-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.to_str().unwrap().to_string(), Scratch(dir))
}

/// Run with the cwd inside `dir`, so a relative target lands in the scratch.
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

/// A backtick region in a redirect **target** expands (task #278): bash
/// creates `f` for `` > `echo f` `` and `ab` for the mid-word form; the
/// single-quoted region is the only suppression, so `'`echo f`'` names the
/// literal file. The target is the second surface of word 0's pipeline.
#[test]
fn backtick_in_a_redirect_target_expands() {
    let (dir, _scratch) = scratch();
    for (script, want) in [
        ("echo hi > `echo f`", "f"),
        ("echo hi > a`echo b`", "ab"),
        ("echo hi > \"`echo f`\"", "f"),
        ("echo hi > '`echo f`'", "`echo f`"),
    ] {
        let (out, err, code) = run_in(&dir, script);
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err}");
        assert!(
            std::path::Path::new(&dir).join(want).exists(),
            "{script}: bash creates `{want}`"
        );
    }
}
