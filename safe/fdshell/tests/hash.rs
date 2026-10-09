#![allow(clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
const HELPER: &str = env!("EXEC_OK_PATH");

/// The test's helper dir first, then the ambient PATH (NixOS has no
/// /usr/bin), so ordinary externals like `echo` still resolve.
fn test_path() -> String {
    let path_dir = HELPER
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("/usr/bin");
    let ambient = std::env::var("PATH").unwrap_or_default();
    format!("{path_dir}:{ambient}")
}

fn run_with_path(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .env("PATH", test_path())
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

/// `hash name path` pins an entry; bare `hash` lists it.
#[test]
fn hash_pins_and_lists() {
    let (out, err, code) = run_with_path(&format!("hash exec_ok {HELPER}; hash"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == format!("exec_ok\t{HELPER}")),
        "pinned entry must be listed, stdout={out:?}"
    );
}

/// Running an external command stores the found path in the table.
#[test]
fn hash_auto_caches_path_lookup() {
    let (out, err, code) = run_with_path("exec_ok; hash");
    // `exec_ok` exits 42; the script continues to `hash` (exit 0).
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == format!("exec_ok\t{HELPER}")),
        "the PATH lookup must be cached, stdout={out:?}"
    );
}

/// A pin to a vanished path self-heals: the command still runs and the entry
/// is replaced with the fresh PATH result.
#[test]
fn hash_stale_pin_self_heals() {
    let (out, err, code) = run_with_path("hash exec_ok /nonexistent-hash-stale; exec_ok; hash");
    assert_eq!(
        code, 0,
        "the command must run despite the stale pin, stderr={err:?}"
    );
    assert!(
        out.lines().any(|l| l == format!("exec_ok\t{HELPER}")),
        "the stale pin must be replaced, stdout={out:?}"
    );
    assert!(!out.contains("/nonexistent-hash-stale"));
}

/// `hash -r` clears the whole table; `hash -r name` clears one entry.
#[test]
fn hash_dash_r_clears() {
    let (out, err, code) = run_with_path(&format!("hash exec_ok {HELPER}; hash -r; hash"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.is_empty(), "the table must be empty, stdout={out:?}");
    let (out, err, code) = run_with_path(&format!(
        "hash a {HELPER}; hash b {HELPER}; hash -r a; hash"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == format!("b\t{HELPER}")));
    assert!(!out.lines().any(|l| l.starts_with("a\t")));
}

/// `hash name` prints the entry, PATH-searching and storing on a miss.
#[test]
fn hash_lookup_prints_and_caches() {
    let (out, err, code) = run_with_path("hash exec_ok");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), HELPER);
    // A name in no table entry and not on PATH: clean error, exit 1.
    let (out, err, code) = run_with_path("hash no-such-command-xyzzy");
    assert_eq!(code, 1);
    assert!(err.contains("not found"), "stderr={err:?}");
    assert!(out.is_empty());
}

/// A pinned path wins over PATH order for the actual exec: two different
/// `exec_ok` binaries on PATH, the pin selects the second one.
#[test]
fn hash_pin_wins_over_path_order() {
    let dir = std::env::temp_dir().join(format!("fdshell-hash-pin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let b = dir.join("exec_ok");
    std::fs::write(&b, b"#!/bin/sh\necho from-B\nexit 7\n").unwrap();
    {
        let mut p = std::fs::metadata(&b).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&b, p).unwrap();
    }
    let helper_dir = HELPER.rsplit_once('/').unwrap().0;
    let output = Command::new(BIN)
        .args([
            "-c",
            &format!("hash exec_ok {}; exec_ok; echo done", b.display()),
        ])
        .env("PATH", format!("{helper_dir}:{}", test_path()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stdout.contains("from-B"),
        "the pinned binary must run, not the first PATH hit, stdout={stdout:?} stderr={stderr}"
    );
    assert!(stdout.contains("done"), "stdout={stdout:?} stderr={stderr}");
}

/// Builtins are never pre-hashed: running one must not add a table entry.
#[test]
fn hash_builtin_not_cached() {
    let (out, err, code) = run_with_path("pwd; hash");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().all(|l| !l.starts_with("pwd\t")),
        "the builtin pwd must not be cached, stdout={out:?}"
    );
}

/// A scratch `PATH` dir of executable files, so `hash`'s folded name matches a real
/// file (bash `hash a\*` finds the file named `a*`).
fn scratch_bin(names: &[&str]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fdshell-hash-esc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\necho ran\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn run_with_bin(bin_dir: &std::path::Path, script: &str) -> (String, String, i32) {
    let path = format!("{}:{}", bin_dir.display(), test_path());
    let output = Command::new(BIN)
        .env("PATH", path)
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

/// POSIX #4.1: `hash`'s name word is folded, so it matches the table's folded keys
/// (`prehash` stores the folded command word): `hash a\*` finds the `PATH` file
/// named `a*` and prints its path rc 0, and the pin form stores the folded key with
/// the folded path. A quoted name keeps the pair, so the literal `a\*` is not found
/// (rc 1, as bash reports `a\*: not found` for the quoted form).
#[test]
fn hash_name_folds_escape_pairs() {
    let bin = scratch_bin(&["a*"]);
    let want = format!("{}/a*", bin.display());
    let (out, err, code) = run_with_bin(&bin, "hash a\\*");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), want, "the folded name must find the file a*");
    // The pin form keys the table by the folded name and folds the path word.
    let (out, err, code) = run_with_bin(&bin, "hash a\\* /tmp/a\\*b; hash");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == "a*\t/tmp/a*b"),
        "the folded key and path must be stored, stdout={out:?}"
    );
    // A quoted name keeps its pair: the literal `a\*` is looked up, not found.
    let (out, err, code) = run_with_bin(&bin, "hash \"a\\*\"");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty());
    assert!(
        err.contains("a\\*"),
        "the raw name is reported, stderr={err:?}"
    );
    // fdshell's own surface (bash ignores `hash -r`'s name list): the removal matches
    // the folded keys, so `hash -r a\*` clears the entry stored under `a*`.
    let (out, err, code) = run_with_bin(&bin, &format!("hash a\\* {HELPER}; hash -r a\\*; hash"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.is_empty(),
        "the folded key must be removed, stdout={out:?}"
    );
    let _ = std::fs::remove_dir_all(&bin);
}
