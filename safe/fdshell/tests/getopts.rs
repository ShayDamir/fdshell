#![allow(clippy::unwrap_used)]

//! `getopts` resolves as an **in-shell intercept**, and the `OPTIND` state it
//! needs is seeded at startup as a visible variable (task #589). Every row below
//! was measured in both shells on this machine on 2026-11-16 before being pinned;
//! the harness removes `OPTIND` and `OPTARG` from the shell's own environment, so a
//! row in a child environment can only come from fdshell's stores, and the child
//! rows use the `env_print` helper (no external `env`/`grep`).
//!
//! ```text
//!                                                        bash        fdshell
//! builtin echo "[$OPTIND]"                               [1]        [1]
//! the OPTIND row of `set`                                OPTIND=1    OPTIND=1
//! OPTIND=7; OPTARG=z; env_print (rows)                   0           0
//! OPTIND=7; export OPTIND; env_print (rows)              1           1
//! OPTARG=z; builtin echo "[$OPTARG]"                     [z]         [z]
//! OPTARG=z; unset OPTARG; builtin echo "[$OPTARG]"       []          []
//! set -- -a; getopts a o  (stderr)                       (empty)     (empty)
//! f(){ local OPTIND=3; }; f; builtin echo "[$OPTIND]"    [1]        [1]
//! f(){ OPTIND=3; }; f; builtin echo "[$OPTIND]"          [3]        [3]
//! OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]"  [1]        [1]
//! unset OPTIND; OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]"  []  []
//! the idiomatic loop, set -- -a -bval x (body)           a 2, b 3 val   same
//! -bval / -b val / -ab val / -b -a x (body)              2 val/3/3 -a   same
//! -ab on ab:c (stderr; the reporting call answers 0)     msg; ? row     same, $0 differs
//! silent :ab:c on -x / -b (stdout; stderr empty both)    x; b as `:`    same
//! one call on -- x / foo / - (name, OPTIND, rc)          ? 2 1 / ? 1 1  same
//! two parameter sets, OPTIND=1 between (body)            val, val2      same
//! explicit operands -a -b val (name, OPTIND, rc, $1)    a 2 0 keepme   same
//! f(){ loop; }; f -b val; echo (OPTIND, name)            3 ?            same
//! the z row and the 1abc row in -c mode (stderr)        bash: prefix     sh: prefix
//! the same two rows from a script file (stderr, rc 1)   line 1 field     no field
//! getopts abc (stderr, rc 2)                            getopts: usage   same
//! set -u, OPTARG after -a (the unbound read reported?)  yes, aborts 127  yes, continues 0
//! ```
//!
//! Rows whose status a `while` that ends on a false condition reports now pin
//! the script status too, at bash parity — the body's last command's status, and
//! `0` when the body never ran (task #595). The `$0` and `line N:` spellings are
//! the already-documented layers, not new decisions.
//!
//! The scan itself is task #590, so the last row pins only that `getopts` is
//! **claimed** — the forked launch path's `failed to resolve command path` is gone.
//! The status is not pinned (bash's no-operand call exits 1), and neither is bash's
//! integer coercion of an unparseable `OPTIND` (bash's `$OPTIND` answers `0`,
//! fdshell's the assigned bytes) — both are #590/#591 decisions. The `local` and
//! scoped-prefix scope rows and the child-environment non-export row are #593's and
//! #589's tests below — referenced, not re-pinned.

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// The helper that prints **its own** environment (built by `build.rs`).
const ENV_PRINT: &str = env!("ENV_PRINT_PATH");

/// The helper dir first, then the ambient `PATH` (Nixos has no `/usr/bin`), so the
/// script's `env_print` call resolves (the `tests/cd_expand.rs` shape).
fn test_path() -> String {
    let path_dir = ENV_PRINT
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("/usr/bin");
    let ambient = std::env::var("PATH").unwrap_or_default();
    format!("{path_dir}:{ambient}")
}

/// `fdshell -c`, with `OPTIND` and `OPTARG` **removed** from the shell's
/// environment: a row in a child environment is then fdshell's own doing, never the
/// harness's (the `tests/ifs_startup.rs::run_env` shape).
fn run(script: &str) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(["-c", script])
        .env("PATH", test_path())
        .env_remove("OPTIND")
        .env_remove("OPTARG")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// `fdshell <script path>` — the script file sees its own path in `$0` (the same
/// harness-environment rule as [`run`]).
fn run_file(path: &str) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.arg(path)
        .env_remove("OPTIND")
        .env_remove("OPTARG")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A pid + counter scratch dir `fdshell-getopts-<tag>-<pid>-<counter>`, removed
/// when the test's guard drops (the `tests/cli_args.rs` shape — pids are
/// recycled, so the scratch must die with its process).
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
    let dir =
        std::env::temp_dir().join(format!("fdshell-getopts-{tag}-{}-{c}", std::process::id(),));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// The seed is a real variable: `$OPTIND` answers it and `set` lists it (bash
/// measures `OPTIND=1`).
#[test]
fn the_startup_optind_is_visible_in_shell() {
    let (out, err, code) = run(r#"builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[1]\n", "bash measures the same");

    let (out, err, code) = run("set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == "OPTIND=1"), "stdout={out:?}");
}

/// An ordinary assignment does **not** export either name — bash measures zero rows
/// — and an explicit `export` is the only exporter (bash measures one row there).
#[test]
fn optind_and_optarg_stay_out_of_the_child_environment() {
    let (out, err, code) = run("OPTIND=7; OPTARG=z; env_print");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        !out.lines().any(|l| l.starts_with("OPTIND=")),
        "stdout={out:?}"
    );
    assert!(
        !out.lines().any(|l| l.starts_with("OPTARG=")),
        "stdout={out:?}"
    );

    let (out, err, code) = run("OPTIND=7; export OPTIND; env_print");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == "OPTIND=7"),
        "the ordinary export path exports, in bash too"
    );
}

/// `OPTARG` needs **no field**: it is a plain store variable, so the ordinary
/// assignment and `unset` paths are the whole story here (the scan sets it in #590).
#[test]
fn optarg_is_an_ordinary_variable_on_the_assignment_path() {
    let (out, err, code) = run(r#"OPTARG=z; builtin echo "[$OPTARG]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[z]\n", "bash measures the same");

    let (out, err, code) = run(r#"OPTARG=z; unset OPTARG; builtin echo "[$OPTARG]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n", "bash measures the same");
}

/// `getopts` is **claimed** as an intercept, so the forked launch path no longer
/// fails it with `failed to resolve command path` (POSIX gaps #6.9). Nothing on
/// stderr is the bash-measured row; the status is left to #590.
#[test]
fn getopts_no_longer_fails_to_resolve_its_path() {
    let (out, err, _code) = run("set -- -a; getopts a o");
    assert_eq!(err, "", "no diagnostic; stdout={out:?}");
}

/// A `local OPTIND=3` is undone when the call returns — the frame's counter
/// mirror (`old_optind`) rides the same restore as the store entry. The plain
/// assignment control has no frame entry and **leaks** (task #593, bash rows
/// measured on this machine).
#[test]
fn local_optind_is_restored_when_the_call_returns() {
    let (out, err, code) = run(r#"f(){ local OPTIND=3; }; f; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[1]\n", "bash measures the same");

    let (out, err, code) = run(r#"f(){ OPTIND=3; }; f; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out, "[3]\n",
        "no frame entry, so the assignment leaks, in bash too"
    );
}

/// The scoped prefix is undone after the command (the visible half of
/// `EnvSave::restore`; the plain-assignment control of #589 stays as pinned).
#[test]
fn a_scoped_optind_prefix_is_undone_after_the_command() {
    let (out, err, code) = run(r#"OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[1]\n", "bash measures the same");
}

/// Unbound before the prefix, unbound after it: the name has no saved entry to
/// restore, and only the saved counter brings the state back (bash answers `[]`,
/// task #593).
#[test]
fn an_unbound_optind_stays_unbound_across_a_scoped_prefix() {
    let (out, err, code) =
        run(r#"unset OPTIND; OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n", "bash measures the same");
}

/// The idiomatic `while getopts optstring name; do … done` loop, end to end —
/// the shape POSIX cares about, and it only terminates because `getopts` runs
/// **in the current shell environment**: every iteration re-invokes the builtin
/// through the loop and sees the `OPTIND` the previous one committed. The script
/// status is pinned at bash parity — the body's last command's (task #595) — and
/// `-c` and script-file modes agree.
#[test]
fn the_idiomatic_loop_runs_end_to_end() {
    let (out, err, code) = run(
        r#"set -- -a -bval x; while getopts "ab:c" o; do echo "o=[$o] i=$OPTIND a=[$OPTARG]"; done"#,
    );
    assert_eq!(
        out, "o=[a] i=2 a=[]\no=[b] i=3 a=[val]\n",
        "bash measures the same"
    );
    assert_eq!(err, "", "no diagnostic; bash too");
    assert_eq!(code, 0, "bash measures the same");

    let dir = scratch("parity");
    let script = dir.join("idiomatic.sh");
    std::fs::write(
        &script,
        "set -- -a -bval x\nwhile getopts \"ab:c\" o; do echo \"o=[$o] i=$OPTIND a=[$OPTARG]\"; done\n",
    )
    .unwrap();
    let path = script.to_str().unwrap().to_string();
    let (out, err, code) = run_file(&path);
    assert_eq!(
        out, "o=[a] i=2 a=[]\no=[b] i=3 a=[val]\n",
        "bash measures the same"
    );
    assert_eq!(err, "", "bash too");
    assert_eq!(
        code, 0,
        "script-file mode agrees with `-c`; bash measures the same"
    );
}

/// The argument forms in one loop, measured in both shells: attached `-bval`,
/// separate `-b val`, and the cluster-end form `-ab val`, where the cluster
/// letter that needs an argument takes the next **whole** word. `-b` takes `-a`
/// as its argument and the word `x` then ends the walk. The plain multi-flag
/// cluster `-ab` ends on `b` with no argument left: verbose mode reports it on
/// **stderr** and the reporting call answers 0, so the body still runs (POSIX's
/// text stops at the error instead).
#[test]
fn the_argument_forms_answer_one_loop() {
    let body = r#"while getopts "ab:c" o; do echo "o=[$o] i=$OPTIND a=[$OPTARG]"; done"#;

    let (out, err, code) = run(&format!("set -- -bval; {body}"));
    assert_eq!(out, "o=[b] i=2 a=[val]\n", "bash measures the same");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0, "the body's last command; bash measures the same");

    let (out, err, code) = run(&format!("set -- -b val; {body}"));
    assert_eq!(out, "o=[b] i=3 a=[val]\n", "bash measures the same");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0, "bash measures the same");

    let (out, err, code) = run(&format!("set -- -ab val; {body}"));
    assert_eq!(
        out, "o=[a] i=1 a=[]\no=[b] i=3 a=[val]\n",
        "the cluster letter takes the whole next word; bash the same"
    );
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0, "bash measures the same");

    let (out, err, code) = run(&format!("set -- -b -a x; {body}"));
    assert_eq!(out, "o=[b] i=3 a=[-a]\n", "bash measures the same");
    assert_eq!(err, "", "x ends the walk; bash too");
    assert_eq!(code, 0, "bash measures the same");

    let (out, err, code) = run(&format!("set -- -ab; {body}"));
    assert_eq!(
        out, "o=[a] i=1 a=[]\no=[?] i=2 a=[]\n",
        "bash measures the same"
    );
    assert_eq!(
        err, "sh: option requires an argument -- b\n",
        "bash measures the same message under its own name: bash: option requires an argument -- b"
    );
    assert_eq!(code, 0, "bash measures the same");
}

/// Silent mode (the optstring's leading `:`) side by side with verbose on the
/// same words, streams asserted separately: the verbose half prints its message
/// on **stderr**, not stdout, and leaves `OPTARG` unbound (reads empty); the
/// silent half prints **nothing** and answers in the variables instead — the
/// offender in `OPTARG` for the unknown option, and for the missing
/// option-argument the option letter in `OPTARG` under the name `:` (the one row
/// whose name is not `?`). Both reporting calls answer 0 and keep going — the
/// body line is that proof; POSIX's text stops at the error. The script status
/// is pinned at bash parity (task #595).
#[test]
fn silent_mode_answers_without_a_diagnostic() {
    let body = r#"while getopts {OPT} o; do echo "o=[$o] i=$OPTIND a=[$OPTARG]"; done"#;

    // verbose: unknown option
    let (out, err, code) = run(&format!(
        "set -- -x; {}",
        body.replace("{OPT}", r#""ab:c""#)
    ));
    assert_eq!(
        out, "o=[?] i=2 a=[]\n",
        "unbound OPTARG reads empty; bash the same"
    );
    assert_eq!(
        err, "sh: illegal option -- x\n",
        "bash measures the same message under its own name: bash: illegal option -- x"
    );
    assert_eq!(code, 0, "bash measures the same");

    // silent: unknown option
    let (out, err, code) = run(&format!(
        "set -- -x; {}",
        body.replace("{OPT}", r#"":ab:c""#)
    ));
    assert_eq!(
        out, "o=[?] i=2 a=[x]\n",
        "the offender lands in OPTARG; bash the same"
    );
    assert_eq!(err, "", "silent mode prints nothing, in bash too");
    assert_eq!(code, 0, "bash measures the same");

    // verbose: missing option-argument
    let (out, err, code) = run(&format!(
        "set -- -b; {}",
        body.replace("{OPT}", r#""ab:c""#)
    ));
    assert_eq!(
        out, "o=[?] i=2 a=[]\n",
        "unbound OPTARG reads empty; bash the same"
    );
    assert_eq!(
        err, "sh: option requires an argument -- b\n",
        "bash under its own name"
    );
    assert_eq!(code, 0, "bash measures the same");

    // silent: missing option-argument
    let (out, err, code) = run(&format!(
        "set -- -b; {}",
        body.replace("{OPT}", r#"":ab:c""#)
    ));
    assert_eq!(
        out, "o=[:] i=2 a=[b]\n",
        "the one row whose name is not `?`; bash the same"
    );
    assert_eq!(err, "", "silent mode prints nothing, in bash too");
    assert_eq!(code, 0, "bash measures the same");

    // the status read inside the script, single call: POSIX's text answers >0
    // (a 2 for this colon row) at the presentation error; bash measures **0**
    // for the reporting call, and so does fdshell.
    let (out, err, code) =
        run(r#"set -- -b; getopts ":ab:c" o; echo "rc=$? o=[$o] a=[$OPTARG] i=$OPTIND""#);
    assert_eq!(out, "rc=0 o=[:] a=[b] i=2\n", "bash measures the same");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);
}

/// The `--` terminator and the first non-`-` argument, each a **single** call so
/// the status read inside the script is the call's own: the terminator answers
/// `?` at rc 1 with `OPTIND` moved past itself, the first non-option ends the
/// walk at rc 1 with the counter untouched, and a lone `-` ends it on its own
/// index. All three at full parity, no diagnostics.
#[test]
fn the_terminator_and_the_first_non_dash_argument_end_the_walk() {
    let probe = r#"getopts "ab:c" o; echo "rc=$? o=[$o] i=$OPTIND a=[$OPTARG]""#;

    let (out, err, code) = run(&format!("set -- -- x; {probe}"));
    assert_eq!(out, "rc=1 o=[?] i=2 a=[]\n", "bash measures the same");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);

    let (out, err, code) = run(&format!("set -- foo; {probe}"));
    assert_eq!(out, "rc=1 o=[?] i=1 a=[]\n", "bash measures the same");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);

    let (out, err, code) = run(&format!("set -- -; {probe}"));
    assert_eq!(out, "rc=1 o=[?] i=1 a=[]\n", "bash measures the same");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);
}

/// `OPTIND=1` restarts the scan for a **new parameter set**: two loops in one
/// script, each with its own `set --`, both answering their own `OPTARG` (the
/// script status is pinned at bash parity — task #595). The explicit-operand
/// form is a single call at full parity: the operand list wins and the
/// positionals stay untouched.
#[test]
fn optind_1_resets_the_scan_for_a_new_parameter_set() {
    let body = r#"while getopts "ab:c" o; do echo "o=[$o] a=[$OPTARG] i=$OPTIND"; done"#;
    let (out, err, code) = run(&format!(
        "set -- -b val; {body}; OPTIND=1; set -- -b val2; {body}"
    ));
    assert_eq!(
        out, "o=[b] a=[val] i=3\no=[b] a=[val2] i=3\n",
        "bash measures the same body lines"
    );
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0, "bash measures the same");

    let (out, err, code) = run(
        r#"set -- keepme; getopts "ab:c" o -a -b val; echo "rc=$? o=[$o] i=$OPTIND a=[$OPTARG] p1=[$1]""#,
    );
    assert_eq!(
        out, "rc=0 o=[a] i=2 a=[] p1=[keepme]\n",
        "bash measures the same"
    );
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);
}

/// `getopts` **inside a function**: `OPTIND` is shared with the calling shell —
/// no save/restore, the caller's problem, and the name write is not scoped
/// either (POSIX says the same; the `local` and prefix rows are #593's tests
/// above). The function's positional parameters are its own: the same call
/// without arguments ends the walk at once and the body never runs.
#[test]
fn a_function_shares_optind_with_the_calling_shell() {
    let (out, err, code) = run(
        r#"f(){ while getopts "ab:c" o; do :; done; }; f -b val; echo "after f i=$OPTIND o=[$o]""#,
    );
    assert_eq!(out, "after f i=3 o=[?]\n", "bash measures the same");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);

    let (out, err, code) = run(
        r#"f(){ while getopts "ab:c" o; do echo in; done; }; set -- -b val; f; echo "after f i=$OPTIND o=[$o]""#,
    );
    assert_eq!(
        out, "after f i=1 o=[?]\n",
        "no in line — f's own positionals are empty; bash the same"
    );
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);
}

/// Diagnostics carry fdshell's `$0` — `sh` in `-c` mode where bash prints
/// `bash:` (the documented own-`$0` layer) — and go to **stderr**, not stdout.
/// The identifier row also loses bash's `line N:` field: fdshell's builtin
/// diagnostics carry no script-line prefix (#591's decision, pinned at unit
/// level, shown here across the process boundary). The statuses read inside the
/// script are at parity: the unknown option answers 0 for the reporting call;
/// the identifier error answers 1 with the scan committed behind it (the unit
/// shape).
#[test]
fn diagnostics_carry_the_shells_own_dollar_zero() {
    let (out, err, code) = run(r#"set -- -z; getopts "ab:c" o; echo "rc=$?""#);
    assert_eq!(out, "rc=0\n", "the reporting call answers 0; bash the same");
    assert_eq!(
        err, "sh: illegal option -- z\n",
        "bash measures bash: illegal option -- z"
    );
    assert_eq!(code, 0);

    let (out, err, code) =
        run(r#"set -- -bval; getopts "ab:c" 1abc; echo "rc=$? i=$OPTIND a=[$OPTARG]""#);
    assert_eq!(
        out, "rc=1 i=2 a=[val]\n",
        "the scan commits, only the name write fails; bash the same"
    );
    assert_eq!(
        err, "sh: getopts: `1abc': not a valid identifier\n",
        "bash measures bash: line 1: getopts: `1abc': not a valid identifier"
    );
    assert_eq!(code, 0);
}

/// From a **script file**, both shells name the script as given — the path in
/// `$0`. The option row is at full parity (bash adds no `line N:` field to
/// option messages); the identifier row is where they part: bash adds its
/// `line 1:` field, fdshell does not. The statuses are at parity: the
/// identifier error answers 1, the reporting call 0.
#[test]
fn a_script_file_names_itself_in_the_diagnostics() {
    let dir = scratch("diag");
    let ident = dir.join("ident.sh");
    let opt = dir.join("opt.sh");
    std::fs::write(&ident, "getopts abc 1abc\n").unwrap();
    std::fs::write(&opt, "set -- -z\ngetopts abc o\n").unwrap();

    let path = ident.to_str().unwrap().to_string();
    let (out, err, code) = run_file(&path);
    assert_eq!(out, "", "bash too");
    assert_eq!(
        err,
        format!("{path}: getopts: `1abc': not a valid identifier\n"),
        "bash measures {path}: line 1: getopts: `1abc': not a valid identifier"
    );
    assert_eq!(code, 1, "bash measures 1 too");

    let path = opt.to_str().unwrap().to_string();
    let (out, err, code) = run_file(&path);
    assert_eq!(out, "", "bash too");
    assert_eq!(
        err,
        format!("{path}: illegal option -- z\n"),
        "bash measures the same"
    );
    assert_eq!(code, 0, "the reporting call's status; bash the same");
}

/// The usage row carries no shell prefix **in either shell** — POSIX's
/// `<prog>` spelling survives exactly here — answers **2**, and writes nothing:
/// full parity on every stream.
#[test]
fn the_usage_line_has_no_shell_prefix_and_exits_two() {
    let (out, err, code) = run("getopts abc");
    assert_eq!(out, "", "nothing on stdout; bash the same");
    assert_eq!(
        err, "getopts: usage: getopts optstring name [arg ...]\n",
        "bash the same"
    );
    assert_eq!(code, 2, "bash's measured 2");
}

/// `OPTARG` is set **only** when an option-argument was found: the README
/// §Variables probe — `set -- -a; getopts "ab:c" o` answers empty for
/// `${OPTARG+set}` — agrees in both shells (fdshell's `+` arm is not its
/// syntax, so it answers empty for every name — the parity is the claim, not
/// the mechanism). The `set -u` rows carry the actual distinction: an `OPTARG`
/// the silent scan **set** reads as usual at full parity, while reading the
/// unbound one after a flag is an **error** in both shells — bash reports its
/// message and aborts the script (rc 127 measured), fdshell reports its chain
/// and continues at rc 0 (the documented nounset layer); the unbound **state**
/// is the pin here.
#[test]
fn optargs_are_set_only_when_an_option_argument_was_found() {
    let (out, err, code) = run(r#"set -- -a; getopts "ab:c" o; builtin echo "[${OPTARG+set}]""#);
    assert_eq!(out, "[]\n", "the probe answers empty in bash too");
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);

    let (out, err, code) =
        run(r#"set -u; set -- -x; getopts ":ab:c" o; builtin echo "[$OPTARG]"; echo reached"#);
    assert_eq!(
        out, "[x]\nreached\n",
        "set and non-empty: reads fine, bash the same"
    );
    assert_eq!(err, "", "bash too");
    assert_eq!(code, 0);

    let (out, err, code) =
        run(r#"set -u; set -- -a; getopts "ab:c" o; builtin echo "[${OPTARG}]"; echo reached"#);
    assert!(
        err.contains("OPTARG: unbound variable"),
        "bash reports bash: line 1: OPTARG: unbound variable and aborts at rc 127"
    );
    assert_eq!(
        out, "reached\n",
        "fdshell reports and continues; bash aborts, printing nothing"
    );
    assert_eq!(
        code, 0,
        "the continuation is fdshell's documented nounset layer; bash measures 127"
    );
}
