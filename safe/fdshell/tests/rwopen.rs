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

fn temp_path(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("rwopen_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

#[test]
fn rw_redirect_reads_existing_content() {
    let path = temp_path("read");
    std::fs::write(&path, b"abc").unwrap();
    let (out, err, code) = run(&format!("exec 3<>{path}; cat <&3; exec 3>&-"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "abc");
}

#[test]
fn rw_redirect_keeps_content_and_shares_file_offset() {
    let path = temp_path("offset");
    std::fs::write(&path, b"abc").unwrap();
    let (out, err, code) = run(&format!(
        "exec 2<>{path}; echo d >&2; cat <&2; exec 2>&-; cat {path}"
    ));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    // `echo d` overwrites "ab" with "d\n" (file is now "d\nc");
    // `cat <&2` resumes at offset 2 ("c"), then `cat` prints the whole file.
    assert_eq!(out, "cd\nc");
}

#[test]
fn rw_redirect_creates_missing_file() {
    let path = temp_path("create");
    let _ = std::fs::remove_file(&path);
    let (_out, err, code) = run(&format!("exec 3<>{path}; exec 3>&-"));
    let exists = std::path::Path::new(&path).exists();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(exists);
}
