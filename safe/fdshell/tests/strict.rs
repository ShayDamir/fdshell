#![allow(clippy::unwrap_used)]

use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
/// Real helper programs (compiled by `build.rs`): `exec_ok` exits 42,
/// `arg_print` prints its arguments. Copied into a scratch dir they become
/// relative-path programs — no shebang, no `PATH`-dependent shell.
const EXEC_OK: &str = env!("EXEC_OK_PATH");
const ARG_PRINT: &str = env!("ARG_PRINT_PATH");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir holding a readable `f`; each test gets its own dir (pid +
/// counter, since tests in one binary share the pid), removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fdshell-strict-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f"), b"hello\n").unwrap();
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

/// Copy a helper program into `dir` under `name` (an ordinary ELF binary:
/// strict's PATH search and fd exec are exercised on a real file).
fn program(dir: &std::path::Path, name: &str, helper: &str) {
    let dst = dir.join(name);
    std::fs::copy(helper, &dst).unwrap();
    std::fs::set_permissions(&dst, PermissionsExt::from_mode(0o755)).unwrap();
}

fn run(dir: &std::path::Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .output()
        .unwrap()
}

/// Run with `PATH` replaced by `path` (relative entries, and an absolute entry
/// for the ban pins) and the process CWD at `dir`.
fn run_path(dir: &std::path::Path, path: &str, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .env("PATH", path)
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

/// The shell's exit status (the last command's).
fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// Strict mode rejects an absolute path, naming the strict/absolute violation.
#[test]
fn strict_bans_absolute_path() {
    let dir = Scratch::new();
    let f = dir.join("f");
    let abs = f.to_str().unwrap();
    // A `--dirfd` is present, so the absolute-path ban (not the dirfd ban)
    // is what fires.
    let out = run(
        &dir,
        &format!("shopt -s strict; builtin openat2 --dirfd %CWD --flags O_RDONLY {abs} %>%f"),
    );
    assert!(
        !out.status.success(),
        "stdout={:?} stderr={:?}",
        stdout(&out),
        stderr(&out)
    );
    let err = stderr(&out);
    assert!(err.contains("strict"), "stderr={err:?}");
    assert!(err.contains("absolute"), "stderr={err:?}");
}

/// Strict mode accepts an explicit `--dirfd` + relative path.
#[test]
fn strict_allows_explicit_dirfd_relative() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "shopt -s strict; builtin openat2 --dirfd %CWD --flags O_RDONLY f %>%fd",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
}

/// Strict mode bans an omitted `--dirfd` (the `AT_FDCWD` / CWD form).
#[test]
fn strict_bans_missing_dirfd() {
    let dir = Scratch::new();
    let out = run(&dir, "shopt -s strict; builtin mkdirat sub %>%d");
    assert!(
        !out.status.success(),
        "stdout={:?} stderr={:?}",
        stdout(&out),
        stderr(&out)
    );
    let err = stderr(&out);
    assert!(err.contains("strict"), "stderr={err:?}");
    assert!(err.contains("dirfd"), "stderr={err:?}");
}

/// Strict mode accepts `renameat2` with both dirfds pinned and relative paths.
#[test]
fn strict_allows_renameat2_with_dirfds() {
    let dir = Scratch::new();
    std::fs::write(dir.join("a"), b"x").unwrap();
    let out = run(
        &dir,
        "shopt -s strict; builtin renameat2 --olddirfd %CWD --newdirfd %CWD a b",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert!(dir.join("b").exists(), "b should exist after rename");
    assert!(!dir.join("a").exists(), "a should be gone after rename");
}

/// Strict is off by default: an absolute path with no dirfd still works
/// (no regression for existing CWD-based scripts).
#[test]
fn default_allows_absolute_path() {
    let dir = Scratch::new();
    let f = dir.join("f");
    let abs = f.to_str().unwrap();
    let out = run(
        &dir,
        &format!("builtin openat2 --flags O_RDONLY {abs} %>%fd"),
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
}

/// The option list reports `strict off`, then `strict on` after enabling.
#[test]
fn shopt_lists_strict_off_then_on() {
    let dir = Scratch::new();
    let out = run(&dir, "shopt");
    assert!(
        stdout(&out).contains("strict off"),
        "stdout={:?}",
        stdout(&out)
    );
    let out = run(&dir, "shopt -s strict; shopt");
    assert!(
        stdout(&out).contains("strict on"),
        "stdout={:?}",
        stdout(&out)
    );
}

/// `set -o strict` / `set +o strict` toggle the option (query exit codes).
#[test]
fn set_dash_o_toggles_strict() {
    let dir = Scratch::new();
    let out = run(&dir, "set -o strict; shopt -q strict");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let out = run(&dir, "set +o strict; shopt -q strict");
    assert!(!out.status.success(), "stderr={}", stderr(&out));
}

/// Strict is bash-compatible for a relative redirect: the target is opened
/// through the shell's pinned `%CWD` fd and the command runs against it.
#[test]
fn strict_relative_redirect_creates_file() {
    let dir = Scratch::new();
    let out = run_path(&dir, ".", "shopt -s strict; builtin echo hi > out");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(std::fs::read_to_string(dir.join("out")).unwrap(), "hi\n");
}

/// Strict bans an absolute redirect target, naming the violation, and creates
/// nothing. The ban fires in the launcher, before the command is resolved.
#[test]
fn strict_bans_absolute_redirect_target() {
    let dir = Scratch::new();
    let abs = dir.join("out").to_str().unwrap().to_string();
    let out = run_path(
        &dir,
        ".",
        &format!("shopt -s strict; builtin echo hi > {abs}"),
    );
    assert!(!out.status.success(), "stdout={:?}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("strict"), "stderr={err:?}");
    assert!(err.contains("absolute"), "stderr={err:?}");
    assert!(!dir.join("out").exists(), "the target must not be created");
}

/// Strict bans an absolute target reached through a glob pattern: an absolute
/// pattern is banned before the walk, so it never reaches the filesystem root.
#[test]
fn strict_bans_absolute_globbed_target() {
    let dir = Scratch::new();
    let out = run_path(
        &dir,
        ".",
        "shopt -s strict; builtin echo hi > /tmp/nosuchdir-t136-*",
    );
    assert!(!out.status.success(), "stdout={:?}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("strict"), "stderr={err:?}");
    assert!(err.contains("absolute"), "stderr={err:?}");
}

/// A relative globbed target is globbed under strict (rooted at the `%CWD`
/// base fd) and opened as its single match.
#[test]
fn strict_globbed_relative_target_opens_the_match() {
    let dir = Scratch::new();
    std::fs::write(dir.join("a1.t"), b"x").unwrap();
    let out = run_path(&dir, ".", "shopt -s strict; builtin echo g > a*.t");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(std::fs::read_to_string(dir.join("a1.t")).unwrap(), "g\n");
}

/// More than one match is bash's ambiguous redirect (unchanged by strict).
#[test]
fn strict_globbed_target_ambiguous_on_multi_match() {
    let dir = Scratch::new();
    for name in ["a1.x", "a2.x"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    let out = run_path(&dir, ".", "shopt -s strict; builtin echo g > a*.x");
    assert!(!out.status.success(), "stdout={:?}", stdout(&out));
    assert!(
        stderr(&out).contains("ambiguous"),
        "stderr={}",
        stderr(&out)
    );
}

/// `<` (read) opens through the same strict base, and an absolute read target
/// is banned.
#[test]
fn strict_relative_read_and_absolute_ban() {
    let dir = Scratch::new();
    let out = run_path(&dir, ".", "shopt -s strict; read line < f; test -n $line");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "");
    let abs = dir.join("f").to_str().unwrap().to_string();
    let out = run_path(&dir, ".", &format!("shopt -s strict; read line < {abs}"));
    assert!(!out.status.success(), "stdout={:?}", stdout(&out));
    assert!(stderr(&out).contains("strict"), "stderr={}", stderr(&out));
}

/// Strict `noclobber` is enforced through the pinned base: an existing target
/// fails with the noclobber message, a fresh one is created, and `>|` clobbers.
#[test]
fn strict_noclobber_through_the_base_fd() {
    let dir = Scratch::new();
    let out = run_path(
        &dir,
        ".",
        "shopt -s strict; set -o noclobber; builtin echo q > f",
    );
    assert!(!out.status.success(), "stdout={:?}", stdout(&out));
    assert!(
        stderr(&out).contains("noclobber"),
        "stderr={}",
        stderr(&out)
    );
    assert_eq!(std::fs::read_to_string(dir.join("f")).unwrap(), "hello\n");
    let out = run_path(
        &dir,
        ".",
        "shopt -s strict; set -o noclobber; builtin echo q > fresh",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(std::fs::read_to_string(dir.join("fresh")).unwrap(), "q\n");
    let out = run_path(
        &dir,
        ".",
        "shopt -s strict; set -o noclobber; builtin echo q >| f",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(std::fs::read_to_string(dir.join("f")).unwrap(), "q\n");
}

/// The `>>` (append) direction opens through the base fd with the same flags
/// as before: strict adds a dirfd, not new semantics.
#[test]
fn strict_append_target() {
    let dir = Scratch::new();
    let out = run_path(&dir, ".", "shopt -s strict; builtin echo b >> f");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(
        std::fs::read_to_string(dir.join("f")).unwrap(),
        "hello\nb\n"
    );
}

/// The `<>` (read-write) direction opens through the base fd: it creates a
/// missing target, never truncates, and an absolute target is banned.
#[test]
fn strict_readwrite_target() {
    let dir = Scratch::new();
    let out = run_path(
        &dir,
        ".",
        "shopt -s strict; exec 3<>fresh; builtin echo z >&3; exec 3>&-",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(std::fs::read_to_string(dir.join("fresh")).unwrap(), "z\n");
    // No truncation: the write lands at offset 0, over the existing bytes.
    let out = run_path(
        &dir,
        ".",
        "shopt -s strict; exec 3<>f; builtin echo XX >&3; exec 3>&-",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(std::fs::read_to_string(dir.join("f")).unwrap(), "XX\nlo\n");
    let abs = dir.join("f").to_str().unwrap().to_string();
    let script = format!("shopt -s strict; exec 3<>{abs}; exec 3>&-");
    let out = run_path(&dir, ".", &script);
    assert!(!out.status.success(), "stdout={:?}", stdout(&out));
    assert!(stderr(&out).contains("strict"), "stderr={}", stderr(&out));
}

/// Strict resolves a relative `PATH` entry against the pinned `%CWD` base fd
/// and runs the program from the fd it opened (`execveat` of the pinned
/// inode). An absolute entry is banned the moment the search reaches it, so a
/// root-escaping `PATH` fails loudly.
#[test]
fn strict_runs_program_from_relative_path() {
    let dir = Scratch::new();
    std::fs::create_dir(dir.join("bin")).unwrap();
    program(&dir.join("bin"), "fdprobe", EXEC_OK);
    let out = run_path(&dir, "bin", "shopt -s strict; fdprobe");
    assert_eq!(code(&out), 42, "stderr={}", stderr(&out));
    // Nothing in the relative entries; the absolute entry must be the ban.
    let out = run_path(&dir, ".:nonexistent:/bin", "shopt -s strict; fdprobe");
    assert!(!out.status.success(), "stdout={:?}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("strict"), "stderr={err:?}");
    assert!(err.contains("absolute"), "stderr={err:?}");
}

/// A `:`-separated empty `PATH` entry means `.` (bash) and stays relative
/// under strict: the scratch-root program is what the search finds.
#[test]
fn strict_empty_path_entry_is_the_working_dir() {
    let dir = Scratch::new();
    program(&dir, "fdprobe", EXEC_OK);
    let out = run_path(&dir, ":nonexistent", "shopt -s strict; fdprobe");
    assert_eq!(code(&out), 42, "stderr={}", stderr(&out));
}

/// Strict bans an absolute program word, and `exec` inherits the ban.
#[test]
fn strict_bans_absolute_program_path() {
    let dir = Scratch::new();
    program(&dir, "fdprobe", EXEC_OK);
    let abs = dir.join("fdprobe").to_str().unwrap().to_string();
    for form in [&abs[..], &format!("exec {abs}")[..]] {
        let out = run_path(&dir, ".", &format!("shopt -s strict; {form}"));
        assert!(!out.status.success(), "{form}: stdout={:?}", stdout(&out));
        let err = stderr(&out);
        assert!(err.contains("strict"), "{form}: stderr={err:?}");
        assert!(err.contains("absolute"), "{form}: stderr={err:?}");
    }
}

/// The strict base is the shell's pinned `%CWD` fd, so it follows `cd`: after
/// `cd in` the relative target lands in `in/`, not in the launch directory.
/// The process CWD stays at the scratch root, so this is what distinguishes
/// the pinned fd from `AT_FDCWD` (LESSONS).
#[test]
fn strict_redirect_base_follows_cd() {
    let dir = Scratch::new();
    std::fs::create_dir(dir.join("in")).unwrap();
    let out = run_path(&dir, ".", "shopt -s strict; cd in; builtin echo hi > f");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(
        std::fs::read_to_string(dir.join("in").join("f")).unwrap(),
        "hi\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("f")).unwrap(),
        "hello\n",
        "the launch dir must not get the file"
    );
}

/// The PATH search follows `%CWD` too: `in/probe` runs after `cd in` even
/// though the process CWD holds a decoy `probe` that cannot be exec'd.
#[test]
fn strict_exec_search_base_follows_cd() {
    let dir = Scratch::new();
    std::fs::create_dir(dir.join("in")).unwrap();
    program(&dir.join("in"), "probe", EXEC_OK);
    // Decoy: readable (so an `O_PATH` probe succeeds) but not executable.
    std::fs::write(dir.join("probe"), b"decoy").unwrap();
    std::fs::set_permissions(dir.join("probe"), PermissionsExt::from_mode(0o644)).unwrap();
    let out = run_path(&dir, ".", "shopt -s strict; cd in; probe");
    assert_eq!(code(&out), 42, "stderr={}", stderr(&out));
}

/// A relative `PATH` pin is what `hash` stores and `type` prints: the joined
/// relative pathname (no `./` prefix for a `.` entry), and re-running the name
/// re-opens the pin through the same base fd (ban and pin agree).
#[test]
fn strict_hash_and_type_print_relative_pins() {
    let dir = Scratch::new();
    program(&dir, "fdprobe", EXEC_OK);
    let out = run_path(
        &dir,
        ".",
        "shopt -s strict; fdprobe; type fdprobe; hash fdprobe .; type fdprobe",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let out = stdout(&out);
    assert!(
        out.contains("fdprobe is fdprobe"),
        "PATH-resolved pin must be relative, stdout={out:?}"
    );
}

/// Command substitution resolves external programs through the same strict
/// machinery (relative `PATH` entry, base fd, fd-based exec).
#[test]
fn strict_command_substitution_runs_relative_program() {
    let dir = Scratch::new();
    program(&dir, "fdprint", ARG_PRINT);
    let out = run_path(&dir, ".", "shopt -s strict; builtin echo $(fdprint SUB)");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "[SUB]\n");
}

/// Strict off is byte-for-byte the old behavior: absolute redirect targets,
/// absolute program words and absolute `PATH` entries all still resolve.
#[test]
fn non_strict_absolute_routes_unchanged() {
    let dir = Scratch::new();
    program(&dir, "fdprobe", EXEC_OK);
    let abs = dir.join("fdprobe").to_str().unwrap().to_string();
    let out = run(&dir, &format!("echo hi > {abs}.out"));
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(
        std::fs::read_to_string(dir.join("fdprobe.out")).unwrap(),
        "hi\n"
    );
    let out = run_path(&dir, dir.to_str().unwrap(), "fdprobe");
    assert_eq!(code(&out), 42, "stderr={}", stderr(&out));
    let out = run_path(&dir, "/bin", &abs);
    assert_eq!(code(&out), 42, "stderr={}", stderr(&out));
}
