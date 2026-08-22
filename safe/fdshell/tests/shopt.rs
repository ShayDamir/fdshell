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
    let path = std::env::temp_dir().join(format!("shopt_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

#[test]
fn noclobber_blocks_overwrite() {
    let path = temp_path("noclobber");
    std::fs::write(&path, b"keep\n").unwrap();
    let (out, err, code) = run(&format!("set -o noclobber; echo hi >{path}"));
    let _ = std::fs::remove_file(&path);
    assert_ne!(code, 0);
    assert!(err.contains("noclobber"), "stderr={err:?}");
    assert!(out.is_empty(), "stdout={out:?}");
}

#[test]
fn noclobber_allows_new_files() {
    let path = temp_path("newfile");
    let _ = std::fs::remove_file(&path);
    let (_out, err, code) = run(&format!("set -o noclobber; echo hi >{path}"));
    let content = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(content.unwrap(), "hi\n");
}

#[test]
fn shopt_u_reenables_overwrite_in_same_shell() {
    let path = temp_path("toggle");
    std::fs::write(&path, b"old\n").unwrap();
    let (_out, err, code) = run(&format!(
        "shopt -s noclobber; shopt -u noclobber; echo new >{path}"
    ));
    let content = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(content.unwrap(), "new\n");
}

#[test]
fn shopt_query_exit_codes() {
    let (_out, _err, code) = run("shopt -s noclobber; shopt -q noclobber");
    assert_eq!(code, 0);
    let (_out, _err, code) = run("shopt -q noclobber");
    assert_eq!(code, 1);
}

#[test]
fn expand_aliases_on_by_default() {
    let (_out, _err, code) = run("shopt -q expand_aliases");
    assert_eq!(code, 0);
    let (_out, _err, code) = run("shopt -u expand_aliases; shopt -q expand_aliases");
    assert_eq!(code, 1);
}

#[test]
fn set_dash_o_toggles_option() {
    let (_out, _err, code) = run("set -o noclobber; shopt -q noclobber");
    assert_eq!(code, 0);
    let (_out, _err, code) = run("set +o noclobber; shopt -q noclobber");
    assert_eq!(code, 1);
}

#[test]
fn unknown_option_fails_actionably() {
    let (_out, err, code) = run("set -o bogus_option");
    assert_ne!(code, 0);
    assert!(err.contains("bogus_option"), "stderr={err:?}");
    let (_out, err, code) = run("shopt -s bogus_option");
    assert_ne!(code, 0);
    assert!(err.contains("bogus_option"), "stderr={err:?}");
}

#[test]
fn shopt_lists_options() {
    let (out, err, code) = run("shopt");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("noclobber off"), "stdout={out:?}");
    assert!(out.contains("expand_aliases on"), "stdout={out:?}");
}

#[test]
fn noclobber_readonly_dir_is_open_error() {
    let dir = std::env::temp_dir().join(format!("shopt_ro_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ro = std::os::unix::fs::PermissionsExt::from_mode(0o555);
    std::fs::set_permissions(&dir, ro).unwrap();
    let target_path = dir.join("newfile");
    let target = target_path.to_str().unwrap();
    let (_out, err, code) = run(&format!("set -o noclobber; echo hi >{target}"));
    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_ne!(code, 0);
    assert!(!err.contains("noclobber"), "stderr={err:?}");
    assert!(err.contains("open"), "stderr={err:?}");
}

#[test]
fn shopt_unknown_flag_fails() {
    let (_out, err, code) = run("shopt -z noclobber");
    assert_ne!(code, 0);
    assert!(err.contains("-z"), "stderr={err:?}");
}

#[test]
fn set_dash_o_lists_options() {
    let (out, err, code) = run("set -o noclobber; set -o");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("noclobber on"), "stdout={out:?}");
    assert!(out.contains("expand_aliases on"), "stdout={out:?}");
}
