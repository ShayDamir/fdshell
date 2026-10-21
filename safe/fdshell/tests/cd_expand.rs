#![allow(clippy::unwrap_used)]

//! `cd`'s operand is an ordinary word (task #284): it goes through the
//! argument pipeline — tilde, `$name`, `$( … )`, backticks, `$(( … ))`, IFS
//! splitting, pathname expansion — and the target is decided on the expanded
//! fields. Every expectation below was measured against `bash -c` (5.3.9) in
//! the same scratch layout before being pinned; the divergences are named in
//! the assertions that carry them.

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
const HELPER: &str = env!("EXEC_OK_PATH");

/// The test's helper dir first, then the ambient `PATH` (NixOS has no
/// /usr/bin), so ordinary externals like `echo` still resolve.
fn test_path() -> String {
    let path_dir = HELPER
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("/usr/bin");
    let ambient = std::env::var("PATH").unwrap_or_default();
    format!("{path_dir}:{ambient}")
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop: pids are recycled,
/// so a dir that outlives its process is a later run's inherited dir with that
/// run's files in it (LESSONS, task #255; sweep task #342). Removal is
/// `.ok()`-tolerant, so teardown never panics.
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

fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-cd-expand-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// The scratch CWD of the task's measurement table: dirs `sub`, `2`, `a b` and
/// a directory literally named `-`, files `aa` and `ab`, so every expansion and
/// glob row has a real filesystem to resolve against.
fn workdir(tag: &str) -> Scratch {
    let dir = scratch(tag);
    for name in ["sub", "2", "a b", "-"] {
        std::fs::create_dir(dir.join(name)).unwrap();
    }
    std::fs::write(dir.join("sub/marker"), b"x\n").unwrap();
    for name in ["aa", "ab"] {
        std::fs::write(dir.join(name), b"x\n").unwrap();
    }
    dir
}

/// Run `fdshell -c` in `cwd` with `HOME` pointed at `home`, so the `~` and
/// bare-`cd` rows never read the developer's home.
fn run_in(cwd: &std::path::Path, home: &std::path::Path, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .env("PATH", test_path())
        .env("HOME", home)
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

/// `run_in` with the child's environment spelled out pair by pair (the shape of
/// `tests/quotes.rs::run_env`, task #282): a `"\0"` value **removes** the key,
/// the only way to give the child no `HOME` at all, since the harness runs with
/// one. Task #287's rows need both an inherited `HOME` and no `HOME` at all.
fn run_env(cwd: &std::path::Path, env: &[(&str, &str)], script: &str) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(["-c", script])
        .current_dir(cwd)
        .env("PATH", test_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        if *value == "\0" {
            cmd.env_remove(key);
        } else {
            cmd.env(key, value);
        }
    }
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Every expansion family reaches the directory: `$name`, `$( … )`, backticks,
/// `$(( … ))`, `"$@"` and a function parameter. bash cds to `<w>/sub` for each.
#[test]
fn cd_operand_expands() {
    let work = workdir("expand");
    let home = scratch("expand-home");
    for script in [
        "V=sub; cd $V; builtin pwd",
        "cd \"$(echo sub)\"; builtin pwd",
        "cd `echo sub`; builtin pwd",
        "f(){ cd \"$1\"; builtin pwd; }; f sub",
    ] {
        let (out, err, code) = run_in(&work, &home, script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert_eq!(
            out.trim(),
            format!("{}/sub", work.display()),
            "script {script:?} must cd into sub"
        );
    }
    // The arithmetic form: a directory named `2`.
    let (out, err, code) = run_in(&work, &home, "cd $((1+1)); builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/2", work.display()));
    // Positionals: `"$@"` is the positional word list, one field.
    let (out, err, code) = run_in(&work, &home, "set -- sub; cd \"$@\"; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/sub", work.display()));
}

/// Tilde and the 0-field forms are the bare `cd`, i.e. `$HOME` (bash: the same
/// for `cd ~`, `cd "$@"` with an empty deque, `cd $unset`, `cd $(true)`).
#[test]
fn cd_zero_fields_and_tilde_are_home() {
    let work = workdir("home");
    let home = scratch("home-home");
    for script in [
        "cd ~; builtin pwd",
        "set --; cd \"$@\"; builtin pwd",
        "cd $unset_var; builtin pwd",
        "cd $(true); builtin pwd",
    ] {
        let (out, err, code) = run_in(&work, &home, script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert_eq!(
            out.trim(),
            home.display().to_string(),
            "script {script:?} must cd into HOME"
        );
    }
}

/// The bare `cd` reads `HOME` through the **shell variable** — the shell's own
/// variables first, the environment fdshell started with as the fallback — i.e.
/// the same lookup `$HOME` and `~` use (task #287, LESSONS #282: one fact, one
/// lookup). Every bash column was measured in `bash -c` (5.3.9) with the child's
/// inherited `HOME` = `S1` and a second scratch dir `S2`:
///
/// ```text
/// HOME=S2; cd; pwd                       bash S2        fdshell before S1
/// HOME=S2; export HOME; cd; pwd          bash S2        fdshell before S1
/// HOME=; cd; pwd                         bash <cwd>, rc 0 (a silent no-op)
/// cd; pwd            (env HOME=S1)       bash S1
/// cd; pwd            (no HOME in env)    bash rc 1, `cd: HOME not set`
/// HOME=S2 cd; pwd; echo $HOME            bash S2 then S1
/// f(){ local HOME=S2; cd; pwd; }; f      bash S2        fdshell before S1
/// ```
///
/// The inherited-only row (`cd; pwd` → `S1`) is the keep-green
/// `cd_literal_rows_stay_green` and `cd_zero_fields_and_tilde_are_home` pair,
/// which run through `run_in`. The plan's `unset HOME` probe of that fallback
/// is **not expressible** here: fdshell's `unset` is fd-only, so `unset HOME`
/// is the parse error `variable must start with '%'` (measured, rc 1), and
/// `HOME` cannot be taken away from a running shell at all — the fallback row
/// is pinned by an inherited-only `HOME` (and, in the unit file, by clearing
/// `state.environ`), never by an unassignment.
#[test]
fn cd_home_is_the_shell_variable() {
    let work = workdir("homevar");
    let s1 = scratch("homevar-s1");
    let s2 = scratch("homevar-s2");
    let inherited = s1.display().to_string();
    let assigned = s2.display().to_string();
    let env: [(&str, &str); 1] = [("HOME", &inherited)];

    // The shell assignment wins over the inherited row, plain and through
    // `export` (which goes through the same `set_var` entry).
    for script in [
        format!("HOME={assigned}; cd; builtin pwd"),
        format!("HOME={assigned}; export HOME; cd; builtin pwd"),
    ] {
        let (out, err, code) = run_env(&work, &env, &script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert_eq!(
            out.trim(),
            assigned,
            "script {script:?} must cd into the shell variable's HOME"
        );
    }

    // An **empty** `HOME` is a value, so the target is the empty path: the open
    // failure on `""` at rc 1, the `cd ""` family, and the run aborts. bash
    // 5.3.9 makes it a silent no-op (`rc 0`, the cwd unchanged) — divergence.
    let (out, err, code) = run_env(&work, &env, "HOME=; cd; builtin echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts: {out:?}");
    assert!(err.contains("ENOENT"), "stderr={err:?}");

    // `HOME` set in **neither** table: `$HOME not set` at rc 1. bash 5.3.9
    // prints `cd: HOME not set` at the same rc (measured) — no
    // password-database login directory for `cd`, so this row agrees.
    let (out, err, code) = run_env(&work, &[("HOME", "\0")], "cd; builtin echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts: {out:?}");
    assert!(err.contains("HOME not set"), "stderr={err:?}");

    // The scoped prefix form: `run_env::apply` lands before the intercept, so
    // the bare `cd` sees `S2` and the assignment is undone after the command —
    // bash 5.3.9 prints `S2` then `S1`, and fdshell agrees on both halves.
    let (out, err, code) = run_env(
        &work,
        &env,
        &format!("HOME={assigned} cd; builtin pwd; builtin echo home=$HOME"),
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out.trim(),
        format!("{assigned}\nhome={inherited}"),
        "the prefix moves the bare cd and is undone after it"
    );

    // A `local` shadow lives in the shell's own table, so it follows for free
    // (the #282 `local HOME` precedent for `~`).
    let (out, err, code) = run_env(
        &work,
        &env,
        &format!("f(){{ local HOME={assigned}; cd; builtin pwd; }}; f"),
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), assigned, "the local shadow must drive the cd");
}

/// The glob rows are the pipeline's own policy: one match cds, many matches are
/// `cd: too many arguments` (bash rc 2, fdshell rc 1 and the report aborts the
/// script — the accepted task #179 family), no match is the open failure.
#[test]
fn cd_glob_rows() {
    let work = workdir("glob");
    let home = scratch("glob-home");
    let (out, err, code) = run_in(&work, &home, "cd s*; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/sub", work.display()));

    let (out, err, code) = run_in(&work, &home, "cd *; builtin pwd; builtin echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("too many arguments"), "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts, so nothing prints: {out:?}");

    let (out, err, code) = run_in(&work, &home, "cd zzz*; builtin echo rc=$?");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty());
    assert!(err.contains("cd"), "the open failure is reported: {err:?}");

    // `set -f` keeps the pattern literal; the shopts drive the rest.
    let (out, _, code) = run_in(&work, &home, "set -f; cd s*; builtin pwd");
    assert_eq!(code, 1, "the literal pattern must fail, stdout={out:?}");
    let (out, err, code) = run_in(&work, &home, "shopt -s nullglob; cd s*; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/sub", work.display()));
    let (out, err, code) = run_in(&work, &home, "shopt -s nullglob; cd zzz*; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out.trim(),
        home.display().to_string(),
        "0 fields is the bare cd"
    );
    let (out, _, code) = run_in(
        &work,
        &home,
        "shopt -s failglob; cd zzz*; builtin echo rc=$?",
    );
    assert_eq!(code, 1, "the failed glob is an error, stdout={out:?}");
}

/// A quoted space keeps one path; extra fields are the usage error. The scoped
/// `IFS=` prefix is the accepted divergence family of
/// `tests/cmd_subst.rs::word0_scoped_ifs_divergence` (LESSONS #268 item 8):
/// fdshell splits the expansion on the **scoped** IFS (2 fields → the usage
/// error), where bash splits with the shell's IFS and reports `a: b: No such
/// file` for one name.
#[test]
fn cd_fields_are_the_target_rule() {
    let work = workdir("fields");
    let home = scratch("fields-home");
    let (out, err, code) = run_in(&work, &home, "cd \"a b\"; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/a b", work.display()));

    for script in ["cd a b", "cd sub extra", "cd $(echo sub; echo extra)"] {
        let (out, err, code) = run_in(&work, &home, &format!("{script}; builtin echo after"));
        assert_eq!(code, 1, "script {script:?}: stderr={err:?}");
        assert!(
            err.contains("too many arguments"),
            "script {script:?}: stderr={err:?}"
        );
        assert!(out.is_empty(), "the run aborts, so nothing prints: {out:?}");
    }

    // The scoped-IFS divergence, named: 2 fields in fdshell, 1 name in bash.
    let (out, _, code) = run_in(&work, &home, "v=a:b; IFS=: cd $v; builtin echo after");
    assert_eq!(
        code, 1,
        "the scoped IFS splits the expansion, stdout={out:?}"
    );
    // The bash-parity form (IFS set as its own statement) splits too.
    let (out, _, code) = run_in(&work, &home, "v=a:b; IFS=:; cd $v; builtin echo after");
    assert_eq!(
        code, 1,
        "the shell IFS splits the expansion, stdout={out:?}"
    );
}

/// `cd -` returns to the previous directory (fdshell prints nothing; bash
/// prints the new directory — task #378), and the form fires on the **expanded**
/// field: `D=-; cd "$D"` goes back even though a directory named `-` exists in
/// the cwd (measured: bash agrees). The literal directory is reached by any
/// other path.
#[test]
fn cd_dash_fires_on_the_expanded_field() {
    let work = workdir("dash");
    let home = scratch("dash-home");
    let (out, err, code) = run_in(&work, &home, "cd sub; cd -; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), work.display().to_string());

    for script in [
        "cd sub; D=-; cd \"$D\"; builtin pwd",
        // The same field through the positional list; `cd sub` first, because
        // `cd -` with no `OLDPWD` is a failure in bash too.
        "cd sub; set -- -; cd \"$@\"; builtin pwd",
    ] {
        let (out, err, code) = run_in(&work, &home, script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert_eq!(
            out.trim(),
            work.display().to_string(),
            "script {script:?} must land on OLDPWD"
        );
    }

    let (out, err, code) = run_in(&work, &home, "cd ./-; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/-", work.display()));
}

/// The `%name` form is read on the **raw** word: the pipeline's `%` arm yields
/// the fd *number*, so the word is never expanded. Quoting does not exempt it
/// (the gate is a raw-text test), an unset name is `fd variable not set`, and a
/// folded `\%d` pair is the literal path (bash's answer for `\%d`). Each row
/// opens its own directory, because `mkdirat` fails on a name that exists.
#[test]
fn cd_fd_variable_is_the_raw_word() {
    let work = workdir("fdvar");
    let home = scratch("fdvar-home");
    for (script, dir) in [
        ("cd %d; builtin pwd", "raw"),
        ("cd \"%d\"; builtin pwd", "quoted"),
    ] {
        let full = format!("builtin mkdirat --dirfd %CWD {dir} %>%d; {script}");
        let (out, err, code) = run_in(&work, &home, &full);
        assert_eq!(code, 0, "script {full:?}: stderr={err:?}");
        assert_eq!(
            out.trim(),
            format!("{}/{}", work.display(), dir),
            "script {full:?} must cd into the stored fd's directory"
        );
    }
    let (out, err, code) = run_in(&work, &home, "cd %NOPE; builtin echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(err.contains("fd variable not set"), "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts: {out:?}");
    let (out, _, code) = run_in(
        &work,
        &home,
        "builtin mkdirat --dirfd %CWD folded %>%d; cd \\%d; builtin echo after",
    );
    assert_eq!(
        code, 1,
        "the folded pair is the literal name `%d`: stdout={out:?}"
    );
}

/// `set -u` fails inside the pipeline: the unbound report, rc 1, and the run
/// aborts (bash's rc is 127 for `bash -c`, the `-c` quirk of LESSONS #424).
#[test]
fn cd_nounset_aborts_the_run() {
    let work = workdir("nounset");
    let home = scratch("nounset-home");
    let (out, err, code) = run_in(&work, &home, "set -u; cd $zz; builtin echo after");
    assert_eq!(code, 1);
    assert!(err.contains("unbound"), "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts: {out:?}");
}

/// `cd ""` is one field naming the empty path: the open failure on `""`, where
/// bash says `cd: null directory` (same rc 1). fdshell's failure **aborts the
/// script** (the accepted task #179 family), so the word after it never runs.
#[test]
fn cd_null_operand_is_the_open_failure() {
    let work = workdir("null");
    let home = scratch("null-home");
    let (out, err, code) = run_in(&work, &home, "cd \"\"; builtin echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts, so nothing prints: {out:?}");
    assert!(err.contains("ENOENT"), "stderr={err:?}");
}

/// `strict` bans nothing on `cd`'s operand, before or after the expansion: the
/// base follows the shell and `cd ..` is the documented escape hatch (README
/// "Strict mode"). The rows pin the exemption of the **expanded** value, so
/// turning the ban on for `cd` is a visible edit (plan §9's open question). The
/// operand is a shell variable, not a `$( … )`: strict bans every absolute
/// `PATH` entry, so a command substitution would yield no field at all.
#[test]
fn cd_is_outside_the_strict_ban() {
    let work = workdir("strict");
    let home = scratch("strict-home");
    let (out, err, code) = run_in(&work, &home, "shopt -s strict; d=sub; cd $d; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/sub", work.display()));
    let (out, err, code) = run_in(&work, &home, "shopt -s strict; d=/tmp; cd $d; builtin pwd");
    assert_eq!(
        code, 0,
        "the expanded absolute path is not banned: stderr={err:?}"
    );
    assert_eq!(out.trim(), "/tmp");
    let (out, err, code) = run_in(&work, &home, "shopt -s strict; d=..; cd $d; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out.trim(),
        work.parent().unwrap().display().to_string(),
        "the base moves, as the escape hatch documents"
    );
}

/// Keep-green rows: a literal path, `..`, the bare `cd` and a missing directory
/// behave as they did before the operand became a substitution surface.
#[test]
fn cd_literal_rows_stay_green() {
    let work = workdir("green");
    let home = scratch("green-home");
    let (out, err, code) = run_in(&work, &home, "cd sub; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/sub", work.display()));
    let (out, err, code) = run_in(&work, &home, "cd sub; cd ..; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), work.display().to_string());
    let (out, err, code) = run_in(&work, &home, "cd; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), home.display().to_string());
    let (out, _, code) = run_in(
        &work,
        &home,
        "cd /nonexistent-cd-expand; builtin echo after",
    );
    assert_eq!(code, 1, "stdout={out:?}");
    // A quoted word never splits and never globs: `cd 's*'` is the open failure
    // on the literal pattern (bash agrees).
    let (out, _, code) = run_in(&work, &home, "cd 's*'; builtin echo after");
    assert_eq!(code, 1, "stdout={out:?}");
}

/// `$_` after an intercept is the **raw** word, where bash stores the expanded
/// field. Accepted divergence, owned by task #376.
#[test]
fn cd_last_arg_is_the_raw_word() {
    let work = workdir("lastarg");
    let home = scratch("lastarg-home");
    let (out, err, code) = run_in(&work, &home, "V=sub; cd $V; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out.trim(),
        "$V",
        "bash prints `sub`; fdshell stores the raw word (task #376)"
    );
}

/// `cd`'s option words (`--`, `-L`, `-P`, `-x`) are operands today: `--` and
/// `-x` are the open failure on the literal word, `-L dir` is the usage error.
/// bash parses the options (`cd --` is the bare `cd`, `cd -L sub` cds,
/// `cd -x` is `invalid option` at rc 2). Task #380 owns that surface and must
/// update these rows when it lands.
#[test]
fn cd_option_words_are_operands() {
    let work = workdir("options");
    let home = scratch("options-home");
    // `--` and `-x` are opened as paths: the ENOENT report at rc 1, where bash
    // prints `$HOME` and `cd: -x: invalid option` at rc 2.
    for script in ["cd --", "cd -x"] {
        let (out, err, code) = run_in(&work, &home, &format!("{script}; builtin echo after"));
        assert_eq!(code, 1, "script {script:?}: stderr={err:?}");
        assert!(out.is_empty(), "the run aborts: {out:?}");
        assert!(err.contains("ENOENT"), "script {script:?}: stderr={err:?}");
    }
    // `-L dir`/`-P dir` are two fields: the usage error, where bash cds.
    for script in ["cd -L sub", "cd -P sub"] {
        let (out, err, code) = run_in(&work, &home, &format!("{script}; builtin echo after"));
        assert_eq!(code, 1, "script {script:?}: stderr={err:?}");
        assert!(out.is_empty(), "the run aborts: {out:?}");
        assert!(
            err.contains("too many arguments"),
            "script {script:?}: stderr={err:?}"
        );
    }
}

/// Escape pairs fold inside the operand, as every word does: `cd su\b` is the
/// directory `sub`, and a pattern protected by a pair never globs.
#[test]
fn cd_operand_folds_escape_pairs() {
    let work = workdir("escape");
    let home = scratch("escape-home");
    let (out, err, code) = run_in(&work, &home, "cd su\\b; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.trim(), format!("{}/sub", work.display()));
    // The folded `*` is mask-protected, so no glob runs: the literal `s*` name
    // fails, exactly as the quoted row does.
    let (out, _, code) = run_in(&work, &home, "cd s\\*; builtin echo after");
    assert_eq!(code, 1, "stdout={out:?}");
}
