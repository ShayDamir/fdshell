#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
const EXEC_OK: &str = env!("CARGO_BIN_EXE_exec_ok");

/// The content the sha256/sha512 enable tests pin; its file digest is a pure
/// function of (content, algo, block size, salt), stable across kernels.
const CONTENT: &[u8] = b"fdshell-verity-e2e\n";
const SHA256_DIGEST: &str = "512eb3c1830461d829fbe3f6b07b2c40bc49114586b3389dad43f8b195abf57b";
const SHA512_DIGEST: &str = "e296f654d46935c2afbd9b7976d08e3299fce789d7572aea31b1372d5d3f6029cf89cf602efbf1c677d952a41463a1549a1c342abd07636faca4651d15f3e976";

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fdshell-verity-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Load-dependent errnos. `FS_IOC_ENABLE_VERITY` is kernel Merkle-tree plus
/// fs-transaction work (btrfs metadata reservations, kernel allocations) and
/// `openat2` races the system-wide fd table, so under a parallel suite run
/// (measured: `cargo mutants -j4`) these fail *transiently*. The shell
/// surfaces a syscall errno as the exit code (`child/error.rs`), so they
/// arrive as exactly these rc values. None of them is an rc any test in this
/// file expects, so retrying them can never mask a pin.
const TRANSIENT: [i32; 7] = [
    sys::errno::EAGAIN,  //  11 fork/exec and child memory pressure
    sys::errno::EIO,     //   5 fs-level transient
    sys::errno::ENOMEM,  //  12 kernel tree-build allocation pressure
    sys::errno::EBUSY,   //  16 fs busy
    sys::errno::ENFILE,  //  23 system-wide open fd pressure (openat2)
    sys::errno::ETXTBSY, //  26 shared target binary (concurrent rebuild)
    sys::errno::ENOSPC,  //  28 btrfs metadata reservation under load
];

/// Bounded retry budget for [`run`]: [`RETRIES`] retries at [`BACKOFF`] plus
/// the final un-retried attempt — 9 attempts, 8 × 10 ms sleeps, ~80 ms
/// worst case per call. A host that keeps failing past the budget fails hard.
const RETRIES: usize = 8;
const BACKOFF: std::time::Duration = std::time::Duration::from_millis(10);

fn spawn(dir: &std::path::Path, script: &str) -> Result<Output, std::io::Error> {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .output()
}

fn is_transient(out: &Output) -> bool {
    out.status.code().is_some_and(|rc| TRANSIENT.contains(&rc))
}

/// Run `script` in `dir`, absorbing only the *load-dependent* failures of a
/// parallel suite: a spawn that fails outright (fork `EAGAIN` under memory
/// pressure, `ETXTBSY` on a concurrently rebuilt binary — std reports both as
/// an `io::Error`, never as an rc) and the [`TRANSIENT`] rc set, each retried
/// after [`BACKOFF`]. Any other rc (including 0) returns immediately, so the
/// pinned rc assertions are untouched; when the budget is exhausted the last
/// output is returned, so the asserting test names the errno and its stderr
/// instead of hiding it. Retrying a failed enable is safe: the kernel marks a
/// file verity only in its final success step, so a [`TRANSIENT`] failure
/// leaves the file non-verity and the repeat re-runs the first-enable path.
fn run(dir: &std::path::Path, script: &str) -> Output {
    for _ in 0..RETRIES {
        match spawn(dir, script) {
            Ok(out) if is_transient(&out) => {
                eprintln!("transient rc {:?}, retrying: {script}", out.status.code());
                std::thread::sleep(BACKOFF);
            }
            Ok(out) => return out,
            Err(e) => {
                eprintln!("spawn failed ({e}), retrying: {script}");
                std::thread::sleep(BACKOFF);
            }
        }
    }
    spawn(dir, script).unwrap()
}

fn stdout(out: &Output) -> String {
    str::from_utf8(&out.stdout).unwrap().to_string()
}

fn stderr(out: &Output) -> String {
    str::from_utf8(&out.stderr).unwrap().to_string()
}

/// Run a script that enables verity on the scratch file: `None` (skip) when
/// the host fs/kernel lacks fs-verity (rc 25 ENOTTY or 95 EOPNOTSUPP), the
/// output when rc 0. Any other rc, after [`run`]'s transient retries, is a
/// hard failure and names it. The one skip rule for every enable-bearing
/// script in this file.
fn enable_run(dir: &std::path::Path, script: &str) -> Option<Output> {
    let out = run(dir, script);
    match out.status.code() {
        Some(0) => Some(out),
        Some(25) | Some(95) => {
            eprintln!(
                "skipping verity test: fs-verity unsupported (rc={:?})",
                out.status.code()
            );
            None
        }
        other => panic!("enable failed: rc={other:?} stderr={}", stderr(&out)),
    }
}

/// Enable verity on `name` with extra `flags`; `false` means skip (see
/// [`enable_run`]).
fn enable_with(dir: &std::path::Path, name: &str, flags: &str) -> bool {
    let script = format!(
        "builtin openat2 --flags O_RDONLY {name} %>%b; \
                          builtin verity %b --enable{flags}"
    );
    enable_run(dir, &script).is_some()
}

fn enable(dir: &std::path::Path, name: &str) -> bool {
    enable_with(dir, name, "")
}

/// Query a non-verity file: prints `enabled=no`, rc 0.
#[test]
fn query_non_verity() {
    let dir = Scratch::new();
    std::fs::write(dir.join("plain"), CONTENT).unwrap();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; builtin verity %b",
    );
    assert_eq!(out.status.code(), Some(0), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "enabled=no\n");
}

/// Enable (sha256) then query: the pinned file digest is printed.
#[test]
fn enable_and_query_sha256() {
    let dir = Scratch::new();
    std::fs::write(dir.join("f"), CONTENT).unwrap();
    if !enable(&dir, "f") {
        return;
    }
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f %>%b; builtin verity %b",
    );
    assert_eq!(out.status.code(), Some(0), "stderr={}", stderr(&out));
    assert_eq!(
        stdout(&out),
        format!("enabled=yes algo=sha256 digest={SHA256_DIGEST}\n")
    );
}

/// Enable with `--algo sha512` then query: the pinned sha512 digest is printed.
#[test]
fn enable_and_query_sha512() {
    let dir = Scratch::new();
    std::fs::write(dir.join("f"), CONTENT).unwrap();
    if !enable_with(&dir, "f", " --algo sha512") {
        return;
    }
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f %>%b; builtin verity %b",
    );
    assert_eq!(
        stdout(&out),
        format!("enabled=yes algo=sha512 digest={SHA512_DIGEST}\n")
    );
}

/// `--digest` check: rc 0 for the pinned digest, rc 1 for a wrong or
/// non-verity file (fail-closed).
#[test]
fn digest_check() {
    let dir = Scratch::new();
    std::fs::write(dir.join("f"), CONTENT).unwrap();
    std::fs::write(dir.join("plain"), CONTENT).unwrap();
    if !enable(&dir, "f") {
        return;
    }
    let ok = run(
        &dir,
        &format!(
            "builtin openat2 --flags O_RDONLY f %>%b; \
                  builtin verity %b --digest {SHA256_DIGEST}"
        ),
    );
    assert_eq!(ok.status.code(), Some(0), "stderr={}", stderr(&ok));
    let wrong = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f %>%b; \
         builtin verity %b --digest 0000",
    );
    assert_eq!(wrong.status.code(), Some(1), "stderr={}", stderr(&wrong));
    let plain = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; \
         builtin verity %b --digest 0000",
    );
    assert_eq!(plain.status.code(), Some(1), "stderr={}", stderr(&plain));
}

/// Enabling verity twice on the same inode exits with EEXIST (17).
#[test]
fn double_enable_is_eexist() {
    let dir = Scratch::new();
    std::fs::write(dir.join("f"), CONTENT).unwrap();
    if !enable(&dir, "f") {
        return;
    }
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f %>%b; builtin verity %b --enable",
    );
    assert_eq!(out.status.code(), Some(17), "stderr={}", stderr(&out));
}

/// The namesake: enable verity on a binary, pin its digest, verify it, and only
/// then `exec_fd` it (exit 42). A wrong digest skips the exec (rc 1).
#[test]
fn verify_then_exec() {
    let dir = Scratch::new();
    std::fs::copy(EXEC_OK, dir.join("bin")).unwrap();
    // Stage 1a: enable *alone*. A script that has already enabled the file must
    // never be retried as a unit: a transient at a later step would make the
    // retry fail at `--enable` with EEXIST (17) and name the wrong errno.
    if !enable(&dir, "bin") {
        return;
    }
    // Stage 1b: query *alone* (a query never changes the file, so `run`'s
    // transient retry is safe here), capturing the digest.
    let out1 = run(
        &dir,
        "builtin openat2 --flags O_RDONLY bin %>%b; builtin verity %b",
    );
    assert_eq!(out1.status.code(), Some(0), "stderr={}", stderr(&out1));
    let s = stdout(&out1);
    assert!(
        s.starts_with("enabled=yes algo=sha256 digest="),
        "stage 1 stdout={s:?}"
    );
    let digest = s.trim_end().split("digest=").nth(1).unwrap().to_string();
    // Stage 2: verify against the captured digest, then exec the binary.
    let ok = run(
        &dir,
        &format!(
            "builtin openat2 --flags O_RDONLY bin %>%b; \
             builtin verity %b --digest {digest} && builtin exec_fd %b"
        ),
    );
    assert_eq!(ok.status.code(), Some(42), "stderr={}", stderr(&ok));
    // Negative: a wrong digest fails the check, so the exec never runs.
    let bad = run(
        &dir,
        &format!(
            "builtin openat2 --flags O_RDONLY bin %>%b; \
             builtin verity %b --digest {SHA256_DIGEST} && builtin exec_fd %b"
        ),
    );
    assert_eq!(bad.status.code(), Some(1), "stderr={}", stderr(&bad));
}

/// Error paths: unset var, bad flag, non-hex digest, unknown algo, `--help`.
#[test]
fn error_paths() {
    let dir = Scratch::new();
    std::fs::write(dir.join("plain"), CONTENT).unwrap();
    let missing = run(&dir, "builtin verity %nope");
    assert_eq!(
        missing.status.code(),
        Some(1),
        "stderr={}",
        stderr(&missing)
    );
    assert!(
        stderr(&missing).contains("fd variable"),
        "stderr={}",
        stderr(&missing)
    );
    let badflag = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; builtin verity %b --bogus",
    );
    assert_eq!(
        badflag.status.code(),
        Some(1),
        "stderr={}",
        stderr(&badflag)
    );
    let badhex = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; builtin verity %b --digest xyz",
    );
    assert_eq!(badhex.status.code(), Some(1), "stderr={}", stderr(&badhex));
    let badalgo = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; builtin verity %b --enable --algo md5",
    );
    assert_eq!(
        badalgo.status.code(),
        Some(1),
        "stderr={}",
        stderr(&badalgo)
    );
    let help = run(&dir, "builtin verity --help");
    assert_eq!(help.status.code(), Some(0), "stderr={}", stderr(&help));
}

/// `--dump descriptor`: the 256-byte descriptor as 16 hex rows; the first row
/// is pinned to the deterministic >= 6.13 descriptor header (version 1,
/// sha256, log2(4096), no salt, 19-byte content).
#[test]
fn dump_descriptor() {
    let dir = Scratch::new();
    std::fs::write(dir.join("f"), CONTENT).unwrap();
    if !enable(&dir, "f") {
        return;
    }
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f %>%b; builtin verity %b --dump descriptor",
    );
    assert_eq!(out.status.code(), Some(0), "stderr={}", stderr(&out));
    let s = stdout(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 16, "stdout={s:?}");
    assert_eq!(
        lines.first().copied(),
        Some("00000000  01 01 0c 00 00 00 00 00  13 00 00 00 00 00 00 00")
    );
}

/// `--dump tree` on a single-block file: the tree item is empty (the root
/// hash lives in the descriptor), so no rows are printed.
#[test]
fn dump_tree_single_block() {
    let dir = Scratch::new();
    std::fs::write(dir.join("f"), CONTENT).unwrap();
    if !enable(&dir, "f") {
        return;
    }
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f %>%b; builtin verity %b --dump tree",
    );
    assert_eq!(out.status.code(), Some(0), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "");
}

/// `--dump descriptor --offset 0 --length 16`: exactly one hex row.
#[test]
fn dump_offset_length() {
    let dir = Scratch::new();
    std::fs::write(dir.join("f"), CONTENT).unwrap();
    if !enable(&dir, "f") {
        return;
    }
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY f %>%b; \
         builtin verity %b --dump descriptor --offset 0 --length 16",
    );
    assert_eq!(out.status.code(), Some(0), "stderr={}", stderr(&out));
    let s = stdout(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 1, "stdout={s:?}");
    assert_eq!(
        lines.first().copied(),
        Some("00000000  01 01 0c 00 00 00 00 00  13 00 00 00 00 00 00 00")
    );
}

/// `--dump` on a non-verity file is a genuine error: the exit code is
/// `ENODATA` (61).
#[test]
fn dump_non_verity() {
    let dir = Scratch::new();
    std::fs::write(dir.join("plain"), CONTENT).unwrap();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; builtin verity %b --dump descriptor",
    );
    assert_eq!(out.status.code(), Some(61), "stderr={}", stderr(&out));
}

/// `--dump` error paths: a bad type, `--offset` without `--dump`, and
/// `--dump` combined with `--enable` all exit 1.
#[test]
fn dump_error_paths() {
    let dir = Scratch::new();
    std::fs::write(dir.join("plain"), CONTENT).unwrap();
    let badtype = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; builtin verity %b --dump bogus",
    );
    assert_eq!(
        badtype.status.code(),
        Some(1),
        "stderr={}",
        stderr(&badtype)
    );
    let noflag = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; builtin verity %b --offset 8",
    );
    assert_eq!(noflag.status.code(), Some(1), "stderr={}", stderr(&noflag));
    let both = run(
        &dir,
        "builtin openat2 --flags O_RDONLY plain %>%b; \
         builtin verity %b --dump descriptor --enable",
    );
    assert_eq!(both.status.code(), Some(1), "stderr={}", stderr(&both));
}
