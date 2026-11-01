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
/// unset HOME; cd; pwd  (env HOME=S1)     bash rc 1 `cd: HOME not set`   fdshell rc 1, run aborts (task #290)
/// HOME=S2; unset HOME; cd; pwd           bash rc 1 `cd: HOME not set`   fdshell rc 1, run aborts
/// ```
///
/// The inherited-only row (`cd; pwd` → `S1`) is the keep-green
/// `cd_literal_rows_stay_green` and `cd_zero_fields_and_tilde_are_home` pair,
/// which run through `run_in`. The `unset HOME` probe of that fallback became
/// expressible with task #290 (`unset NAME` took over the shell-variable
/// spelling, including the inherited `environ` row), and it is pinned at the
/// bottom of this test.
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
    assert!(
        err.contains("No such file or directory"),
        "bash is a silent no-op here; stderr={err:?}"
    );

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

    // The `unset HOME` probe of the **fallback** half, expressible since task
    // #290 took `unset NAME` over to shell variables: taking the inherited
    // `HOME` away leaves the bare `cd` with no target, and an assignment made
    // *before* the unset is gone with it (bash 5.3.9 measured, inherited
    // `HOME=S1`):
    //
    // ```text
    // unset HOME; cd; pwd            bash rc 1 `cd: HOME not set`, pwd prints the cwd
    // HOME=S2; unset HOME; cd; pwd   bash rc 1 `cd: HOME not set`, pwd prints the cwd
    // ```
    //
    // fdshell reports the same words at the same rc, and its rc-1 report
    // **aborts the run** (the accepted family), so the row pins the message and
    // the rc — the continuation is what bash's `pwd` prints and fdshell never
    // reaches.
    for script in [
        "unset HOME; cd".to_string(),
        format!("HOME={assigned}; unset HOME; cd"),
    ] {
        let (out, err, code) = run_env(&work, &env, &script);
        assert_eq!(code, 1, "script {script:?}: stderr={err:?}");
        assert!(out.is_empty(), "script {script:?}: stdout={out:?}");
        assert!(
            err.contains("HOME not set"),
            "script {script:?}: stderr={err:?}"
        );
    }
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

/// `cd -` returns to the previous directory **and writes it to standard output**
/// (POSIX XCU 4.57 — every row's bytes are `bash -c`'s own answer, measured in
/// the same layout first), and the form fires on the **expanded** field: `D=-;
/// cd "$D"` goes back even though a directory named `-` exists in the cwd
/// (measured: bash agrees). The literal directory is reached by any other path —
/// the path form stays silent, so that row is one line.
#[test]
fn cd_dash_fires_on_the_expanded_field() {
    let work = workdir("dash");
    let home = scratch("dash-home");
    let w = work.display().to_string();
    for script in [
        "cd sub; cd -; builtin pwd",
        "cd sub; D=-; cd \"$D\"; builtin pwd",
        // The same field through the positional list; `cd sub` first, because
        // `cd -` with no `OLDPWD` is a failure in bash too.
        "cd sub; set -- -; cd \"$@\"; builtin pwd",
    ] {
        let (out, err, code) = run_in(&work, &home, script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        // Two lines: the `-` form's print, then `pwd`'s — the same path, and both
        // are exactly what bash prints.
        assert_eq!(
            out,
            format!("{w}\n{w}\n"),
            "script {script:?} must land on OLDPWD"
        );
    }

    let (out, err, code) = run_in(&work, &home, "cd ./-; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("{w}/-\n"), "the path form prints nothing");
}

/// POSIX XCU 4.57's print, on every route that decides the field is `-`, and
/// every row measured against `bash -c` (5.3.9) in the same layout before being
/// pinned: the plain form, the double bounce, the capture, and `strict` (which
/// bans nothing on `cd` — README "Strict mode"). The printed value is what
/// fdshell's own `pwd` prints there, `getcwd` being the shell's only source for
/// the cwd's path; bash prints its `PWD` value, and the two can only part on a
/// path reached through a symlink, which `cd`'s `O_NOFOLLOW` open refuses
/// (`cd_dash_prints_the_physical_path`).
#[test]
fn cd_dash_prints_the_new_directory() {
    let work = workdir("dashprint");
    let home = scratch("dashprint-home");
    let w = work.display().to_string();

    let (out, err, code) = run_in(&work, &home, "cd sub; cd -");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out,
        format!("{w}\n"),
        "bash od-verified: the path, one newline"
    );

    // Every `-` use prints: bash answers `W`, `W/sub`, then `pwd`'s `W/sub`.
    let (out, err, code) = run_in(&work, &home, "cd sub; cd -; cd -; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("{w}\n{w}/sub\n{w}/sub\n"));

    // The print is captured when fd 1 is the capture pipe, and a `$( … )` value
    // loses its trailing newline on assignment, as `tests/cmd_subst.rs` pins.
    let (out, err, code) = run_in(
        &work,
        &home,
        "x=$(cd sub; cd -); builtin echo \"x=$x\"; cd sub; builtin pwd",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("x={w}\n{w}/sub\n"));

    // `strict` changes nothing here; bash has no `strict` option, so this row is
    // fdshell's own — and it is bash's shape above without the shopt.
    let (out, err, code) = run_in(&work, &home, "shopt -s strict; cd sub; cd -; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("{w}\n{w}\n"));
}

/// `cd - > f`: the redirect belongs to the command, so the line lands in the file
/// opened in the cwd **at redirect time** (`sub/f`, bash-verified) and only
/// `pwd`'s line reaches stdout. The parent's `redirect::Scope` already moves fd 1
/// for intercepted builtins (`validation.rs`), so the print follows it.
#[test]
fn cd_dash_print_goes_where_the_redirect_goes() {
    let work = workdir("dashredirect");
    let home = scratch("dashredirect-home");
    let w = work.display().to_string();
    let (out, err, code) = run_in(&work, &home, "cd sub; cd - > f; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("{w}\n"), "only `pwd`'s line is left on stdout");
    assert_eq!(
        std::fs::read(work.join("sub/f")).unwrap(),
        format!("{w}\n").as_bytes(),
        "bash writes the same bytes into `sub/f`"
    );
}

/// The printed value is fdshell's own `pwd` answer — the **physical** path — where
/// bash prints its `PWD` value. They can only part on a chain with a symlink hop,
/// and `cd`'s `O_NOFOLLOW` open refuses a symlink operand, so the row dies at the
/// open (rc 1, nothing on stdout) where bash prints its logical `…/link` twice.
/// The all-physical twin of the same chain is bash-exact, and that is the rule
/// this test pins: the two shells print the same bytes wherever they can both move.
#[test]
fn cd_dash_prints_the_physical_path() {
    let work = scratch("dashlink");
    std::fs::create_dir_all(work.join("d/d2")).unwrap();
    std::os::unix::fs::symlink("d", work.join("link")).unwrap();
    let w = work.display().to_string();

    let (out, err, code) = run_in(&work, &work, "cd link; cd d2; cd -; builtin pwd");
    assert_eq!(
        code, 1,
        "bash cds through the link and prints {w}/link twice: stderr={err:?}"
    );
    assert!(
        out.is_empty(),
        "the run aborts at the symlink open: {out:?}"
    );
    assert!(err.contains("Not a directory"), "stderr={err:?}");

    let (out, err, code) = run_in(&work, &work, "cd d; cd d2; cd -; builtin pwd");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out,
        format!("{w}/d\n{w}/d\n"),
        "bash answers the same bytes here"
    );
}

/// The print is gated on a **successful** move: a `cd -` with nothing in the
/// `OLDCWD` slot reports the failure and prints nothing on stdout, exactly as
/// bash does. The words and the continuation are the named divergences — bash
/// says `cd: OLDPWD not set` at rc 1 and its run **continues** (the task #179
/// family), fdshell reports and aborts.
#[test]
fn cd_dash_prints_only_after_a_successful_move() {
    let work = workdir("dashnone");
    let home = scratch("dashnone-home");
    let (out, err, code) = run_in(&work, &home, "cd -");
    assert_eq!(code, 1, "bash's rc is 1 too: stderr={err:?}");
    assert!(out.is_empty(), "bash's stdout is empty too: {out:?}");
    assert!(err.contains("fd variable not set"), "stderr={err:?}");
}

/// The write itself failing is an error like any other: bash reports
/// `cd: write error: No space left on device` at rc 1, moves the cwd, and
/// **continues** (so it prints `after`). fdshell reports the same fact in its own
/// words at rc 1 and aborts the run — the accepted task #179 family, so the row
/// pins the abort and the message, never a source frame.
#[test]
fn cd_dash_write_failure_is_reported() {
    let work = workdir("dashfull");
    let home = scratch("dashfull-home");
    let (out, err, code) = run_in(&work, &home, "cd sub; cd - >/dev/full; builtin echo after");
    assert_eq!(
        code, 1,
        "bash prints `after` after its rc-1 error: stderr={err:?}"
    );
    assert!(out.is_empty(), "the run aborts, so nothing prints: {out:?}");
    assert!(
        err.contains("failed to write the new directory"),
        "the variant's own text: {err:?}"
    );
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
    assert!(
        err.contains("No such file or directory"),
        "bash: `cd: null directory`; stderr={err:?}"
    );
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

/// `$_` after an intercept is the last **expanded** field, as bash sets it —
/// the divergence task #376 owned is closed, so this row is a bash-parity pin.
/// The twin is the `'…'` pair: a quoted word is not expanded, so the literal
/// name `$V` is the field, and the row cds into a directory literally named
/// `$V` (the same pair the unit pin in `intercept/cd/tests.rs` walks). An
/// **option** word is a field too — bash sets `$_` from the last word it read,
/// including `--` (task #380's surface).
#[test]
fn cd_last_arg_is_the_expanded_field() {
    let work = workdir("lastarg");
    let home = scratch("lastarg-home");
    std::fs::create_dir(work.join("$V")).unwrap();
    for (script, want) in [
        ("V=sub; cd $V; builtin echo \"$_\"", "sub"),
        ("V=sub; cd '$V'; builtin echo \"$_\"", "$V"),
        ("cd --; builtin echo \"$_\"", "--"),
        ("cd -- sub; builtin echo \"$_\"", "sub"),
    ] {
        let (out, err, code) = run_in(&work, &home, script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert_eq!(
            out.trim(),
            want,
            "script {script:?} matches bash (task #376)"
        );
    }
}

/// The option surface (task #380): the four words bash accepts (`--`, `-L`,
/// `-P`, `-e`) are parsed on the **expanded** fields, from the head of the list,
/// and each of the last three is a no-op — the move is a pinned
/// `O_DIRECTORY + O_NOFOLLOW` fd, so `-L` and `-P` have nothing to disagree
/// about here. Every bash column was measured in `bash -c` (5.3.9) in this same
/// layout; the divergences are named in the assertions that carry them.
#[test]
fn cd_parses_the_accepted_option_words() {
    let work = workdir("options");
    let home = scratch("options-home");
    // `cd --` is the bare `cd`, `cd -- sub` leaves `sub` as the operand, and the
    // option word may arrive from an expansion — quoting is no gate.
    for (script, want) in [
        ("cd --; builtin pwd", home.display().to_string()),
        ("cd -- sub; builtin pwd", format!("{}/sub", work.display())),
        (
            "V=-L; W=sub; cd \"$V\" \"$W\"; builtin pwd",
            format!("{}/sub", work.display()),
        ),
    ] {
        let (out, err, code) = run_in(&work, &home, script);
        assert_eq!(code, 0, "script {script:?}: stderr={err:?}");
        assert_eq!(out.trim(), want, "script {script:?} matches bash");
    }
    // After `--` the dash rule is `-` only, so `-L` is a directory name: the open
    // failure on the name, which is exactly what bash reports for it.
    let (out, err, code) = run_in(&work, &home, "cd -- -L; builtin echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts: {out:?}");
    assert!(err.contains("No such file or directory"), "stderr={err:?}");
    // Pinned divergences: bash accepts a cluster (`-LP sub` cds, rc 0) and names
    // the first bad character of `---` (`--`), while fdshell matches the four
    // words whole and names the whole word; and where bash prints a usage line at
    // rc 2 and continues, fdshell prints no usage line and reports at rc 1 (the
    // task #179 family).
    for (script, want) in [
        ("cd -x", "cd: -x: invalid option"),
        ("cd -LP sub", "cd: -LP: invalid option"),
        ("cd ---", "cd: ---: invalid option"),
    ] {
        let (out, err, code) = run_in(&work, &home, &format!("{script}; builtin echo after"));
        assert_eq!(code, 1, "script {script:?}: stderr={err:?}");
        assert!(out.is_empty(), "the run aborts: {out:?}");
        assert!(err.contains(want), "script {script:?}: stderr={err:?}");
    }
    // A lone `-` is not an option — it is the `OLDPWD` operand — so the operand
    // count is what answers: bash's rc 2, fdshell's rc 1.
    let (out, err, code) = run_in(&work, &home, "cd - a b; builtin echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert!(out.is_empty(), "the run aborts: {out:?}");
    assert!(err.contains("too many arguments"), "stderr={err:?}");
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
