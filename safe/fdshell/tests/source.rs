#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A script file in the temp dir, removed when the guard drops.
struct TempScript(String);

impl TempScript {
    fn new(name: &str, content: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("fdshell-it-source-{name}-{}", std::process::id()));
        std::fs::write(&path, content).unwrap();
        Self(path.to_str().unwrap().to_string())
    }
}

impl std::fmt::Display for TempScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Drop for TempScript {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn source_runs_file_in_current_shell() {
    let script = TempScript::new("vars", "A=hello\necho got-$A\n");
    let output = Command::new(BIN)
        .args(["-c", &format!("source {script}; echo after-$A")])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(
        stdout.lines().collect::<Vec<_>>().join("|"),
        "got-hello|after-hello",
        "stdout={stdout:?} stderr={:?}",
        str::from_utf8(&output.stderr).unwrap()
    );
}

#[test]
fn dot_command_sources_file() {
    let script = TempScript::new("dot", "B=world\n");
    let output = Command::new(BIN)
        .args(["-c", &format!(". {script}; echo $B")])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        "world",
        "stderr={:?}",
        str::from_utf8(&output.stderr).unwrap()
    );
}

#[test]
fn source_missing_file_fails() {
    let output = Command::new(BIN)
        .args(["-c", "source /nonexistent-source-it-xxxxxxxx"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "missing file must fail the script"
    );
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("failed to open file"), "stderr={stderr}");
}

#[test]
fn source_no_argument_fails() {
    let output = Command::new(BIN)
        .args(["-c", "source"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    assert!(!output.status.success(), "source without a file must fail");
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("missing file argument"), "stderr={stderr}");
}
