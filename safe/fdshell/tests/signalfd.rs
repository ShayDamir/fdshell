#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A pid + counter scratch dir `fdshell-signalfd-<tag>-<pid>-<counter>`, created
/// exclusively and removed when the test's guard drops, so the script file
/// written inside it dies with the test (LESSONS, "pid + counter is
/// collision-free only if").
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
    loop {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fdshell-signalfd-{tag}-{}-{c}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Ok(()) => return Scratch(dir),
            Err(e) => panic!("cannot create a private scratch dir {dir:?}: {e}"),
        }
    }
}

/// `signalfd %s USR1` creates an fd var that becomes readable when SIGUSR1 is
/// delivered to the shell. The script prints `ready` after creating the
/// signalfd, then waits for it to become readable.
#[test]
fn signalfd_catches_delivered_signal() {
    let script = "signalfd %s USR1; builtin echo ready; \
                  wait readable %s) builtin echo caught ;; done; unset %s";
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    // Wait for the signalfd to be created.
    let first = lines.next().unwrap().unwrap();
    assert_eq!(first, "ready", "first line must be the ready marker");

    // Deliver SIGUSR1 to the shell; the signalfd must catch it.
    let status = sys::signal::kill(sys::Pid::from_raw(pid as i32), sys::signalfd::SIGUSR1);
    assert!(status.is_ok(), "failed to send SIGUSR1: {status:?}");

    let second = lines.next().unwrap().unwrap();
    assert_eq!(
        second, "caught",
        "the signalfd must become readable on SIGUSR1"
    );

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr={:?}",
        str::from_utf8(&output.stderr)
    );
}

/// `signalfd` with no signals is a clean error (exit 1).
#[test]
fn signalfd_no_signals_errors() {
    let output = Command::new(BIN)
        .args(["-c", "signalfd %s"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let err = str::from_utf8(&output.stderr).unwrap();
    assert!(err.contains("signalfd"), "stderr={err}");
}

/// `signalfd` with an unknown signal is a clean error (exit 1).
#[test]
fn signalfd_bad_signal_errors() {
    let output = Command::new(BIN)
        .args(["-c", "signalfd %s FOO"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let err = str::from_utf8(&output.stderr).unwrap();
    assert!(err.contains("not a signal"), "stderr={err}");
}

/// A non-UTF-8 byte inside a user word is the user's own input, so at the
/// argument-parsing entry point it must name the bad argument (exit 1) and never
/// be reported as an internal state (LESSONS, "A NUL byte is reachable only at
/// the entry points that ingest outside bytes"). The byte is written into a
/// script file: a `-c` shell escape measures a different word path.
#[test]
fn signalfd_non_utf8_flag_value_names_the_argument() {
    let dir = scratch("utf8");
    let path = dir.join("sig.sh");
    std::fs::write(&path, b"signalfd %s USR1 --flags 0x\xff\n").unwrap();

    let output = Command::new(BIN)
        .arg(path.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    let err = str::from_utf8(&output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "stderr={err}");
    assert!(err.contains("not a valid flag"), "stderr={err}");
    assert!(err.contains("0x"), "stderr={err}");
    assert!(!err.contains("impossible"), "stderr={err}");
    assert!(!err.contains("should never occur"), "stderr={err}");
}
