#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

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

/// A file in the temp dir, removed when the guard drops.
struct TempFile(String);

impl TempFile {
    fn new(name: &str, content: &str) -> Self {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "fdshell-it-positional-{name}-{}",
            std::process::id()
        ));
        std::fs::write(&path, content).unwrap();
        Self(path.to_str().unwrap().to_string())
    }

    fn path(&self) -> &str {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Symptom A of task #134: the `set --` words are `$1..$N` in order (bash:
/// `[a][b][c][3][a b c][a b c]`).
#[test]
fn set_dash_dash_positionals_are_one_based() {
    let (out, err, code) = run(r#"set -- a b c; echo "[$1][$2][$3][$#][$@][$*]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a][b][c][3][a b c][a b c]\n");
}

/// Symptom B of task #134: a function call rebinds `$1..` only, so its name
/// never reaches `$#`, `$@`, `$*` (bash: `a b` / `2` / `a` / `b`).
#[test]
fn function_call_does_not_expose_its_name() {
    let (out, err, code) = run(r#"f() { echo "$@"; echo $#; echo "$1"; echo "$2"; }; f a b"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a b\n2\na\nb\n");
}

/// Concatenation makes every `±1` of the index map observable. The braced
/// form is a documented divergence (task #177), so the words run bare.
#[test]
fn positional_concatenation_is_in_order() {
    let (out, err, code) = run(r#"set -- a b c; echo "[$1$2$3]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[abc]\n");
}

/// `shift` moves `$1..` and never `$0` (bash: `[bash][2][b][c]`).
#[test]
fn shift_keeps_program_name_and_rebases_positionals() {
    let (out, err, code) = run(r#"set -- a b c; shift; echo "[$0][$#][$1][$2]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[sh][2][b][c]\n");
}

/// `set -u` reports an out-of-range `$N` and names the index (bash:
/// `4: unbound variable`). fdshell's `-c` mode exits 1 where bash exits 127
/// (LESSONS.md accepts the rc asymmetry).
#[test]
fn nounset_out_of_range_positional_bails() {
    let (out, err, code) = run(r#"set -- a; set -u; echo "${4}""#);
    assert_eq!(out, "");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("4: unbound variable"),
        "the report must name the index, got {err:?}"
    );
}

/// `$0` is always bound, so `set -u` never reports it (bash prints it).
#[test]
fn nounset_zero_is_always_bound() {
    let (out, err, code) = run(r#"set -u; echo "[$0]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[sh]\n");
}

/// bash: `source f x y` gives the extras as `$1`/`$2`, keeps `$0`, and the
/// caller's positionals are restored when the source returns.
#[test]
fn source_extras_are_positionals_and_the_callers_survive() {
    let file = TempFile::new("source", "S1=$1\nS2=$2\nS0=$0\n");
    let script = format!(
        "set -- A B; source {} x y; echo \"[$S1][$S2][$S0][$1][$2]\"",
        file.path()
    );
    let (out, err, code) = run(&script);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[x][y][sh][A][B]\n");
}

/// Script mode seeds `$0` with the script path (argv[1]) and `$1..` with the
/// words after it — the only shape that reaches the seed pop in `main.rs`.
/// Measured against `bash <path> a b`, which prints the same line.
#[test]
fn script_mode_zero_is_the_script_path() {
    let file = TempFile::new("script", "echo \"[$0][$1][$2][$#]\"\n");
    let output = Command::new(BIN)
        .arg(file.path())
        .args(["a", "b"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let out = str::from_utf8(&output.stdout).unwrap().to_string();
    let err = str::from_utf8(&output.stderr).unwrap().to_string();
    assert_eq!(output.status.code().unwrap_or(-1), 0, "stderr={err:?}");
    assert_eq!(out, format!("[{}][a][b][2]\n", file.path()));
}

/// `--fd` mode has no path argument, so the seed must not pop: `$0` keeps the
/// invocation name and the words after `--fd N` are `$1..`.
#[test]
fn fd_mode_keeps_the_default_program_name() {
    let mut child = Command::new(BIN)
        .args(["--fd", "0", "a", "b"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"echo \"[$0][$1][$#]\"\n")
        .unwrap();
    child.stdin.take().unwrap();
    let output = child.wait_with_output().unwrap();
    let out = str::from_utf8(&output.stdout).unwrap().to_string();
    let err = str::from_utf8(&output.stderr).unwrap().to_string();
    assert_eq!(output.status.code().unwrap_or(-1), 0, "stderr={err:?}");
    assert_eq!(out, "[fdshell][a][2]\n");
}
