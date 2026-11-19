//! E2E bash-parity tests for the `read` builtin's flag families: default
//! backslash processing, `-r`, `-d delim` (including `-d ''`), `-t` timeout,
//! and the EOF status rule (EOF before the delimiter → 1, even with data).
//!
//! `read` rejects direct redirects, so stdin is fed through `exec < file`;
//! a fifo (O_RDWR, no peer) is the deterministic "open, empty, no EOF"
//! source for the `-t 0` timeout case.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Per-test scratch dir, removed on drop. Named with pid + an atomic counter:
/// tests in one binary share the process and run on parallel threads, so a
/// pid-only name would collide (LESSONS).
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fdshell-read-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl std::ops::Deref for Scratch {
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

fn run(dir: &std::path::Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    str::from_utf8(&out.stdout).unwrap().to_string()
}

fn stderr(out: &Output) -> String {
    str::from_utf8(&out.stderr).unwrap().to_string()
}

/// Default mode processes backslashes: `a\nb` (a, backslash, n, b) → `anb`.
#[test]
fn default_backslash_processing() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\nb\\n" > in; exec < in; read x; printf "[%s]\n" "$x""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[anb]\n", "script={script}");
}

/// `-r` keeps the backslash literal: `a\nb` stays `a\nb`.
#[test]
fn r_keeps_backslash_literal() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\nb\\n" > in; exec < in; read -r x; printf "[%s]\n" "$x""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a\\nb]\n", "script={script}");
}

/// A backslash + newline is a continuation: both bytes are dropped.
#[test]
fn continuation_line_dropped() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\\\nb\\n" > in; exec < in; read x; printf "[%s]\n" "$x""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[ab]\n", "script={script}");
}

/// A trailing backslash at EOF is dropped (bash behavior), status 1.
#[test]
fn trailing_backslash_at_eof_dropped() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\" > in; exec < in; read x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a] rc=1\n", "script={script}");
}

/// `-r` keeps a trailing backslash at EOF literal, status 1.
#[test]
fn trailing_backslash_at_eof_raw_kept() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\" > in; exec < in; read -r x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a\\] rc=1\n", "script={script}");
}

/// `-d :` stops at the colon: `a:b:c` → `a`, status 0.
#[test]
fn colon_delimiter() {
    let dir = Scratch::new();
    let script = r#"printf "a:b:c" > in; exec < in; read -d : x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a] rc=0\n", "script={script}");
}

/// `-d ':|'` stops at the first byte of the set: `ab|cd` → `ab`, status 0.
#[test]
fn multi_char_delimiter_set() {
    let dir = Scratch::new();
    let script =
        r#"printf "ab|cd" > in; exec < in; read -d ":|" x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[ab] rc=0\n", "script={script}");
}

/// `-d ''` reads the whole stream; EOF is still a failure (status 1), data kept.
#[test]
fn empty_delim_reads_to_eof() {
    let dir = Scratch::new();
    let script = r#"printf "a:b:c" > in; exec < in; read -d "" x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a:b:c] rc=1\n", "script={script}");
}

/// EOF before the newline delimiter → status 1, even with partial data.
#[test]
fn eof_without_newline_status_1() {
    let dir = Scratch::new();
    let script = r#"printf "abc" > in; exec < in; read x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[abc] rc=1\n", "script={script}");
}

/// A newline-terminated line → status 0.
#[test]
fn newline_terminated_status_0() {
    let dir = Scratch::new();
    let script = r#"printf "abc\\n" > in; exec < in; read x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[abc] rc=0\n", "script={script}");
}

/// `-t 0` with data immediately available reads it, status 0.
#[test]
fn timeout_zero_data_ready() {
    let dir = Scratch::new();
    let script = r#"printf "hi\\n" > in; exec < in; read -t 0 x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[hi] rc=0\n", "script={script}");
}

/// `-t 0` on an open, empty fifo (O_RDWR: no data, no EOF) times out:
/// status 1, the variable is unset.
#[test]
fn timeout_zero_no_data_fifo() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --mode 0600 ch %>%ch; \
                  builtin openat2 --flags O_RDWR ch %>%p; \
                  read -t 0 L -u %p; \
                  echo $?:$L";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    // status 1 and the variable left unset: an unset `$L` expands to empty, so
    // the word collapses to `1:`.
    assert_eq!(stdout(&out), "1:\n", "script={script}");
}

// ---- task #600: bash's numeric-argument grammars for `-n` and `-t` ----

/// Task #600 (#608): bash's `-t` grammar accepts fractions (`0.5`, `.5`,
/// `1.0`, measured) and `read` waits their fraction. On the open, empty fifo
/// each row times out after the waited fraction at status 1 (fdshell's own
/// status rule, pinned by the `-t 0` rows above) and parses silently: the
/// acceptance, not an error report, is what is pinned. The run takes ~2 s.
#[test]
fn timeout_fractions_wait_and_time_out() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --mode 0600 ch %>%ch; \
                  builtin openat2 --flags O_RDWR ch %>%p; \
                  read -t 0.5 L -u %p; echo $?:$L; \
                  read -t .5 L -u %p; echo $?:$L; \
                  read -t 1.0 L -u %p; echo $?:$L";
    let start = std::time::Instant::now();
    let out = run(&dir, script);
    let elapsed = start.elapsed();
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "1:\n1:\n1:\n", "script={script}");
    assert_eq!(
        stderr(&out),
        "",
        "accepted shapes parse silently: script={script}"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(1700),
        "each row waited its fraction: {elapsed:?}"
    );
}

/// Task #600: past `u32::MAX` the widened grammar reaches the saturate-to-
/// infinite rule, so `-t 999999999999` blocks for a late writer — bash's
/// measured parity — and so does `4294967296`, where bash returns immediately
/// (non-monotonic overflow, not the contract; recorded in the README). Both
/// rows pin fdshell's measured behaviour: the late line is read, status 0.
#[test]
fn timeout_past_u32_blocks_for_a_late_writer() {
    for word in ["999999999999", "4294967296"] {
        let dir = Scratch::new();
        let setup = run(&dir, "builtin mkfifoat --mode 0600 fifo");
        assert_eq!(setup.status.code(), Some(0), "mkfifoat: {}", stderr(&setup));
        let writer_fifo = dir.join("fifo");
        let writer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&writer_fifo)
                .unwrap();
            f.write_all(b"late\n").unwrap();
        });
        let script = format!("exec < fifo; read -t {word} L; echo $?:$L");
        let out = run(&dir, &script);
        writer.join().unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "script={script} stderr={}",
            stderr(&out)
        );
        assert_eq!(stdout(&out), "0:late\n", "script={script}");
    }
}

// ---- task #604: the trim at the line's edges, and the tail kept verbatim ----

/// Task #604 (bash 5.3.9 measured, `cat -A`-checked): with two targets the
/// last variable gets the tail of the line with its separator text intact —
/// `a··b⇥c` answers `a` / `b⇥c`, not a rejoin with single spaces.
#[test]
fn last_variable_keeps_the_separator_text() {
    let dir = Scratch::new();
    let script =
        r#"printf 'a  b\tc\n' > in; exec < in; read x y; printf "[%s][%s] rc=%s\n" "$x" "$y" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a][b\tc] rc=0\n", "script={script}");
}

/// Task #604 (bash 5.3.9 measured): IFS whitespace is stripped at the line's
/// edges, internal separators stay — `··a··b··` into one variable answers
/// `a··b`, and the all-whitespace line `···` answers the empty value.
#[test]
fn edges_are_trimmed_internal_separators_stay() {
    for (word, want) in [("'  a  b  \n'", "[a  b] rc=0\n"), ("'   \n'", "[] rc=0\n")] {
        let dir = Scratch::new();
        let script = format!(
            "printf {word} > in; exec < in; read x; printf \"[%s] rc=%s\\n\" \"$x\" \"$?\""
        );
        let out = run(&dir, &script);
        assert_eq!(
            out.status.code(),
            Some(0),
            "script={script} stderr={}",
            stderr(&out)
        );
        assert_eq!(stdout(&out), want, "script={script}");
    }
}

/// Task #604 (bash 5.3.9 measured): the trim applies to the text `-n` capped,
/// so `··a··` with `-n 5` answers `a`.
#[test]
fn trim_applies_to_the_capped_text() {
    let dir = Scratch::new();
    let script =
        r#"printf '  a  \n' > in; exec < in; read -n 5 x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a] rc=0\n", "script={script}");
}

/// Task #604 (bash 5.3.9 measured): newline is in the default-IFS trim set, so
/// an EOF read (`-d ''`) of `\na·b·` answers `a·b` at rc 1 — the newline is
/// trimmed at the edges, and the EOF status rule (pinned above) still holds.
#[test]
fn newline_edges_trimmed_on_an_eof_read() {
    let dir = Scratch::new();
    let script =
        r#"printf '\na b \n' > in; exec < in; read -d "" x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a b] rc=1\n", "script={script}");
}

/// Task #600 (the verified-unknown sibling): `read -n` parses its count
/// through bash's `legal_number` grammar, so `" 3"`, `+3` and `03` each take
/// 3 characters of a 6-character line at rc 0 (bash 5.3.9 measured).
#[test]
fn n_accepts_legal_number_shapes() {
    for word in ["\" 3\"", "+3", "03"] {
        let dir = Scratch::new();
        let script = format!(
            "printf 'abcdef\\n' > in; exec < in; read -n {word} v; printf '[%s] rc=%s\\n' \"$v\" \"$?\""
        );
        let out = run(&dir, &script);
        assert_eq!(
            out.status.code(),
            Some(0),
            "script={script} stderr={}",
            stderr(&out)
        );
        assert_eq!(stdout(&out), "[abc] rc=0\n", "script={script}");
    }
}
