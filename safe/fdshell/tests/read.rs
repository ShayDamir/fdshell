//! E2E bash-parity tests for the `read` builtin's flag families: default
//! backslash processing, `-r`, `-d delim` (including `-d ''`), `-t` timeout,
//! the EOF status rule (EOF before the delimiter → 1, even with data), and the
//! `-p` prompt's terminal gate on the fd `read` actually reads from.
//!
//! `read` rejects direct redirects, so stdin is fed through `exec < file`;
//! a fifo (O_RDWR, no peer) is the deterministic "open, empty, no EOF"
//! source for the `-t 0` timeout case.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
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

// ---- task #607: `-p` is gated on the fd `read` reads from being a terminal ----

/// Runs the script with `input` piped into the child's stdin (a pipe is not a
/// terminal) so the streams stay separable: the pin is on captured stderr.
fn run_with_piped_stdin(dir: &std::path::Path, script: &str, input: &str) -> Output {
    use std::io::Write as _;
    let mut child = Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
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
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

/// Runs the script under util-linux `script`, which attaches a pty and forwards
/// the test's piped stdin into it, making the shell's stdin a terminal (LESSONS:
/// "Test tty-gated behavior from safe-crate integration tests via `script`");
/// returns the pty stream, which carries both streams plus the fed input's echo
/// with `\r\n` — so rows assert `contains` / absence, never exact equality.
/// Empty `input` feeds nothing (the row's `read` is fed by its own fd). Every
/// row ends with a read that gets its data, so the shell exits on its own and
/// the wait is bounded. `script` runs the command through a shell, so the inner
/// script is single-quoted and must quote with double quotes itself.
fn run_on_a_tty(dir: &std::path::Path, script: &str, input: &str) -> String {
    use std::io::Write as _;
    let mut child = Command::new("script")
        .current_dir(dir)
        .args(["-qec", &format!("{BIN} -c '{script}'"), "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if !input.is_empty() {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "script={script}");
    str::from_utf8(&out.stdout).unwrap().to_string()
}

/// Task #607 (bash 5.3.9 measured): stdin is a pipe, so the prompt is not drawn.
#[test]
fn prompt_stays_silent_when_stdin_is_a_pipe() {
    let dir = Scratch::new();
    let script = r#"read -p "P: " x; echo got=$x"#;
    let out = run_with_piped_stdin(&dir, script, "answer\n");
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "got=answer\n", "script={script}");
    assert_eq!(
        stderr(&out),
        "",
        "a non-tty fd read from draws no prompt: script={script}"
    );
}

/// Task #607 (bash 5.3.9 measured): stdin is a regular file — silence, as for a
/// pipe.
#[test]
fn prompt_stays_silent_when_stdin_is_a_regular_file() {
    let dir = Scratch::new();
    let script = r#"printf "answer\n" > in; exec < in; read -p "P: " x; echo got=$x"#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "got=answer\n", "script={script}");
    assert_eq!(
        stderr(&out),
        "",
        "a non-tty fd read from draws no prompt: script={script}"
    );
}

/// Task #607 (bash 5.3.9 measured): stdin is a terminal, so the prompt is drawn.
#[test]
fn prompt_is_drawn_when_stdin_is_a_tty() {
    let dir = Scratch::new();
    let stream = run_on_a_tty(&dir, r#"read -p "P: " x; echo got=$x"#, "answer\n");
    assert!(stream.contains("P: "), "a tty fd read from draws: {stream}");
    assert!(
        stream.contains("got=answer"),
        "the fed line is read: {stream}"
    );
}

/// Task #607 (bash 5.3.9 measured): stdin is the terminal but `-u 3` reads a
/// regular file — the fd tested is the fd read from, so nothing is drawn even
/// though stdin would pass the test. Every fd-3 row establishes fd 3 itself, so
/// no harness-inherited fd (a jobserver pipe) can leak into the mapping.
#[test]
fn prompt_stays_silent_on_a_tty_when_the_u_fd_is_a_regular_file() {
    let dir = Scratch::new();
    let script = r#"printf "answer\n" > in; exec 3< in; read -u 3 -p "P: " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "");
    assert!(
        !stream.contains("P: "),
        "the fd read from is not a tty: {stream}"
    );
    assert!(
        stream.contains("got=answer"),
        "the fd's line is read: {stream}"
    );
}

/// Task #607 (bash 5.3.9 measured): the task's process-substitution row, whose
/// executable stand-in here is a here-string — fdshell has no `<(...)`, and
/// bash measures the same silence for `exec 3<<<answer` (neither fd is a
/// terminal), so the rule pins identically.
#[test]
fn prompt_stays_silent_on_a_tty_when_the_u_fd_is_a_here_string() {
    let dir = Scratch::new();
    let script = r#"exec 3<<<answer; read -u 3 -p "P: " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "");
    assert!(
        !stream.contains("P: "),
        "the fd read from is not a tty: {stream}"
    );
    assert!(
        stream.contains("got=answer"),
        "the fd's line is read: {stream}"
    );
}

/// Task #607 (bash 5.3.9 measured): stdin is the terminal and `-u 3` is opened
/// on `/dev/tty` — under the `script` wrapper both names the same pty, so the fd
/// passes the test and the prompt is drawn.
#[test]
fn prompt_is_drawn_on_a_tty_when_the_u_fd_is_the_tty() {
    let dir = Scratch::new();
    let script = r#"exec 3</dev/tty; read -u 3 -p "P: " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "answer\n");
    assert!(stream.contains("P: "), "a tty fd read from draws: {stream}");
    assert!(
        stream.contains("got=answer"),
        "the fed line is read: {stream}"
    );
}

// ---- task #623: the `-p` operand is an ordinary word, expanded by the pipeline ----

// bash decodes prompt escapes only for its four prompt drawers, so the `-p`
// operand goes through the normal word pipeline — parameter, command
// substitution (both spellings), arithmetic, the escape-pair fold, `IFS`
// splitting, glob — and the operand slot takes the first field. Every row below
// was measured against bash 5.3.9 on this machine on 2026-11-20 before pinning;
// the pty rows feed their line through the harness's stdin.

/// Task #623 (bash 5.3.9 measured 2026-11-20: `P[V] ` then `got=data`): a
/// parameter expansion in the operand fires before the prompt is drawn.
#[test]
fn the_operand_word_expands_on_a_tty() {
    let dir = Scratch::new();
    let script = r#"v=V; read -p "P[$v] " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(
        stream.contains("P[V] "),
        "bash draws the expansion: {stream}"
    );
    assert!(
        stream.contains("got=data"),
        "the fed line is read: {stream}"
    );
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: `P[hi] ` and `P[bt] `): the two
/// command-substitution spellings, and the arithmetic arm, all fire on this word.
#[test]
fn the_operand_word_fires_the_substitution_arms() {
    let dir = Scratch::new();
    let script = r#"read -p "P[$(echo hi)] " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(stream.contains("P[hi] "), "bash draws the same: {stream}");
    assert!(
        stream.contains("got=data"),
        "the fed line is read: {stream}"
    );

    let script = r#"read -p "P[`echo bt`] " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(stream.contains("P[bt] "), "bash draws the same: {stream}");

    let script = r#"read -p "P[$((1+1))] " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(stream.contains("P[2] "), "bash draws the same: {stream}");
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: `*.txt ` with a match in the
/// directory): a quoted operand is not globbed — quoting protects this word from
/// pathname expansion, exactly as it does everywhere else.
#[test]
fn a_quoted_operand_keeps_its_glob_char() {
    let dir = Scratch::new();
    let script = r#"touch one.txt; read -p "*.txt" x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    // The prompt has no newline of its own, so the drawn prompt runs straight into
    // the row's output; the pty's echo of the typed line cannot produce that
    // concatenation, which is what makes the needle the drawn prompt.
    assert!(
        stream.contains("*.txtgot=data"),
        "bash draws the pattern itself: {stream}"
    );
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: the matched name `one.txt ` when it
/// is the only match): unquoted, the operand globs.
#[test]
fn an_unquoted_operand_is_globbed_to_its_match() {
    let dir = Scratch::new();
    let script = r#"touch one.txt; read -p *.txt x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(
        stream.contains("one.txtgot=data"),
        "bash draws the match: {stream}"
    );
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: prompt `A`, and `B` is the variable
/// name, so its `got=` is empty): the operand slot takes the **first** field of
/// the split — the pin. Which fields become variable names is the word-expansion
/// policy task's question, so fdshell's target list still comes from the raw
/// words and its `got=` carries the line; the README records the pair.
#[test]
fn the_operand_slot_takes_the_first_field() {
    let dir = Scratch::new();
    let script = r#"v="A B"; read -p $v x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(
        stream.contains("Agot="),
        "the prompt is the first field, with no newline of its own: {stream}"
    );
    assert!(stream.contains("got=data"), "fdshell's target: {stream}");
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: `P\a$ `, the word text as the
/// parser left it): the operand is not prompt-decoded — its escape pairs fold as
/// any word's do, and no prompt escapes are read in it.
#[test]
fn the_operand_folds_its_escape_pairs_and_decodes_nothing() {
    let dir = Scratch::new();
    let script = r#"read -p "P\\a\$ " x; echo got=$x"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(
        stream.contains("P\\a$ got=data"),
        "bash draws the folded word text: {stream}"
    );
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: the file is made and nothing is
/// drawn): the operand's expansion runs at parse time, whether or not the prompt
/// is — a `read -p` off a terminal still pays for the expansion (and the fork).
#[test]
fn the_operand_expands_even_when_nothing_is_drawn() {
    let dir = Scratch::new();
    let script = r#"printf "data\n" > in; exec < in; read -p "P[$(touch made)] " x; ls made"#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(
        stdout(&out),
        "made\n",
        "bash makes the file too: script={script}"
    );
    assert_eq!(
        stderr(&out),
        "",
        "a non-tty source draws no prompt: script={script}"
    );
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: `P[V]du=x`): the last expanded
/// field after the read is the variable word, not the operand's expansion —
/// expanding this one word does not hand the pipeline's field list to the frame
/// (LESSONS: "`$_` is the last expanded field: expanding intercepts hand their
/// list to one frame (task #376)" is that task's owner).
#[test]
fn the_last_expanded_field_after_the_read_stays_the_raw_word() {
    let dir = Scratch::new();
    let script = r#"v=V; read -p "P[$v]" x; echo du=$_"#;
    let stream = run_on_a_tty(&dir, script, "data\n");
    assert!(
        stream.contains("P[V]du=x"),
        "bash measures the same: {stream}"
    );
}

/// Task #623 (bash 5.3.9 measured 2026-11-20: `nope: unbound variable`, and the
/// builtin does not run): a failing operand expansion is reported and the
/// builtin runs nothing. The report names the variable bash names; its wording is
/// fdshell's own (the README's failing-expansion paragraph), and the exit code is
/// the shell-wide 1 rather than bash's 127 — the documented family.
#[test]
fn a_nounset_operand_is_reported_and_runs_nothing() {
    let dir = Scratch::new();
    let script = r#"set -u; read -p "P[$nope]" x; echo ran rc=$?"#;
    let out = run(&dir, script);
    assert_eq!(out.status.code(), Some(1), "script={script}");
    assert_eq!(stdout(&out), "", "the builtin never runs: script={script}");
    assert!(
        stderr(&out).contains("nope: unbound variable"),
        "the report names the variable: {}",
        stderr(&out)
    );
}

// ---- task #609: the tail of a chunk feeds the next read on the same source ----

// bash keeps the bytes a `read(2)` chunk carried past the end of the line and
// answers them to the next read on that source. Every row below was measured
// against bash 5.3.9 on this machine, 6/6, before being pinned.

/// Three reads from one file: the first takes the chunk and the other two answer
/// what it left behind (bash `x=a y=b z=c`).
#[test]
fn three_reads_from_one_file_get_the_three_lines() {
    let dir = Scratch::new();
    let script = r#"printf "a\nb\nc\n" > in; exec < in; read x; read y; read z; printf "x=%s y=%s z=%s\n" "$x" "$y" "$z""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "x=a y=b z=c\n", "script={script}");
}

/// The `-n` cap is a stop reason: the line takes two characters and the next read
/// gets the rest of the line (bash `x=ab y=cdef`).
#[test]
fn a_capped_read_leaves_the_rest_of_the_line_for_the_next_one() {
    let dir = Scratch::new();
    let script = r#"read -n 2 x; read y; printf "x=%s y=%s\n" "$x" "$y""#;
    let out = run_with_piped_stdin(&dir, script, "abcdef\n");
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "x=ab y=cdef\n", "script={script}");
}

/// A `-d` delimiter is the third stop reason (bash `x=a y=b`).
#[test]
fn a_delimited_read_leaves_the_rest_of_the_line_for_the_next_one() {
    let dir = Scratch::new();
    let script = r#"read -d : x; read -d : y; printf "x=%s y=%s\n" "$x" "$y""#;
    let out = run_with_piped_stdin(&dir, script, "a:b:c\n");
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "x=a y=b\n", "script={script}");
}

/// The buffer follows the fd the script named, so two `-u 3` reads on one file
/// answer its first two lines (bash `x=a y=b`).
#[test]
fn two_reads_on_one_u_fd_get_the_two_lines() {
    let dir = Scratch::new();
    let script = r#"printf "a\nb\n" > in; exec 3< in; read -u 3 x; read -u 3 y; printf "x=%s y=%s\n" "$x" "$y""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "x=a y=b\n", "script={script}");
}

/// The multi-line `while read` the bug was about: bash iterates three times, and a
/// reader that drops its chunk iterates once.
#[test]
fn while_read_iterates_every_line_of_the_source() {
    let dir = Scratch::new();
    let script =
        r#"printf "a\nb\nc\n" > in; exec < in; while read l; do printf "got=%s\n" "$l"; done"#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "got=a\ngot=b\ngot=c\n", "script={script}");
}

/// The pin that protects the buffer's lifetime rule: a read from piped stdin
/// buffers a tail, `exec <` then reassigns stdin to another file, and the next
/// read must answer the **second** file — the buffer belongs to the descriptor it
/// was taken from, not to the name (bash 5.3.9 measured `a=FIRST-1 b=SECOND-1`).
#[test]
fn a_reassigned_stdin_loses_the_stale_buffer() {
    let dir = Scratch::new();
    let script = r#"printf "SECOND-1\nSECOND-2\n" > second; read a; exec < second; read b; printf "a=%s b=%s\n" "$a" "$b""#;
    let out = run_with_piped_stdin(&dir, script, "FIRST-1\nFIRST-2\n");
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "a=FIRST-1 b=SECOND-1\n", "script={script}");
}

// ---- task #610: a nameless `read` fills REPLY with the raw line ----

// bash's extension over POSIX's ≥1-name rule. Every expectation below was
// measured against bash 5.3.9 on this machine before being pinned: the raw line
// (no edge trim, no field splitting), the EOF assignment of an empty REPLY at
// status 1, and the explicit `read REPLY` name staying an ordinary, trimmed
// target.

/// [`run`] with the `build.rs` helper dir first on `PATH` (the
/// `tests/getopts.rs::run` shape) so the script's `env_print` call resolves,
/// and with `REPLY` removed from the harness environment so an absent row is
/// the script's own doing.
fn run_with_helper_path(dir: &std::path::Path, script: &str) -> Output {
    let helper_dir = env!("ENV_PRINT_PATH")
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("/usr/bin");
    let ambient = std::env::var("PATH").unwrap_or_default();
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .env("PATH", format!("{helper_dir}:{ambient}"))
        .env_remove("REPLY")
        .output()
        .unwrap()
}

/// No names: the raw line arrives whole, edges included (bash
/// `rc=0 REPLY=[  a  b  ]`).
#[test]
fn no_names_assign_reply_the_raw_line() {
    let dir = Scratch::new();
    let script =
        r#"printf '  a  b  \n' > in; exec < in; read; printf 'rc=%s REPLY=[%s]\n' "$?" "$REPLY""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "rc=0 REPLY=[  a  b  ]\n", "script={script}");
}

/// EOF with nothing read: bash **assigns** REPLY the empty value at status 1 —
/// a named target stays fdshell's pinned divergence (unset), REPLY is assigned
/// (bash `rc=1 REPLY=[]`).
#[test]
fn no_names_at_eof_clears_reply() {
    let dir = Scratch::new();
    let script = r#"REPLY=old; : > e; exec < e; read; printf 'rc=%s REPLY=[%s]\n' "$?" "$REPLY""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "rc=1 REPLY=[]\n", "script={script}");
}

/// The same EOF row under `set -u`: bash prints `[]` without an unbound report,
/// because REPLY was assigned — the row pins the assignment, not the value
/// (fdshell's `${NAME+word}` is not expanded, LESSONS, so the set-ness marker
/// cannot be used at this level).
#[test]
fn set_u_reports_nothing_for_a_reply_cleared_at_eof() {
    let dir = Scratch::new();
    let script = r#"set -u; : > e; exec < e; read; printf '[%s]\n' "$REPLY""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[]\n", "script={script}");
}

/// Flags-only forms take the same REPLY path (bash rows: `rc=0 REPLY=[a\nb]`,
/// `rc=0 REPLY=[ab]`, `rc=0 REPLY=[ab]`), each after backslash processing,
/// delimiter removal and the `-n`/`-d` stop — with no trim.
#[test]
fn a_flags_only_read_fills_reply() {
    for (printf_body, read_part, want) in [
        (r#""a\\\\nb\\n""#, "read -r", "rc=0 REPLY=[a\\nb]\n"),
        (r#""abcdef\\n""#, "read -n 2", "rc=0 REPLY=[ab]\n"),
        (r#""ab:cd\\n""#, "read -d :", "rc=0 REPLY=[ab]\n"),
    ] {
        let dir = Scratch::new();
        let script = format!(
            "printf {printf_body} > in; exec < in; {read_part}; printf 'rc=%s REPLY=[%s]\\n' \"$?\" \"$REPLY\""
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

/// An explicit `read REPLY` is an ordinary target — trimmed like any other
/// (bash `rc=0 REPLY=[a  b]`), which is why "no names" must stay a distinct
/// signal rather than a synthetic REPLY target.
#[test]
fn an_explicit_reply_name_is_trimmed_like_any_target() {
    let dir = Scratch::new();
    let script = r#"printf '  a  b  \n' > in; exec < in; read REPLY; printf 'rc=%s REPLY=[%s]\n' "$?" "$REPLY""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "rc=0 REPLY=[a  b]\n", "script={script}");
}

/// The multi-line `while read` through the REPLY path (also the putback buffer
/// of task #609): three lines, all raw (bash rows `[line one]`…`[line three]`).
#[test]
fn while_read_fills_reply_for_every_line() {
    let dir = Scratch::new();
    let script = r#"printf 'line one\nline two\nline three\n' > in; exec < in; while read; do printf '[%s]\n' "$REPLY"; done"#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(
        stdout(&out),
        "[line one]\n[line two]\n[line three]\n",
        "script={script}"
    );
}

/// A newline-only line assigns REPLY the empty string at rc 0 in both shells.
#[test]
fn a_newline_only_line_assigns_an_empty_reply() {
    let dir = Scratch::new();
    let script = r#"printf '\n' > in; exec < in; read; printf 'rc=%s REPLY=[%s]\n' "$?" "$REPLY""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "rc=0 REPLY=[]\n", "script={script}");
}

/// The env row mirroring `tests/getopts.rs`: REPLY is an ordinary variable —
/// absent from the child environment by default, present after an ordinary
/// `export` (bash agrees on both).
#[test]
fn reply_stays_out_of_the_child_environment() {
    let dir = Scratch::new();
    let script = r#"printf "x\\n" > in; exec < in; read; env_print"#;
    let out = run_with_helper_path(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).lines().any(|l| l.starts_with("REPLY=")),
        "bash measures no REPLY row here: script={script}"
    );

    let script = r#"printf "x\\n" > in; exec < in; read; export REPLY; env_print"#;
    let out = run_with_helper_path(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stdout(&out).lines().any(|l| l == "REPLY=x"),
        "the ordinary export path exports, in bash too: script={script}"
    );
}

// ---- task #630: one shared stdin buffer for the parse layer and `read` ----

// bash answers the parse layer and `read` from one buffered reader for
// stdin; fdshell joins the two sides through the task #609 buffer. Every row
// here was measured against bash 5.3.9 on this machine before being pinned.

/// Runs the script as a **stdin-fed session** — no `-c`, the script arrives
/// on the piped stdin, so the parse layer's buffered reader is in play (the
/// `tests/repl.rs::run_repl` shape). Off a terminal `prompt::resolve` draws
/// nothing, so for the clean rows here stdout is exact and stderr empty.
fn run_stdin_session(dir: &std::path::Path, script: &str) -> Output {
    use std::io::Write as _;
    let mut child = Command::new(BIN)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

/// Task #630 (bash 5.3.9 measured): the builtin's line sits in the parse
/// layer's buffered chunk — with one shared buffer `read` answers `second`
/// and nothing runs as a command after it (bash `top`, `[second]`, rc 0).
#[test]
fn a_stdin_fed_session_shares_the_stdin_buffer() {
    let dir = Scratch::new();
    let out = run_stdin_session(&dir, "echo top; read x; echo [$x]\nsecond\n");
    assert_eq!(
        out.status.code(),
        Some(0),
        "the session ends clean: stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "top\n[second]\n");
    assert_eq!(stderr(&out), "");
}

/// Task #630 (bash 5.3.9 measured): the forward direction — the builtin takes
/// the parse layer's whole buffered second line, and the parser then reads
/// EOF and exits without executing anything else.
#[test]
fn a_read_consumes_the_parsers_buffered_line() {
    let dir = Scratch::new();
    let out = run_stdin_session(&dir, "read x; echo [$x]\necho kept\n");
    assert_eq!(
        out.status.code(),
        Some(0),
        "the session ends clean: stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[echo kept]\n");
    assert_eq!(stderr(&out), "");
}

/// Task #630 (bash 5.3.9 measured): the reverse direction — `read -n 4` takes
/// `echo` from the shared buffer and puts back ` kept\n`, so the parser's
/// next line is `kept`: command-not-found on both shells. Both stderrs carry
/// reports (so only stdout is pinned), and the shells' exit codes disagree
/// here (bash 127, fdshell 0 — the REPL's EOF exit is pinned elsewhere), so
/// the status is left out.
#[test]
fn a_read_putback_is_visible_to_the_parsers_next_line() {
    let dir = Scratch::new();
    let out = run_stdin_session(&dir, "read -n 4 x; echo [$x]\necho kept\n");
    assert_eq!(
        stdout(&out),
        "[echo]\n",
        "stderr carries the report: {}",
        stderr(&out)
    );
}
