#![allow(clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::str;

use sys::Pid;
use sys::signalfd::{SIGTERM, SIGUSR1};

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

/// Run `setup` in a spawned shell, wait for its `ready` line (so the registration
/// has happened), deliver `sig`, then poke stdin and collect the exit status.
///
/// What is observable **without** the deferred executor (task #585 does not run a
/// stored action yet) is survival: a shell that survives a signal whose default
/// action is terminate had the disposition installed, and one that dies of it did
/// not. Each row lives in its own process, so no test reads the dispositions of the
/// test process itself (LESSONS, task #582).
fn deliver(setup: &str, sig: i32) -> std::process::ExitStatus {
    let script = format!("{setup}; builtin echo ready; read x; builtin echo [{{x}}]");
    let mut child = Command::new(BIN)
        .args(["-c", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready = lines.next().unwrap().unwrap();
    assert_eq!(
        ready, "ready",
        "the registration must print its marker first"
    );

    sys::signal::kill(Pid::from_raw(child.id() as i32), sig).unwrap();

    // Poke stdin: an ignored signal leaves a blocked `read` blocked, so this is how
    // the shell gets to finish; a recorded one has already interrupted it (`sa_flags`
    // is 0 — no `SA_RESTART`), and a reset one never got here.
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"ping\n").ok();
    stdin.flush().ok();
    drop(stdin);
    child.wait_with_output().unwrap().status
}

// ---- registration and the list form ---------------------------------------

/// One pin covers the format, the quoting and the input grammar: the printed line
/// re-inputs as itself, so the second listing is byte-identical to the first
/// (`eval "$(trap)"` is the round trip POSIX promises, bash-measured quoting).
#[test]
fn the_list_form_re_inputs() {
    let (out, err, code) = run(r#"trap "echo a'b" USR1; trap; eval "$(trap)"; trap"#);
    assert_eq!(code, 0, "stderr={err:?}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "out={out:?} stderr={err:?}");
    assert_eq!(
        lines.first().copied(),
        Some("trap -- 'echo a'\\''b' SIGUSR1"),
        "out={out:?} stderr={err:?}"
    );
    assert_eq!(
        lines.get(1).copied(),
        lines.first().copied(),
        "the printed form must re-input"
    );
}

/// A shared action prints one line per condition, in the store's order — ascending
/// condition number, which puts `EXIT` first (bash 5.3.9, measured).
#[test]
fn one_action_prints_one_line_per_condition() {
    let (out, err, code) = run("trap 'echo x' USR1 INT; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out, "trap -- 'echo x' SIGINT\ntrap -- 'echo x' SIGUSR1\n",
        "the store's order is the print order (stderr={err:?})"
    );

    let (out, err, code) = run("trap 'echo y' EXIT; trap 'echo x' USR1; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    // bash 5.3.9 (measured): the two list lines, then the natural end runs the
    // EXIT action (task #585's executor).
    assert_eq!(
        out, "trap -- 'echo y' EXIT\ntrap -- 'echo x' SIGUSR1\ny\n",
        "EXIT comes first (stderr={err:?})"
    );
}

/// POSIX's null action **is** printed: an empty pair of quotes is the ignore form's
/// own line, not an absent entry (measured, and pinned in the unit tests too).
#[test]
fn the_ignore_form_prints_its_line() {
    let (out, err, code) = run("trap '' USR1; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "trap -- '' SIGUSR1\n");
}

/// The reset form prints nothing afterwards: the condition has no entry, and the
/// default prints no line (measured; and the reset is real, pinned below by the
/// signal that kills the shell again).
#[test]
fn a_reset_condition_prints_nothing() {
    let (out, err, code) = run("trap 'x' USR1; trap - USR1; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");

    // POSIX: a first operand that is a decimal integer makes every operand a
    // condition, and the action the default — `trap 0` resets `EXIT`.
    let (out, err, code) = run("trap 'x' 0; trap 0; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "", "the leading integer is a reset, not an action");
}

/// The `EXIT` slot registers without a `sigaction` — `sigaction` answers `EINVAL`
/// for `0`, so the entry is storage for the executor and nothing else (task #583).
#[test]
fn exit_registration_needs_no_sigaction() {
    let (out, err, code) = run("trap 'x' EXIT; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "trap -- 'x' EXIT\n");
}

// ---- the words go through the expansion pipeline --------------------------

/// Operands are expanded at registration (bash-measured): a `$name` value becomes
/// the action, a quoted glob character stays itself, and an escaped `$` is not
/// expanded. The stored bytes are printed verbatim, which is what makes the list
/// form re-inputtable.
#[test]
fn the_operands_are_expanded_like_any_command_words() {
    let (out, err, code) = run(r#"X="echo hi"; trap "$X" USR1; trap"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "trap -- 'echo hi' SIGUSR1\n");

    let (out, err, code) = run(r#"trap '*' USR1; trap"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "trap -- '*' SIGUSR1\n", "a quoted word is not globbed");

    let (out, err, code) = run(r#"trap "echo \$f" USR1; trap"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "trap -- 'echo $f' SIGUSR1\n");
}

/// `$_` is the last **expanded** field the command consumed — the rule the field
/// list the handler hands back exists for.
#[test]
fn trap_leaves_last_arg_to_its_last_operand() {
    let (out, err, code) = run("trap 'echo hi' USR1; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[USR1]\n");
}

// ---- errors: report, non-zero status, no abort ----------------------------

/// POSIX XCU 2.15: an invalid name or number is a non-zero status and must not
/// abort the shell (bash's measured numbers: 1 for a bad condition, 2 for usage).
#[test]
fn an_invalid_condition_is_reported_and_the_script_carries_on() {
    let (out, err, code) = run("trap 'echo x' BADSIG; echo rc=$?; echo continued");
    assert_eq!(code, 0, "the script must still run (stderr={err:?})");
    assert!(err.contains("BADSIG"), "stderr={err:?}");
    assert!(
        err.contains("invalid signal specification"),
        "stderr={err:?}"
    );
    assert_eq!(out, "rc=1\ncontinued\n", "bash's measured status is 1");
}

/// Conditions are processed one at a time: the good one is registered before the
/// bad one is reported (measured in bash, and the reason the grammar hands the
/// operands over as a list rather than resolving them all up front).
#[test]
fn a_bad_condition_does_not_undo_the_ones_before_it() {
    let (out, err, code) = run("trap 'x' USR1 BADSIG; echo rc=$?; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("BADSIG"), "stderr={err:?}");
    assert_eq!(out, "rc=1\ntrap -- 'x' SIGUSR1\n");
}

/// The usage forms (an action with nothing to apply it to) answer bash's 2, and the
/// script carries on.
#[test]
fn usage_is_reported_and_the_script_carries_on() {
    for (script, expected) in [
        ("trap -; echo rc=$?; echo continued", "rc=2\ncontinued\n"),
        (
            r#"trap 'x'; echo rc=$?; echo continued"#,
            "rc=2\ncontinued\n",
        ),
        (
            r#"trap ''; echo rc=$?; echo continued"#,
            "rc=2\ncontinued\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "stderr={err:?}");
        assert!(err.contains("usage"), "stderr={err:?}");
        assert_eq!(out, expected, "script={script:?} stderr={err:?}");
    }
}

/// SIGKILL and SIGSTOP are POSIX's "undefined results"; bash accepts them silently,
/// fdshell rejects them — a deliberate task decision, pinned for both spellings of
/// each (the unit tests pin the numbers).
#[test]
fn the_reserved_pair_is_rejected_here_not_ignored() {
    for name in ["KILL", "SIGKILL", "STOP", "SIGSTOP"] {
        let (out, err, code) = run(&format!("trap 'x' {name}; echo rc=$?; echo continued"));
        assert_eq!(code, 0, "the script must still run (stderr={err:?})");
        assert!(err.contains(name), "stderr={err:?}");
        assert!(err.contains("cannot be trapped"), "stderr={err:?}");
        assert_eq!(
            out, "rc=1\ncontinued\n",
            "bash accepts it silently; fdshell says 1"
        );
    }
}

/// The one signal the name table cannot spell is a real `sigaction` argument on
/// this machine and answers `EINVAL` (glibc keeps 32 for itself). bash answers 0
/// there; fdshell reports the syscall and answers non-zero — the divergence
/// task #586 documents.
#[test]
fn a_signal_glibc_reserves_reports_the_syscall() {
    let (out, err, code) = run("trap 'x' 32; echo rc=$?; echo continued");
    assert_eq!(
        code, 0,
        "the report does not abort the script (stderr={err:?})"
    );
    assert!(
        err.contains("failed to set the signal disposition"),
        "stderr={err:?}"
    );
    assert!(
        err.contains("Invalid argument"),
        "the errno stays in the chain: {err:?}"
    );
    assert_eq!(
        out, "rc=1\ncontinued\n",
        "bash answers 0 here, fdshell reports the syscall and says 1"
    );
}

/// POSIX XCU 2.15's entry rule: in a non-interactive shell a signal **ignored on
/// entry** cannot be trapped or reset, "although no error need be reported". The
/// set is not empty here the way bash's is — Rust's runtime ignores `SIGPIPE` — so
/// the refusal is observable: nothing is printed, and the status is 0.
#[test]
fn the_inherited_ignore_is_refused_silently() {
    let (out, err, code) = run("trap 'x' PIPE; trap; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "rc=0\n", "no line: the condition was refused");
    assert_eq!(err, "", "no error need be reported");

    let (out, err, code) = run("trap - PIPE; trap; echo rc=$?");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "rc=0\n", "a reset is refused the same way");
    assert_eq!(err, "");

    // A signal NOT ignored on entry is trapped normally, so the rule is a refusal
    // of one condition and not a shut-off of the builtin.
    let (out, err, code) = run("trap 'x' USR1; trap");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "trap -- 'x' SIGUSR1\n");
}

// ---- the dispositions are really installed --------------------------------

/// `Action::Record`: the shell survives SIGUSR1, whose default action terminates —
/// so the handler is installed (the body itself runs in task #585; a handled signal
/// interrupts a blocking `read`, which is what the shell reports here).
#[test]
fn a_registered_action_survives_the_signal() {
    let status = deliver("trap 'echo a' USR1", SIGUSR1);
    assert!(
        status.signal().is_none(),
        "the shell must survive a signal it traps; killed by {:?}",
        status.signal()
    );
}

/// `Action::Ignore`: SIGTERM is discarded, and the poked stdin lets the shell
/// finish (the ignore form is a disposition, not an entry in a table).
#[test]
fn the_ignore_form_survives_the_signal() {
    let status = deliver("trap '' TERM", SIGTERM);
    assert!(
        status.signal().is_none(),
        "an ignored signal must not kill the shell; killed by {:?}",
        status.signal()
    );
}

/// The reset arm calls `sigaction` too: after `trap - USR1` the shell dies of
/// SIGUSR1 again, exactly as an untrapped shell does — the pin that catches a reset
/// that only touches the table.
#[test]
fn a_reset_gives_the_signal_its_default_action_back() {
    let status = deliver("trap 'x' USR1; trap - USR1", SIGUSR1);
    assert_eq!(
        status.signal(),
        Some(SIGUSR1),
        "the disposition must be back to the kernel's own"
    );
}

/// The `EXIT` slot is table-only, so registering it must not disturb the process
/// disposition: the shell reaches its normal exit (status 0) instead of dying of a
/// signal.
#[test]
fn registering_exit_leaves_the_process_disposition_alone() {
    let status = deliver("trap 'echo a' EXIT", SIGUSR1);
    assert_eq!(
        status.signal(),
        Some(SIGUSR1),
        "`EXIT` is not a signal: SIGUSR1 keeps its default action"
    );
}

// ---- the deferred executor (task #585) -------------------------------------

/// Write `content` to a pid-uniqued script file and run it (the file mode,
/// task #291's entry), reporting stdout, stderr and the exit code.
fn run_script_file(tag: &str, content: &str) -> (String, String, i32) {
    let path = std::env::temp_dir().join(format!("fdshell-trap-{tag}-{}.sh", std::process::id()));
    std::fs::write(&path, content).unwrap();
    let output = Command::new(BIN)
        .arg(path.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Spawn the REPL (no `-c`), pipe `input` to stdin, close it, and capture
/// stdout/stderr/exit code (the shape of `tests/repl.rs::run_repl`).
fn run_repl(input: &str) -> (String, String, i32) {
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Spawn the REPL under a pty (`script -qec <bin>`, so `isatty(0)` is true),
/// write `input`, close the pty's stdin (that does deliver a pty EOF), settle,
/// and report stdout plus whether the shell was still alive (the shape of
/// `tests/repl.rs::run_tty` — sleep, `try_wait`, `kill` a live child, drain;
/// the pty echoes the input, so assertions `contains`, never exact equality).
fn run_tty(input: &str) -> (String, bool) {
    let mut child = Command::new("script")
        .args(["-qec", BIN, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    drop(child.stdin.take().unwrap());
    std::thread::sleep(std::time::Duration::from_millis(500));
    let alive = child.try_wait().unwrap().is_none();
    if alive {
        let _ = child.kill();
    }
    let output = child.wait_with_output().unwrap();
    (str::from_utf8(&output.stdout).unwrap().to_string(), alive)
}

/// The `deliver` shape extended to report what the shell printed (the plan's
/// pin 20: the signal lands while `read` blocks, the handler flags it, and the
/// boundary that follows the finished `read` runs the action — then the poke
/// lets the script finish, so the assertion is the printed action and status
/// 0 (bash runs the trap at the same boundary; its `read` returns 128+sig
/// instead of finishing — the shapes' statuses differ, the order is the same).
#[test]
fn a_signal_during_a_blocking_read_runs_the_action() {
    let mut child = Command::new(BIN)
        .args([
            "-c",
            "trap 'echo a' USR1; builtin echo ready; read x; builtin echo [{$x}]",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready = lines.next().unwrap().unwrap();
    assert_eq!(
        ready, "ready",
        "the registration must print its marker first"
    );
    sys::signal::kill(Pid::from_raw(child.id() as i32), SIGUSR1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    child.stdin.as_mut().unwrap().write_all(b"ping\n").unwrap();
    drop(child.stdin.take().unwrap());
    // The rest of stdout belongs to the reader that already took `ready`
    // (`wait_with_output` drains an empty pipe and loses buffered bytes). The
    // signal interrupts the blocked `read` (the handler runs first —
    // `sa_flags` is 0), and the boundary after the finished `read` runs the
    // action: `a`, then the script's own `[ping]` line (bash runs the trap at
    // the same point; its `read` answers 130 instead of finishing — the
    // statuses differ, the order is the same).
    let rest: Vec<String> = lines.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        rest,
        ["a", "[{ping}]"],
        "the action runs at the boundary after the read"
    );
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(0), "the script finishes");
}

// ---- the exit paths ----------------------------------------------------------------

/// The `exit` builtin runs the EXIT trap before the process ends, and the
/// requested status wins (bash 5.3.9, measured: `bye`, rc 3).
#[test]
fn exit_runs_the_exit_trap_with_the_requested_status() {
    let (out, err, code) = run("trap 'echo bye' EXIT; exit 3");
    assert_eq!(code, 3, "stderr={err:?}");
    assert_eq!(out, "bye\n", "stderr={err:?}");
}

/// The `-c` mode's natural end (status 0) runs the trap (measured: `hi`,
/// `bye`, rc 0).
#[test]
fn a_natural_end_at_zero_runs_the_exit_trap() {
    let (out, err, code) = run("trap 'echo bye' EXIT; echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\nbye\n", "stderr={err:?}");
}

/// A non-zero last status is what the shell exits with, trap printed first
/// (measured: `bye`, rc 1).
#[test]
fn a_natural_end_at_a_nonzero_status_keeps_it() {
    let (out, err, code) = run("trap 'echo bye' EXIT; false");
    assert_eq!(code, 1, "stderr={err:?}");
    assert_eq!(out, "bye\n", "stderr={err:?}");
}

/// A script file's EOF is an exit path (measured: `bye`, rc 0).
#[test]
fn script_eof_runs_the_exit_trap() {
    let (out, err, code) = run_script_file("eof", "trap 'echo bye' EXIT\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "bye\n", "stderr={err:?}");
}

/// The REPL's piped-EOF exit path runs the trap (measured against
/// `bash --noediting`: `bye`, rc 0; the prompts echo on stdout, hence
/// `contains`).
#[test]
fn repl_eof_on_a_pipe_runs_the_exit_trap() {
    let (out, err, code) = run_repl("trap 'echo bye' EXIT\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("bye\n"), "stdout={out:?} stderr={err:?}");
}

/// The interactive exit path, gated on `ignoreeof` (on for a tty, so the row
/// turns it off first — that is the whole point). The LESSONS tty shape:
/// `script -qec`, `contains` assertions only (the pty echoes the input).
#[test]
fn repl_eof_on_a_tty_runs_the_exit_trap() {
    let (out, alive) = run_tty("set +o ignoreeof\ntrap 'echo bye' EXIT\n");
    assert!(!alive, "the shell exits by itself after the trap: {out:?}");
    assert!(
        out.contains("bye"),
        "the trap runs at the EOF exit: {out:?}"
    );
}

// ---- the boundary and the `$?` rule --------------------------------------------------

/// The task's pin shape (measured, bash 5.3.9): the action runs at the
/// boundary after the `kill` command, and the shell's `$?` afterwards predates
/// the action — the `[ "$?" = 1 ]` test sees `false`'s status, so status 0
/// proves the restore.
#[test]
fn status_restore_after_a_signal_trap() {
    let (out, err, code) = run(r#"trap 'echo hit' USR1; kill -USR1 $$; false; [ "$?" = 1 ]"#);
    assert_eq!(code, 0, "stdout={out:?} stderr={err:?}");
    assert!(out.contains("hit"), "the action ran: {out:?}");
}

/// Inside the action, `$?` is the status the shell held when the action was
/// invoked (the status `kill` left: 0 — measured); the action's own commands
/// change it live (`false` → 1 inside); and the shell sees its pre-action
/// value again after the action (measured, bash 5.3.9, line for line).
#[test]
fn the_action_sees_the_pre_trap_status_and_restore_is_exact() {
    let (out, err, code) = run(
        r#"trap 'echo in-trap-rc=$?; false; echo post-rc=$?' USR1; kill -USR1 $$; echo rc-after-trap=$?; echo tail"#,
    );
    assert_eq!(code, 0, "stdout={out:?} stderr={err:?}");
    assert_eq!(
        out, "in-trap-rc=0\npost-rc=1\nrc-after-trap=0\ntail\n",
        "stderr={err:?}"
    );
}

/// The action runs through the in-process runner, so an assignment inside it
/// lands in the shell's own store (measured: `X=set`).
#[test]
fn the_action_runs_in_the_current_environment() {
    let (out, err, code) = run(r#"trap 'X=set' USR1; kill -USR1 $$; echo X=$X"#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "X=set\n", "stderr={err:?}");
}

/// An `exit` inside a running trap action exits with that status, the EXIT
/// trap running for the process's end (measured: `hit`, `bye`, rc 2).
#[test]
fn exit_inside_a_trap_action_exits_with_its_status() {
    let (out, err, code) =
        run("trap 'echo bye' EXIT; trap 'echo hit; exit 2' USR1; kill -USR1 $$; echo not-reached");
    assert_eq!(code, 2, "stdout={out:?} stderr={err:?}");
    assert_eq!(out, "hit\nbye\n", "stderr={err:?}");
}

/// The EXIT trap runs at most once per shell: an `exit` inside the action
/// ends the shell without re-entering the trap, and that status wins
/// (measured: `bye` once, rc 5).
#[test]
fn the_exit_trap_runs_at_most_once_per_shell() {
    let (out, err, code) = run("trap 'echo bye; exit 5' EXIT; exit 3");
    assert_eq!(code, 5, "stderr={err:?}");
    assert_eq!(out, "bye\n", "one line only: {out:?} stderr={err:?}");
}

/// An empty action is the ignore disposition: it consumes its signal and runs
/// nothing (the pin against the executor running `Ignore` entries).
#[test]
fn the_ignore_form_needs_no_execution() {
    let (out, err, code) = run("trap '' USR1; kill -USR1 $$; echo after");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "after\n", "the disposition did the work: {out:?}");
}

/// `0` and `EXIT` are the same slot (task #583/#584's key `0`), so registering
/// the numeric spelling reaches the same executor path (measured: `bye`, rc 3).
#[test]
fn zero_and_exit_are_the_same_slot_at_the_exit_path() {
    let (out, err, code) = run("trap 'echo bye' 0; exit 3");
    assert_eq!(code, 3, "stderr={err:?}");
    assert_eq!(out, "bye\n", "stderr={err:?}");
}

/// A bad action is reported and the shell carries on. bash reports it at
/// registration (`trap: line 1: syntax error …`, then `after`, rc 0); fdshell
/// stores the bytes verbatim (task #584's decision), so the report surfaces at
/// execution instead — same continuation, same status.
#[test]
fn a_bad_action_is_reported_and_the_shell_carries_on() {
    let (out, err, code) = run("trap 'echo hit &&' USR1; kill -USR1 $$; echo after");
    assert_eq!(code, 0, "stdout={out:?} stderr={err:?}");
    assert_eq!(out, "after\n");
    assert!(err.contains("parse error"), "stderr={err:?}");
    assert!(err.contains("expected command after"), "stderr={err:?}");
}

/// Loop control raised inside an action propagates through the boundary like
/// any other control the runner returns (measured: `loop-1`, `hit`,
/// `after-loop` — the `break` stops the loop).
#[test]
fn loop_control_raised_inside_an_action_propagates() {
    let (out, err, code) = run(
        r#"trap 'echo hit; break' USR1; for i in 1 2 3; do echo loop-$i; kill -USR1 $$; done; echo after-loop"#,
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "loop-1\nhit\nafter-loop\n", "stderr={err:?}");
}

// ---- the block-executor boundaries -------------------------------------------------

/// The plain `run.rs` boundary: a signal flagged by the command before it is
/// the next check.
#[test]
fn the_run_boundary_is_the_first_check() {
    let (out, err, code) = run("trap 'echo hit' USR1; kill -USR1 $$");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hit\n", "stderr={err:?}");
}

/// The `for_run` boundary: the signal is flagged inside the body (the
/// boundary inside the body's statement runs first — §4 — and the loop's own
/// merge line is next in line), so the action prints before the loop's tail.
#[test]
fn the_for_run_boundary_runs_the_pending_action() {
    let (out, err, code) =
        run("trap 'echo hit' USR1; for i in one; do kill -USR1 $$; done; echo tail");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hit\ntail\n", "stderr={err:?}");
}

/// The `case_exec` boundary, same shape through a clause body.
#[test]
fn the_case_exec_boundary_runs_the_pending_action() {
    let (out, err, code) =
        run("trap 'echo hit' USR1; case a in a) kill -USR1 $$;; esac; echo tail");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hit\ntail\n", "stderr={err:?}");
}

/// The `wait` boundary: the flag is set during the assignment's substitution
/// (the handler runs when the shell's `waitid` is interrupted — the resume
/// path, §5's syscall-wake rule — and the assignment has no boundary of its
/// own), the poll times out into its `after` arm, and the round's boundary
/// line runs the action. The action prints **twice**: the arm child inherits
/// the flagged pending mask through its fork (the mask was set before the
/// dispatch) and runs the action at its own boundary, and the parent's round
/// line runs it from the parent's copy (the plan's pins 16/17 substitute
/// shapes measured unstable across this mechanism; the direct-kill rows above
/// are the deterministic boundary pins).
#[test]
fn the_wait_boundary_runs_the_pending_action() {
    let (out, err, code) = run(
        "trap 'echo hit' USR1; signalfd %v PIPE; X=$(kill -USR1 $$; echo); \
         wait readable %v) echo armed;; after 50) echo ready;; done; echo tail",
    );
    assert_eq!(code, 0, "stdout={out:?} stderr={err:?}");
    assert_eq!(out, "ready\nhit\nhit\ntail\n", "stderr={err:?}");
}

/// A signal arriving during a **blocking** poll is `wait`'s own report, not a
/// trap (plan §5.7: the poll's reporting stays as it is — `sa_flags == 0`
/// means the `poll` answers `EINTR` after the handler flags). bash runs the
/// trap first and reports `128+sig` (measured 138); fdshell flags the bit,
/// reports its own `poll failed` chain, exits 1, and nothing runs — the gap
/// predates task #585 and is not silently fixed here.
#[test]
fn a_signal_during_the_blocking_poll_reports_its_own_chain() {
    let mut child = Command::new(BIN)
        .args([
            "-c",
            "trap 'echo hit' USR1; signalfd %s PIPE; builtin echo ready; \
             wait readable %s) echo fired;; done",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready = lines.next().unwrap().unwrap();
    assert_eq!(
        ready, "ready",
        "the registration must print its marker first"
    );
    // Settle first: the signal must land while the shell is blocked in the
    // `poll` (the kernel delivers pending signals at syscall exits — a signal
    // that arrives during userspace time flags the executor's static but never
    // interrupts the `poll`, and the block then waits forever).
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys::signal::kill(Pid::from_raw(child.id() as i32), SIGUSR1).unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = str::from_utf8(&output.stdout).unwrap().to_string();
    let stderr = str::from_utf8(&output.stderr).unwrap().to_string();
    assert!(
        !stdout.contains("hit"),
        "the trap does not run first: {stdout:?}"
    );
    assert!(!stdout.contains("fired"), "no arm ran: {stdout:?}");
    assert!(stderr.contains("poll failed"), "stderr={stderr:?}");
    assert!(stderr.contains("errno 4"), "the EINTR chain: {stderr:?}");
    assert_eq!(output.status.code(), Some(1), "stderr={stderr:?}");
}
