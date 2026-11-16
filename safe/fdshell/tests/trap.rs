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
    assert_eq!(
        out, "trap -- 'echo y' EXIT\ntrap -- 'echo x' SIGUSR1\n",
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
