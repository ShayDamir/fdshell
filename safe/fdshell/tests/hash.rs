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

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// A pid + counter scratch dir `fdshell-hash-<tag>-<pid>-<counter>`, removed
/// when the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-hash-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// A pinned path wins over PATH order for the actual exec: two different
/// `exec_ok` binaries on PATH, the pin selects the second one.
#[test]
fn hash_pin_wins_over_path_order() {
    let dir = scratch("pin");
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
fn scratch_bin(names: &[&str]) -> Scratch {
    let dir = scratch("esc");
    for name in names {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\necho ran\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn run_with_bin(bin_dir: &std::path::Path, script: &str) -> (String, String, i32) {
    run_in(bin_dir, &std::env::current_dir().unwrap(), script)
}

/// `fdshell -c` with `bin_dir` first on `PATH`, run in `cwd` — the glob rows
/// need a scratch CWD, never the repository's (LESSONS #255/#342).
fn run_in(bin_dir: &std::path::Path, cwd: &std::path::Path, script: &str) -> (String, String, i32) {
    let path = format!("{}:{}", bin_dir.display(), test_path());
    let output = Command::new(BIN)
        .env("PATH", path)
        .current_dir(cwd)
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

/// A scratch CWD holding `files`, so a glob row has matches to find.
fn scratch_with(tag: &str, files: &[&str]) -> Scratch {
    let dir = scratch(tag);
    for name in files {
        std::fs::write(dir.join(name), "x\n").unwrap();
    }
    dir
}

/// POSIX #4.1: `hash`'s name word folds its escape pair **inside** the word
/// expansion (task #279 moved the surface from a bare fold to `substitute_args`),
/// so it matches the table's keys (`prehash` stores the expanded command word):
/// `hash a\*` finds the `PATH` file named `a*` and prints its path rc 0, and the
/// pin form stores the folded key with the folded path. A quoted name keeps the
/// pair, so the literal `a\*` is not found (rc 1, as bash reports `a\*: not
/// found` for the quoted form).
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
}

/// The folded `*` is mask-protected, so a name word with an escape pair never
/// globs even in a directory full of matches (bash measures the same): `hash a\*`
/// in a dir with `aa ab` looks up the name `a*`, finds the `PATH` file `a*`, and
/// prints its path rc 0.
#[test]
fn hash_folded_name_never_globs() {
    let bin = scratch_bin(&["a*"]);
    let cwd = scratch_with("folded-glob", &["aa", "ab"]);
    let (out, err, code) = run_in(&bin, &cwd, "hash a\\*; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out.lines().next(),
        Some(format!("{}/a*", bin.display()).as_str()),
        "the folded name is the lookup key, stdout={out:?} stderr={err:?}"
    );
    assert!(
        !out.contains("aa\n") && !out.contains("ab\n"),
        "no glob fields, stdout={out:?}"
    );
}

/// Task #279: every name word goes through the argument pipeline, so an
/// expansion names the command (bash parity): `$( … )`, backticks, `$name` and
/// `$(( … ))` expand, a double-quoted expansion still expands, and single
/// quotes are the only suppression.
#[test]
fn hash_name_words_expand() {
    for spelling in [
        "$(echo exec_ok)",
        "`echo exec_ok`",
        "\"$(echo exec_ok)\"",
        "$n",
    ] {
        let (out, err, code) = run_with_path(&format!("n=exec_ok; hash {spelling}; hash"));
        assert_eq!(code, 0, "{spelling}: stderr={err:?}");
        assert!(
            out.lines().any(|l| l == format!("exec_ok\t{HELPER}")),
            "{spelling}: the expanded name must be the key, stdout={out:?}"
        );
    }
    // `$(( … ))` names the command too: the name `2`, bash's `not found` rc 1.
    let (out, err, code) = run_with_path("hash $((1+1)); echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        err.contains("hash: 2: not found"),
        "the expanded name is reported, stderr={err:?}"
    );
    assert_eq!(out.trim(), "rc=1", "stdout={out:?}");
    // Single quotes suppress: the literal name is looked up, rc 1.
    let (out, err, code) = run_with_path("hash '$(echo exec_ok)'; echo rc=$?; hash");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        err.contains("hash: $(echo exec_ok): not found"),
        "stderr={err:?}"
    );
    assert_eq!(out.lines().next(), Some("rc=1"), "stdout={out:?}");
    assert!(
        !out.contains(&format!("exec_ok\t{HELPER}")),
        "the quoted name stores nothing, stdout={out:?}"
    );
}

/// A name word is pathname-expanded with the shell's own glob policy (bash
/// parity, measured): `hash *` names every match, an unmatched pattern passes
/// through, `set -f` passes the pattern verbatim, `nullglob` yields 0 fields
/// (the bare listing), `failglob` reports `no match: PATTERN` and — fdshell's
/// rule for a failed expansion — aborts the run (task #179).
#[test]
fn hash_name_words_glob() {
    let bin = scratch_bin(&["exec_ok"]);
    let cwd = scratch_with("glob", &["aa", "ab"]);
    // bash hashes each field and reports `aa: not found`, `ab: not found` rc 1;
    // fdshell's 2-field pin form owns the pair (accepted divergence, #375), and
    // the pair is the *glob fields*, not the pattern.
    let (out, err, code) = run_in(&bin, &cwd, "hash *; hash");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == "aa\tab"),
        "the glob fields are the pin pair, stdout={out:?}"
    );
    assert!(
        !out.lines().any(|l| l.starts_with("*\t")),
        "the literal pattern is not a key, stdout={out:?}"
    );
    // No match: the default policy passes the pattern word through.
    let (out, err, code) = run_in(&bin, &cwd, "hash zzz*; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("hash: zzz*: not found"), "stderr={err:?}");
    assert_eq!(out.trim(), "rc=1", "stdout={out:?}");
    // `set -f`: no pathname expansion at all.
    let (out, err, code) = run_in(&bin, &cwd, "set -f; hash *; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("hash: *: not found"), "stderr={err:?}");
    assert_eq!(out.trim(), "rc=1", "stdout={out:?}");
    // `nullglob`: 0 fields is the bare listing, rc 0.
    let (out, err, code) = run_in(&bin, &cwd, "shopt -s nullglob; hash zzz*; echo rc=$?; hash");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("rc=0"), "stdout={out:?}");
    assert!(
        !out.lines().any(|l| l.starts_with("zzz*")),
        "0 fields stores nothing, stdout={out:?}"
    );
    // `failglob`: bash's report, and the run aborts (fdshell's #179 rule).
    let (out, err, code) = run_in(&bin, &cwd, "shopt -s failglob; hash zzz*; echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("no match: zzz*"), "stderr={err:?}");
    assert!(
        !out.contains("after"),
        "the failed expansion aborts the run, stdout={out:?}"
    );
}

/// The name words split on `IFS` like any argument, and 0 fields is the bare
/// listing: `hash $@` with no positionals, an unset `$zz`, `hash $(true)`.
#[test]
fn hash_name_words_split_and_vanish() {
    // One word, two fields: fdshell's pin form owns the pair (#375).
    let (out, err, code) = run_with_path("n=\"x y\"; hash $n; hash");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == "x\ty"),
        "the split fields are the pin pair, stdout={out:?}"
    );
    // 0 fields: the bare listing, rc 0, no key written.
    for spelling in ["$@", "$zz", "$(true)"] {
        let (out, err, code) = run_with_path(&format!("hash {spelling}; echo rc=$?; hash"));
        assert_eq!(code, 0, "{spelling}: stderr={err:?}");
        assert!(out.contains("rc=0"), "{spelling}: stdout={out:?}");
        assert!(
            !out.contains(spelling),
            "{spelling}: 0 fields must store nothing, stdout={out:?}"
        );
    }
    // The positionals expand into the name words.
    let (out, err, code) = run_with_path(&format!("set -- exec_ok {HELPER}; hash $@; hash"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == format!("exec_ok\t{HELPER}")),
        "the positional pair must be stored, stdout={out:?}"
    );
}

/// The option word is read **after** the expansion: `hash $(echo -r)` clears
/// the table, and fdshell's `-r NAME…` / `-d NAME…` removals take the expanded
/// names.
#[test]
fn hash_options_are_parsed_after_expansion() {
    let (out, err, code) = run_with_path(&format!("hash exec_ok {HELPER}; hash $(echo -r); hash"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        !out.lines().any(|l| l.starts_with("exec_ok\t")),
        "the expanded `-r` must clear the table, stdout={out:?}"
    );
    // fdshell's extension: `hash -r name…` removes the expanded name only.
    let (out, err, code) = run_with_path(&format!(
        "hash a {HELPER}; hash b {HELPER}; hash -r $(echo a); hash"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        !out.lines().any(|l| l.starts_with("a\t")),
        "the expanded name must be removed, stdout={out:?}"
    );
    assert!(
        out.lines().any(|l| l.starts_with("b\t")),
        "the other entry stays, stdout={out:?}"
    );
}

/// `hash -d name…` (bash's delete, absent from fdshell before task #279): a
/// present name is removed silently rc 0, a missing name reports bash's
/// `hash: NAME: not found` rc 1, and a bare `hash -d` is fdshell's usage error
/// (bash's getopt diagnostic, `-d: option requires an argument`, rc 1).
#[test]
fn hash_dash_d_deletes_entries() {
    let (out, err, code) = run_with_path(&format!(
        "hash exec_ok {HELPER}; hash -d $(echo exec_ok); echo rc=$?; hash"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("rc=0"), "stdout={out:?}");
    assert!(
        !out.lines().any(|l| l.starts_with("exec_ok\t")),
        "the expanded name must be deleted, stdout={out:?}"
    );
    let (out, err, code) = run_with_path("hash -d nosuch; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("hash: nosuch: not found"), "stderr={err:?}");
    assert_eq!(out.trim(), "rc=1", "stdout={out:?}");
    // A name list: every present name goes, any missing one sets rc 1.
    let (out, err, code) = run_with_path(&format!(
        "hash a {HELPER}; hash b {HELPER}; hash -d a nosuch; echo rc=$?; hash"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("hash: nosuch: not found"), "stderr={err:?}");
    assert_eq!(out.lines().next(), Some("rc=1"), "stdout={out:?}");
    assert!(
        !out.lines().any(|l| l.starts_with("a\t")),
        "the present name must be deleted, stdout={out:?}"
    );
    assert!(
        out.lines().any(|l| l.starts_with("b\t")),
        "the untouched entry stays, stdout={out:?}"
    );
    // No operand: fdshell's usage error, which aborts the run (task #179).
    let (out, err, code) = run_with_path("hash -d; echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("bad usage"), "stderr={err:?}");
    assert!(
        !out.contains("after"),
        "the usage error aborts the run, stdout={out:?}"
    );
}

/// fdshell's pin form takes the **expanded** path word too (its extension over
/// bash's `-p`), and a third field stays the usage error (bash hashes each
/// name; the option/list surface is task #375).
#[test]
fn hash_pin_path_word_expands() {
    let (out, err, code) = run_with_path(&format!("hash exec_ok $(echo {HELPER}); hash"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == format!("exec_ok\t{HELPER}")),
        "the expanded path must be stored, stdout={out:?}"
    );
    // Three fields: bash hashes each name, fdshell reports its usage error.
    let (out, err, code) = run_with_path("hash a b c; echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("bad usage"), "stderr={err:?}");
    assert!(
        !out.contains("after"),
        "the usage error aborts the run, stdout={out:?}"
    );
}

/// A leading `~` expands in a name word. bash accepts a name with a slash
/// silently and stores nothing; fdshell reports its own `not found` — an
/// accepted divergence, owned by task #375.
#[test]
fn hash_name_word_expands_tilde() {
    let home = std::env::var("HOME").unwrap();
    let (out, err, code) = run_with_path("hash ~/no-such-dir; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        err.contains(&format!("hash: {home}/no-such-dir: not found")),
        "the expanded name is reported, stderr={err:?}"
    );
    assert_eq!(out.trim(), "rc=1", "stdout={out:?}");
}

/// An unbound fd name stays literal in a name word, exactly as in `echo a%b`,
/// so `hash a%b` looks up the literal name (bash parity).
#[test]
fn hash_percent_name_stays_literal() {
    let bin = scratch_bin(&["exec_ok"]);
    let cwd = scratch_with("percent", &["aa"]);
    let (out, err, code) = run_in(&bin, &cwd, "hash a%b; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("hash: a%b: not found"), "stderr={err:?}");
    assert_eq!(out.trim(), "rc=1", "stdout={out:?}");
}

/// Strict mode's absolute ban runs on the **expanded** pin path, not the word
/// as written (LESSONS #278 rule 4): a path the expansion makes absolute is
/// rejected when the pinned command runs, a relative one runs.
#[test]
fn hash_pin_path_is_banned_absolute_only_after_expansion() {
    let dir = scratch_with("strict-pin", &[]);
    let program = dir.join("fdprobe");
    std::fs::write(&program, "#!/bin/sh\necho ran-probe\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let target = dir.join("fdtarget");
    std::fs::write(&target, "#!/bin/sh\necho ran-target\n").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
    let run = |script: &str| {
        let output = Command::new(BIN)
            .env("PATH", ".")
            .current_dir(&*dir)
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
    };
    // The expansion (here `$PWD`, with no subprocess to run under `PATH=.`)
    // makes the path absolute, and strict mode must see that.
    let (out, err, code) = run("shopt -s strict; hash fdprobe $PWD/fdtarget; fdprobe");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(
        err.contains("forbids absolute program paths"),
        "the ban runs on the expanded path, stderr={err:?}"
    );
    assert!(!out.contains("ran-target"), "stdout={out:?}");
    // The same pin with a relative path runs the pinned target.
    let (out, err, code) = run("shopt -s strict; hash fdprobe fdtarget; fdprobe");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), "ran-target", "stdout={out:?}");
}
