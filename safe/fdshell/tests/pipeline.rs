#![allow(clippy::unwrap_used)]

use std::fs::File;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::str;
use std::time::{Duration, Instant};
use sys::fcntl::{LOCK_EX, LOCK_UN, O_CLOEXEC, O_RDWR};
use sys::openat2::open;
use sys::signal::{SIGKILL, kill};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir with a regular `lock` file; removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fdshell-pipeline-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lock"), b"").unwrap();
        Self(dir)
    }
}

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn run(dir: &Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .output()
        .unwrap()
}

/// All pids whose parent is `parent`.
fn children_of(parent: u32) -> Vec<u32> {
    let mut pids = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap() {
        let name = entry.unwrap().file_name();
        let Ok(pid) = name.to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // "pid (comm) state ppid …" — comm may contain spaces and parens.
        let fields = stat.rsplit(')').next().unwrap_or("");
        let ppid = fields
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok());
        if ppid == Some(parent) {
            pids.push(pid);
        }
    }
    pids
}

/// A child that still runs the fdshell binary — a forked stage that has not
/// exec'd yet (a builtin stage).
fn is_fdshell(pid: u32) -> bool {
    let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    cmdline.starts_with(BIN.as_bytes())
}

/// `(fd number, readlink target)` pairs of a live process.
fn fd_table(pid: u32) -> Vec<(u32, String)> {
    let mut table = Vec::new();
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return table;
    };
    for entry in entries.flatten() {
        let fd = entry.file_name().to_string_lossy().parse().unwrap_or(0);
        let Ok(target) = std::fs::read_link(entry.path()) else {
            continue;
        };
        table.push((fd, target.to_string_lossy().into_owned()));
    }
    table
}

/// `"pipe:[<inode>"]` targets of the pipes this test process itself holds.
/// Pipeline stages inherit those fds (e.g. a harness jobserver pipe), so the
/// fd-table assertions must exclude them: only pipes this process did not
/// open are pipeline pipes.
fn self_pipe_targets() -> Vec<String> {
    let mut targets = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc/self/fd") else {
        return targets;
    };
    for entry in entries.flatten() {
        let Ok(target) = std::fs::read_link(entry.path()) else {
            continue;
        };
        let target = target.to_string_lossy().into_owned();
        if target.starts_with("pipe:[") {
            targets.push(target);
        }
    }
    targets
}

/// The pipe entries of `table` that are not inherited from the test process.
fn own_pipes(table: &[(u32, String)], inherited: &[String]) -> Vec<(u32, String)> {
    table
        .iter()
        .filter(|(_, t)| t.starts_with("pipe:[") && !inherited.contains(t))
        .cloned()
        .collect()
}

/// True if `table` is a builtin stage with no pipeline fds left: exactly two
/// pipeline pipe entries (the dup2'd stdin and its redirect clone) and no
/// sibling pidfds — the steady state of the blocked stage under test.
fn stage_fds_clean(table: &[(u32, String)], inherited: &[String]) -> bool {
    let own = own_pipes(table, inherited);
    own.len() == 2
        && own.iter().any(|(fd, _)| *fd == 0)
        && table
            .iter()
            .all(|(_, t)| !t.starts_with("anon_inode:[pidfd]"))
}

/// A builtin stage never execs, so without the cleanup it would keep every
/// inherited pipeline pipe end (notably the upstream write ends) and every
/// sibling pidfd open. Hold the lock from this process so the builtin stage
/// blocks (no external holder to race), then inspect its fd table: only its
/// two redirect clones may be pipeline pipes, no pidfds.
#[test]
fn builtin_stage_does_not_hold_sibling_fds() {
    let dir = Scratch::new();
    // Hold the lock from this process: `builtin flock %f` blocks while
    // `lock_fd` is open.
    let cpath = std::ffi::CString::new(dir.join("lock").to_str().unwrap()).unwrap();
    let lock_fd = open(cpath.as_c_str(), O_RDWR | O_CLOEXEC).unwrap();
    lock_fd.flock(LOCK_EX).unwrap();
    // The stages inherit the pipes this process holds (harness jobserver
    // pipes etc.); exclude them from the fd-table assertions.
    let inherited = self_pipe_targets();
    let mut shell = Command::new(BIN)
        .args([
            "-c",
            "builtin openat2 --flags O_RDWR lock %>%f; true | builtin flock %f",
        ])
        .current_dir(&*dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // The stage-1 builtin blocks on the lock until this test ends. Pick the
    // shell's child that is still the fdshell binary and whose fd table
    // already satisfies the full assertions below: exactly two pipeline pipe
    // entries (one of them fd 0) and no pidfds. Only the blocked stage's
    // steady state does — transient states (before close_inherited ran) and
    // stage 0 (which keeps four pipeline pipe refs of its own) never match,
    // so they are simply re-polled.
    let deadline = Instant::now() + Duration::from_secs(9);
    let table = loop {
        let hit = children_of(shell.id()).into_iter().find_map(|pid| {
            let table = fd_table(pid);
            (is_fdshell(pid) && stage_fds_clean(&table, &inherited)).then_some(table)
        });
        if let Some(table) = hit {
            break table;
        }
        if Instant::now() > deadline {
            let _ = shell.kill();
            panic!("no blocked builtin stage with a clean fd table within 9 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let own = own_pipes(&table, &inherited);
    let pidfds: Vec<_> = table
        .iter()
        .filter(|(_, t)| t.starts_with("anon_inode:[pidfd]"))
        .collect();
    assert_eq!(own.len(), 2, "pipeline pipe ends leaked: {table:?}");
    assert!(
        own.iter().any(|(fd, _)| *fd == 0),
        "fd 0 must be the stage stdin pipe: {table:?}"
    );
    assert!(pidfds.is_empty(), "sibling pidfds leaked: {table:?}");

    let _ = shell.kill();
    let _ = shell.wait();
    // Release the lock so the orphaned stage (reparented to init) can finish.
    let _ = lock_fd.flock(LOCK_UN);
}

/// A builtin middle stage must not prevent the downstream external stage
/// from seeing EOF: the pipeline must complete (no deadlock) with exit 0.
/// `true` passes no data through, so `cat` prints nothing.
#[test]
fn builtin_middle_stage_pipeline_completes() {
    let dir = Scratch::new();
    let out = run(&dir, "echo a | builtin true | cat");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "");
}

/// A builtin stage that captures (e.g. `builtin memfd %>%m`) must keep its own
/// capture socketpair: `close_inherited` closes every *sibling* pair but skips
/// the stage's own (the `j == i` guard). Flipping that guard closes the stage's
/// own socket, so the capture `try_clone` hits a closed fd and the pipeline
/// fails with EBADF.
#[test]
fn builtin_stage_keeps_own_capture_pair() {
    let dir = Scratch::new();
    let out = run(&dir, "true | builtin memfd %>%m");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// Stage 0 must keep the shell's stdin, not the first pipe's read end.
/// `builtin statx /proc/self/fd/0` reports what fd 0 points at without
/// reading it: the fed-in file (`kind=file size=6`) when the wiring is
/// right, the first pipe (`kind=fifo size=0`) when stage 0 was handed its
/// own output pipe — the state that deadlocks `cat | cat`.
#[test]
fn first_stage_keeps_shell_stdin() {
    let dir = Scratch::new();
    let inp = dir.join("in");
    std::fs::write(&inp, b"hello\n").unwrap();
    let out = Command::new(BIN)
        .current_dir(&*dir)
        .args(["-c", "builtin statx /proc/self/fd/0 | cat"])
        .stdin(Stdio::from(File::open(&inp).unwrap()))
        .output()
        .unwrap();
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
    assert!(
        stdout.contains("kind=file size=6"),
        "stage 0 stdin is not the shell's stdin: {stdout:?}"
    );
}

/// A later stage still reads the previous stage's pipe: stage 1's fd 0 is a
/// fifo even though stage 0 now keeps the shell's stdin.
#[test]
fn later_stage_still_reads_the_pipe() {
    let dir = Scratch::new();
    let inp = dir.join("in");
    std::fs::write(&inp, b"hello\n").unwrap();
    // `builtin statx /proc/self/fd/0` as the *second* stage: its stdin must
    // be `pipes[0]`, a fifo.
    let out = Command::new(BIN)
        .current_dir(&*dir)
        .args(["-c", "builtin echo x | builtin statx /proc/self/fd/0 | cat"])
        .stdin(Stdio::from(File::open(&inp).unwrap()))
        .output()
        .unwrap();
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
    assert!(
        stdout.contains("kind=fifo"),
        "stage 1 stdin is not the pipe: {stdout:?}"
    );
}

/// Run `script` with `stdin` fed from a pipe, giving up after `limit`.
/// `cat | cat` deadlocks when stage 0's stdin is the first pipe's read end
/// (nothing ever writes to it), so the shell and every stage still alive are
/// SIGKILLed before the panic — otherwise the deadlock would hang the suite.
fn run_bounded(script: &str, stdin: &str, limit: Duration) -> Output {
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(stdin.as_bytes()).unwrap();
    drop(pipe); // EOF, so a correctly wired stage 0 sees end of input
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => panic!("try_wait failed: {e}"),
        }
        if Instant::now() >= deadline {
            for pid in children_of(child.id()) {
                let _ = kill(sys::Pid::from_raw(pid as i32), SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!("`{script}` did not finish in {limit:?} — a pipeline stage's stdin is miswired");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn cat_pipe_to_cat_completes() {
    // 5 s: a correctly wired pipeline finishes in milliseconds, while the
    // bound stays under nextest's 10 s slow-timeout so the diagnostic panic
    // (not a bare timeout) reports a miswired stdin.
    let out = run_bounded("cat | cat", "yo\n", Duration::from_secs(5));
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={:?}",
        str::from_utf8(&out.stderr)
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "yo\n");
}
