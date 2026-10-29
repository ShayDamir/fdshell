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

/// Task #292: a keyword block written as a pipeline **stage** is a command — the
/// word behind `|` is in command position — so it runs in the forked stage like
/// any other stage. A block stage sits in the middle of a pipeline and forwards
/// the pipe, and text written behind its closer is the *next statement*, not
/// part of the block (`; echo tail`, `&& echo y`). bash 5.3.9 parity on every
/// row; on `master` the block's words were the previous stage's arguments and
/// the loop never ran.
#[test]
fn block_stage_runs_and_forwards_the_pipe() {
    let dir = Scratch::new();
    for (script, want) in [
        ("echo a | while :; do cat; break; done | cat", "a\n"),
        ("echo a | while :; do echo b; break; done | cat", "b\n"),
        (
            "echo a | while :; do echo b; break; done; echo tail",
            "b\ntail\n",
        ),
        (
            "echo a | while :; do echo b; break; done && echo y",
            "b\ny\n",
        ),
        ("echo a | if :; then cat; fi | cat", "a\n"),
        ("echo a | for i in 1 2; do echo $i; done | cat", "1\n2\n"),
    ] {
        let out = run(&dir, script);
        assert_eq!(
            (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
            (want, Some(0)),
            "{script}: stderr={}",
            str::from_utf8(&out.stderr).unwrap()
        );
    }
}

/// A redirect written **inside** a block stage takes effect in the fork (the
/// stage applies its own redirects), so the file is created and holds the data.
#[test]
fn a_block_stages_own_redirect_writes_the_file() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "echo a | while :; do echo b > out.txt; break; done; cat out.txt",
    );
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("b\n", Some(0)),
        "stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(std::fs::read(dir.join("out.txt")).unwrap(), b"b\n");
}

/// Task #292 reaches the trailing-redirect family of task **#298** in its
/// pipeline-stage form: a redirection written after a block's closer belongs to
/// the whole block, so the stage's output lands in the file and stdout stays
/// empty — bash's answer. Before #298 the stage's scan ended at the closer,
/// `> out.txt` was not the stage's redirect, the file was never created and the
/// output went to stdout (rc 0, no diagnostic).
#[test]
fn a_redirect_after_a_block_closer_belongs_to_the_block_stage() {
    let dir = Scratch::new();
    let out = run(&dir, "echo a | while :; do echo b; break; done > out.txt");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("", Some(0)),
        "stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(std::fs::read(dir.join("out.txt")).unwrap(), b"b\n");
}

/// A capture fd var works inside a block stage: the stage's write lands in the
/// memfd the parent created (the fork keeps the stage's own capture pair), and
/// the capture's size is the bytes the stage wrote.
#[test]
fn a_block_stage_writes_into_a_capture() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin memfd %>%m; echo a | while :; do echo b >&%m; break; done; builtin statx %m",
    );
    let stdout = str::from_utf8(&out.stdout).unwrap().to_string();
    assert_eq!(out.status.code(), Some(0), "stderr={:?}", out.stderr);
    assert!(
        stdout.contains("kind=file size=2"),
        "the stage's `b\\n` must reach the capture: {stdout}"
    );
}

/// A redirect naming a fd var that is **not set** fails inside the block stage,
/// and the failure carries its own context (`StageFailed`) on top of the
/// redirect's report: the chain names the compound command, the launch and the
/// unset fd var, rc 1.
#[test]
fn a_block_stage_failure_reports_its_own_context() {
    let dir = Scratch::new();
    let out = run(&dir, "echo a | while :; do echo b >&%nope; break; done");
    let err = str::from_utf8(&out.stderr).unwrap().to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(
        err.contains("compound command in a pipeline stage failed")
            && err.contains("fd variable 'nope' is not set"),
        "{err}"
    );
}

/// `time` behind the `|` is the stage's **word 0**, not a transparent prefix of
/// the block's keyword: the fork execs the external `time` found on the `PATH`,
/// which cannot run `while`, so the loop body never runs and the pipeline
/// reports a failure (the exact rc is the external `time`'s — 127 on a host with
/// `/usr/bin/time`, 127/126 there — bash's is 2, a syntax error). The `&&` twin
/// (`true && time while …`) runs the loop, where the prefix IS transparent —
/// pinned in `tests/cond_blocks.rs`.
#[test]
fn time_behind_the_pipe_is_the_stages_word_zero() {
    let dir = Scratch::new();
    let out = run(&dir, "echo a | time while :; do echo b; break; done");
    assert_eq!(
        str::from_utf8(&out.stdout).unwrap(),
        "",
        "the stage's loop must not run: stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_ne!(out.status.code(), Some(0), "bash rejects the line (rc 2)");
}

/// A here-document inside a block stage is fed to the stage's command in the
/// fork: the body arrives (bash parity), and a function definition as a stage
/// dies with the fork, so the name is unknown afterwards (fdshell's missing
/// command is rc 1, bash's is 127).
#[test]
fn a_heredoc_and_a_function_definition_inside_a_block_stage() {
    let dir = Scratch::new();
    let out = run(&dir, "echo a | while :; do cat <<T; break;\nbody\nT\ndone");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("body\n", Some(0)),
        "stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    let out = run(&dir, "echo a | f() { echo b; }; echo tail");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("tail\n", Some(0)),
        "bash: the stage's fork defines `f`, the parent never sees it"
    );
    let out = run(&dir, "echo a | f() { echo b; }; f");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("", Some(1)),
        "bash: rc 127 `f: command not found`"
    );
}

/// Task #345: the word behind a pipeline `|` is the stage's word 0, and a
/// here-doc operator there is the stage's **only** redirection: POSIX's
/// `simple_command = REDIRECT… [word…]` needs no command word, so the body
/// feeds the empty stage's stdin, the stage exits 0 and the pipeline is silent.
/// The pipe **split** rule is untouched (the shape is a pipeline: the first
/// stage's stdout is a pipe nobody reads), and the io-number form behaves the
/// same. bash 5.3.9 rc 0, no output, on every row.
#[test]
fn heredoc_only_stage_is_a_redirect_only_stage() {
    let dir = Scratch::new();
    for (script, want) in [
        ("true | <<EOF\nbody\nEOF\necho done", "done\n"),
        ("cat | <<EOF\nbody\nEOF", ""),
        ("3<<EOF\nbody\nEOF", ""),
        ("<<EOF | cat\nbody\nEOF", ""),
        ("true | 3<<EOF\nbody\nEOF\necho done", "done\n"),
    ] {
        let out = run(&dir, script);
        assert_eq!(
            (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
            (want, Some(0)),
            "script {script:?}: stderr={}",
            str::from_utf8(&out.stderr).unwrap()
        );
    }
}

// ── A user function written as a stage (task #481) ───────────────────────────
// A stage's name resolves through the same order a plain command uses: the
// function first, then the builtin table, then `PATH`. Every row below was
// measured against bash 5.3.9 and `target/debug/fdshell` with stdin
// `/dev/null`; the assertions state **fdshell's** value with bash's named in the
// message, and the group-shape members of the family stay parse errors owned by
// tasks #190/#186 (`tests/group.rs`).

/// `run` with stdin from `/dev/null`: some rows below have a stage that **reads**
/// stdin (`cat | f`), so the harness's stdin must not leak into them.
fn run_stdin_null(dir: &Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// A function in stage position runs, in the stage's own fork: as the first
/// stage, as the last one, in both at once, with arguments reaching the body's
/// positionals, with a scoped `NAME=value` prefix, and backgrounded. Every row
/// is bash parity (before #481 the stage fell to the `PATH` search and reported
/// `failed to resolve command path: "f"`).
#[test]
fn a_function_stage_runs_the_function() {
    let dir = Scratch::new();
    for (script, want) in [
        ("f(){ echo hi; }; f | cat", "hi\n"),
        ("f(){ echo hi; }; cat | f", "hi\n"),
        ("f(){ echo a; }; g(){ cat; }; f | g", "a\n"),
        ("f(){ echo \"$1\"; }; f x | cat", "x\n"),
        ("V=x; f(){ echo \"$V $1\"; }; V=y f z | cat", "y z\n"),
        ("f(){ echo hi; }; f | cat & wait; echo done", "hi\ndone\n"),
    ] {
        let out = run_stdin_null(&dir, script);
        assert_eq!(
            (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
            (want, Some(0)),
            "script {script:?}: bash prints `{want}` at rc 0; stderr={}",
            str::from_utf8(&out.stderr).unwrap()
        );
    }
}

/// The reported row: the stage's **own** redirections apply on both sides of the
/// pipe — `z` gets the function's line, `g` gets what `cat` reads from the pipe
/// (nothing: the function wrote to `z`, not to stdout). Before #481 both files
/// were empty, because no stage ran at all.
#[test]
fn a_function_stage_keeps_its_own_redirects() {
    let dir = Scratch::new();
    let out = run_stdin_null(&dir, "f(){ echo hi; }; f > z | cat > g");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("", Some(0)),
        "bash: no stdout, rc 0; stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(
        std::fs::read(dir.join("z")).unwrap(),
        b"hi\n",
        "bash: `hi\\n`"
    );
    assert_eq!(std::fs::read(dir.join("g")).unwrap(), b"", "bash: empty");
}

/// The status a stage function reports: its own last status (the last stage's
/// status is the pipeline's). A `break`/`continue` the body raises, and the
/// depth cap a recursive body hits, die with the fork — only the status is
/// reported, as `pipeline/child/block.rs` documents for a block stage. bash
/// exempts every stage but the last from `set -e` (task **#490**); that gap is
/// not visible here, where the failing function is the last stage in one row and
/// the pipeline status is the last stage's in the other — both rows are parity.
#[test]
fn a_function_stage_reports_the_status_it_runs_with() {
    let dir = Scratch::new();
    for (script, want) in [
        ("f(){ false; }; cat | f; echo rc=$?", "rc=1\n"),
        ("f(){ false; }; f | cat; echo rc=$?", "rc=0\n"),
        ("f(){ exit 3; }; cat | f; echo rc=$?", "rc=3\n"),
        ("f(){ f; echo x; }; f | cat", ""),
    ] {
        let out = run_stdin_null(&dir, script);
        assert_eq!(
            (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
            (want, Some(0)),
            "script {script:?}: bash prints `{want}` at rc 0; stderr={}",
            str::from_utf8(&out.stderr).unwrap()
        );
    }
}

/// The stage's fork is a **subshell**: the function's `cd` and its assignment
/// die with the child, so the parent's working directory and its `V` are
/// untouched — bash behaves the same.
#[test]
fn a_function_stages_state_changes_die_with_the_fork() {
    let dir = Scratch::new();
    let out = run_stdin_null(&dir, "f(){ cd /tmp; echo in; }; f | cat; echo pwd=$(pwd)");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        (format!("in\npwd={}\n", dir.display()).as_str(), Some(0)),
        "bash: `in` then the scratch dir (the fork's `cd` died with it); stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    let out = run_stdin_null(&dir, "f(){ V=1; }; f | cat; echo V=${V:-unset}");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("V=unset\n", Some(0)),
        "bash: `V=unset` at rc 0 (the fork's assignment died with it)"
    );
}

/// The function arm sits **after** `child_main`'s own work: the trace names the
/// call and the body's command, and the scoped-prefix/substitution steps have
/// already run (the data rows above pin those). The rows assert the two trace
/// lines as members — the pipeline's stages write concurrently, so their order
/// is not the assertion, and fdshell additionally traces its own `set -x`
/// (a divergence pinned in `tests/xtrace.rs`).
#[test]
fn a_function_stage_traces_the_call() {
    let dir = Scratch::new();
    let out = run_stdin_null(&dir, "set -x; f(){ echo hi; }; f | cat");
    let err = str::from_utf8(&out.stderr).unwrap().to_string();
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("hi\n", Some(0)),
        "bash prints `hi` at rc 0; stderr={err}"
    );
    assert!(err.contains("+ f") && err.contains("+ echo hi"), "{err}");
}

/// The prefix rows stay where they were: `builtin` and `command` bypass functions
/// (POSIX #2.4), so the stage falls to their own answer — bash reports on stderr
/// at rc 0 (`f: command not found`, `builtin: f: not a shell builtin`), fdshell
/// reports its own chain at rc 0.
#[test]
fn a_function_stage_keeps_the_prefix_rows() {
    let dir = Scratch::new();
    for (script, want) in [
        ("f(){ echo hi; }; command f | cat", "not found"),
        ("f(){ echo hi; }; builtin f | cat", "not a shell builtin"),
    ] {
        let out = run_stdin_null(&dir, script);
        let err = str::from_utf8(&out.stderr).unwrap().to_string();
        assert_eq!(
            (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
            ("", Some(0)),
            "script {script:?}: bash is silent on stdout at rc 0, naming `{want}` on stderr"
        );
        assert!(err.contains(want), "{script:?}: stderr={err}");
    }
}
