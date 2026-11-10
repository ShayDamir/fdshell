#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use std::ops::Deref;
use std::path::PathBuf;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::redirect::OpenRedirectError;
use crate::mask::MaskBit::Unquoted;
use crate::options::{NOCLOBBER, STRICT};
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource, open_redirect_files};
use crate::state::ShellState;
use sys::fcntl::{O_DIRECTORY, O_RDONLY};
use sys::pipe::pipe2;

static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it runs
/// on a failing test's unwind, which is exactly the test that used to leak.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A scratch dir with `a1 a2` (unit-test cwd is the repo root, so the pattern
/// below is absolute). The returned pair is the dir path plus its drop guard
/// — bind both (`let (dir, _scratch) = scratch();`).
fn scratch() -> (String, Scratch) {
    let c = COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-redir-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a1", "a2"] {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    (String::from(dir.to_str().unwrap()), Scratch(dir))
}

#[test]
fn multi_match_target_is_ambiguous_redirect() {
    let cell = ForkCell::new(ShellState::new());
    let (dir, _scratch) = scratch();
    let path = ShortCStr::from_vec(format!("{dir}/a*").into_bytes()).unwrap();
    let def = RedirectDef {
        export_to: 1,
        extra_to: None,
        direction: RedirectDirection::Write,
        source: RedirectSource::path(path, Vec::new()),
    };
    let report = open_redirect_files(&[def], &cell).err().unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::AmbiguousRedirect { .. }
    ));
}

#[test]
fn single_match_target_opens_the_match() {
    let cell = ForkCell::new(ShellState::new());
    let (dir, _scratch) = scratch();
    let path = ShortCStr::from_vec(format!("{dir}/a1").into_bytes()).unwrap();
    let def = RedirectDef {
        export_to: 1,
        extra_to: None,
        direction: RedirectDirection::Write,
        source: RedirectSource::path(path, Vec::new()),
    };
    let fds = open_redirect_files(&[def], &cell).unwrap();
    assert_eq!(fds.len(), 1);
}

/// A cell with `%CWD` pinned to `dir` and the given options set: the launcher
/// here is the unit test, so strict mode's route (`redirect/open/strict.rs`)
/// runs in-process — integration tests spawn the binary, which the coverage
/// build does not instrument.
fn cell_with(dir: &str, options: u32) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    let fd = sys::openat2::open(CString::new(dir).unwrap().as_c_str(), O_DIRECTORY).unwrap();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.insert_cwd(fd);
        state.options |= options;
    }
    cell
}

fn write_def(path: &str) -> RedirectDef {
    RedirectDef {
        export_to: 1,
        extra_to: None,
        direction: RedirectDirection::Write,
        source: RedirectSource::path(
            ShortCStr::from_vec(path.as_bytes().to_vec()).unwrap(),
            Vec::new(),
        ),
    }
}

/// The `&>` family's def: fd 1 (`export_to`) and fd 2 (`extra_to`) share the
/// def's single open (task #251).
fn combined_def(path: &str) -> RedirectDef {
    RedirectDef {
        export_to: 1,
        extra_to: Some(2),
        direction: RedirectDirection::Write,
        source: RedirectSource::path(
            ShortCStr::from_vec(path.as_bytes().to_vec()).unwrap(),
            Vec::new(),
        ),
    }
}

/// The single-open pin: one `combined` def yields **one** opened fd, above
/// every target of the command, and the path is opened exactly once (fd 2 is
/// fed by a clone of that fd in `resolve`, not by a second open).
#[test]
fn combined_def_opens_the_path_once() {
    let (dir, _scratch) = scratch();
    let abs = format!("{dir}/a1");
    let cell = ForkCell::new(ShellState::new());
    let fds = open_redirect_files(&[combined_def(&abs)], &cell).unwrap();
    assert_eq!(
        fds.len(),
        1,
        "one operator, one open: fd 2 is a clone, not a second open"
    );
    assert!(
        fds.first().unwrap().fd.as_raw() >= 3,
        "the open sits above the def's highest target (fd 2)",
    );
}

/// The same under strict mode: the combined target is opened once through the
/// pinned `%CWD` base fd.
#[test]
fn strict_combined_def_opens_the_target_once() {
    let (dir, _scratch) = scratch();
    std::fs::remove_file(format!("{dir}/a1")).unwrap();
    let fds = open_redirect_files(&[combined_def("a1")], &cell_with(&dir, STRICT)).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&format!("{dir}/a1")).exists());
}

/// `noclobber` gates the combined `&>` exactly like the plain `>`: its
/// direction is `Write`, so an existing target is refused and untouched.
#[test]
fn noclobber_rejects_existing_combined_target() {
    let (dir, _scratch) = scratch();
    std::fs::write(format!("{dir}/a1"), b"keep").unwrap();
    let abs = format!("{dir}/a1");
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= NOCLOBBER;
    }
    let report = open_redirect_files(&[combined_def(&abs)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::Noclobber { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(format!("{dir}/a1")).unwrap(),
        "keep"
    );
}

/// The fresh target under `noclobber` is created **once**: a second exclusive
/// open of the same path would hit the file the first open just created and
/// report `Noclobber` (the two-open bug, task #251).
#[test]
fn noclobber_creates_fresh_combined_target() {
    let (dir, _scratch) = scratch();
    std::fs::remove_file(format!("{dir}/a1")).unwrap();
    let abs = format!("{dir}/a1");
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= NOCLOBBER;
    }
    let fds = open_redirect_files(&[combined_def(&abs)], &cell).unwrap();
    assert_eq!(fds.len(), 1, "the fresh target is opened once");
}

/// Strict opens a relative target through the pinned `%CWD` fd: the word is
/// globbed and opened at the base, so the file appears in the pinned dir.
#[test]
fn strict_relative_target_opens_through_the_base_fd() {
    let (dir, _scratch) = scratch();
    std::fs::remove_file(format!("{dir}/a1")).unwrap();
    let fds = open_redirect_files(&[write_def("a1")], &cell_with(&dir, STRICT)).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&format!("{dir}/a1")).exists());
}

/// Strict bans an absolute target before the glob walk (the filesystem-root
/// resolution the ban exists for); the same word opens non-strict.
#[test]
fn strict_absolute_target_is_banned() {
    let (dir, _scratch) = scratch();
    let abs = format!("{dir}/a1");
    let report = open_redirect_files(&[write_def(&abs)], &cell_with(&dir, STRICT))
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::StrictAbsoluteTarget { .. }
    ));
}

/// A relative pattern is globbed at the base fd, so two matches are bash's
/// ambiguous redirect even though the process CWD holds neither name.
#[test]
fn strict_relative_pattern_multi_match_is_ambiguous() {
    let (dir, _scratch) = scratch();
    let report = open_redirect_files(&[write_def("a*")], &cell_with(&dir, STRICT))
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::AmbiguousRedirect { .. }
    ));
}

/// `noclobber` under strict is the exclusive create through the base fd: an
/// existing target is refused, and the target keeps its content.
#[test]
fn strict_noclobber_rejects_existing_target() {
    let (dir, _scratch) = scratch();
    std::fs::write(format!("{dir}/a1"), b"keep").unwrap();
    let report = open_redirect_files(&[write_def("a1")], &cell_with(&dir, STRICT | NOCLOBBER))
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::Noclobber { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(format!("{dir}/a1")).unwrap(),
        "keep"
    );
}

/// Non-strict `noclobber` keeps its process-CWD exclusive create: an existing
/// target is refused with the same error strict mode raises.
#[test]
fn noclobber_rejects_existing_target() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= NOCLOBBER;
    }
    let abs = format!("{dir}/a1");
    let report = open_redirect_files(&[write_def(&abs)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::Noclobber { .. }
    ));
}

/// The process-CWD twin of `strict_noclobber_propagates_non_eexist_open_failure`:
/// an exclusive create that fails with an errno **other than** `EEXIST` (here
/// `ENOENT`: `sub/` is absent from the scratch dir) propagates the open failure,
/// so the guard on `EEXIST` stays exact (task #479's gate run).
#[test]
fn noclobber_propagates_non_eexist_open_failure() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= NOCLOBBER;
    }
    let abs = format!("{dir}/sub/ghost");
    let report = open_redirect_files(&[write_def(&abs)], &cell)
        .err()
        .unwrap();
    assert!(matches!(report.current_context(), OpenRedirectError::Open));
}

/// `noclobber` under strict creates a *fresh* target through the base fd: the
/// refusal above is about the existing file, not about strict mode.
#[test]
fn strict_noclobber_creates_fresh_target() {
    let (dir, _scratch) = scratch();
    let fds =
        open_redirect_files(&[write_def("fresh")], &cell_with(&dir, STRICT | NOCLOBBER)).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&format!("{dir}/fresh")).exists());
}

/// A strict noclobber open that fails with an errno other than `EEXIST` (here
/// `ENOENT`: the parent dir `sub/` is absent under the pinned base) propagates
/// the open failure with its own errno — `Noclobber` is only the `EEXIST`
/// refusal, so the guard on that errno must stay exact.
#[test]
fn strict_noclobber_propagates_non_eexist_open_failure() {
    let (dir, _scratch) = scratch();
    let report = open_redirect_files(
        &[write_def("sub/ghost")],
        &cell_with(&dir, STRICT | NOCLOBBER),
    )
    .err()
    .unwrap();
    assert!(matches!(report.current_context(), OpenRedirectError::Open));
}

/// The target word as the parser builds it: the word, its own mask, its
/// zero-byte quote spans and its `quoted` flag (task #278).
fn word_def(word: &str, quoted: bool) -> RedirectDef {
    RedirectDef {
        export_to: 1,
        extra_to: None,
        direction: RedirectDirection::Write,
        source: RedirectSource::path_word(
            ShortCStr::from_vec(word.as_bytes().to_vec()).unwrap(),
            vec![Unquoted; word.len()],
            Vec::new(),
            quoted,
        ),
    }
}

/// A shell variable the target word can expand: `$name` reads `state.strings`.
fn set_var(cell: &ForkCell<ShellState>, name: &str, value: &str) {
    let mut state = cell.borrow_mut().unwrap();
    state.strings.insert(
        ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap(),
        sys::ImportedStr::shell(ShortCStr::from_vec(value.as_bytes().to_vec()).unwrap()),
    );
}

/// One field, expanded: the file named by the variable opens, not the word.
#[test]
fn expanded_target_opens_the_name_it_produces() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    set_var(&cell, "n", &format!("{dir}/a1"));
    let fds = open_redirect_files(&[word_def("$n", false)], &cell).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(
        !std::path::Path::new(&format!("{dir}/$n")).exists(),
        "the word as written never becomes a file"
    );
}

/// bash's one-name rule, the 0-field arm: `> $(true)` / `> $empty` is an
/// ambiguous redirect, and nothing is opened.
#[test]
fn zero_field_target_is_ambiguous() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    set_var(&cell, "empty", "");
    let report = open_redirect_files(&[word_def("$empty", false)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::AmbiguousRedirect { .. }
    ));
    assert!(
        std::fs::read_dir(dir)
            .unwrap()
            .all(|e| e.unwrap().file_name() != "empty")
    );
}

/// The >1-field arm: `n="a b"; > $n` is bash's ambiguous redirect too.
#[test]
fn multi_field_target_is_ambiguous() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    set_var(&cell, "two", &format!("{dir}/a1 {dir}/a2"));
    let report = open_redirect_files(&[word_def("$two", false)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::AmbiguousRedirect { .. }
    ));
}

/// A target whose expansion **fails** carries the substitution error under
/// `TargetExpand`, and the cause stays in the chain: `$((1/0))` is the arith
/// division error, not an ambiguous redirect and not an open (the integration
/// pin is `tests/redirect.rs::a_failing_target_expansion_keeps_its_cause`).
#[test]
fn a_failing_expansion_reports_target_expand() {
    let (_dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    let report = open_redirect_files(&[word_def("$((1/0))", false)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::TargetExpand
    ));
    // The cause survives the `change_context`: the rendered chain names the
    // arith error (LESSONS §4: one `change_context`, the chain stays readable).
    assert!(
        format!("{report:?}").contains("division"),
        "the arith cause must stay in the chain, got {report:?}"
    );
}

/// Strict mode's absolute ban runs on the **expansion**: `> $abs` under strict
/// is the ban, not a filesystem-root open (the hole strict mode exists to
/// close, README "Strict mode").
#[test]
fn strict_ban_runs_on_the_expanded_field() {
    let (dir, _scratch) = scratch();
    let cell = cell_with(&dir, STRICT);
    set_var(&cell, "abs", &format!("{dir}/ghost"));
    let report = open_redirect_files(&[word_def("$abs", false)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::StrictAbsoluteTarget { .. }
    ));
    assert!(
        !std::path::Path::new(&format!("{dir}/ghost")).exists(),
        "the ban runs before any filesystem access"
    );
}

/// A relative expansion is globbed at the pinned `%CWD` base, not at the
/// process CWD: two matches under the base are the ambiguous redirect.
#[test]
fn expanded_target_globs_at_the_strict_base() {
    let (dir, _scratch) = scratch();
    let cell = cell_with(&dir, STRICT);
    set_var(&cell, "pat", "a*");
    let report = open_redirect_files(&[word_def("$pat", false)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::AmbiguousRedirect { .. }
    ));
}

/// The redirect's own policy on a **pattern** field with no match: the literal
/// word opens (`nullglob` is task #369, measured against bash: bash deletes the
/// pattern and reports `ambiguous redirect`; fdshell opens the pattern).
#[test]
fn expanded_pattern_with_no_match_keeps_its_literal_word() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= crate::options::NULLGLOB;
    }
    set_var(&cell, "pat", &format!("{dir}/zzz*"));
    let fds = open_redirect_files(&[word_def("$pat", false)], &cell).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&format!("{dir}/zzz*")).exists());
}

/// The `%` byte stays out of the target's trigger gate (measured bash parity):
/// a `%`-only word never reaches the substitution, so `> a%b` is the literal
/// file name and the `%fd` arm never runs on it.
#[test]
fn percent_only_target_keeps_the_literal_fast_path() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    let abs = format!("{dir}/a%b");
    let fds = open_redirect_files(&[write_def(&abs)], &cell).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&abs).exists());
}

/// `noclobber` on an expanded name refuses, and names the **expanded** target
/// (bash: `set -C; n=f; echo hi > $n` → `f: cannot overwrite existing file`).
#[test]
fn noclobber_reports_the_expanded_target_name() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= NOCLOBBER;
    }
    let abs = format!("{dir}/a1");
    set_var(&cell, "n", &abs);
    let report = open_redirect_files(&[word_def("$n", false)], &cell)
        .err()
        .unwrap();
    let OpenRedirectError::Noclobber { name } = report.current_context() else {
        panic!(
            "expected a noclobber refusal, got {:?}",
            report.current_context()
        );
    };
    assert_eq!(name.as_bytes().unwrap(), abs.as_bytes());
}

/// A plain write def on an explicit target fd (the order pins below need the
/// fd-2 half of the pair, which `write_def` fixes to fd 1).
fn write_def_to(export_to: i32, path: &str) -> RedirectDef {
    RedirectDef {
        export_to,
        extra_to: None,
        direction: RedirectDirection::Write,
        source: RedirectSource::path(
            ShortCStr::from_vec(path.as_bytes().to_vec()).unwrap(),
            Vec::new(),
        ),
    }
}

/// The open pre-pass runs in the order the defs were **written** and stops at
/// the first failure, so the losers' side effects follow that order too:
/// `echo x 2>missing/dir/e >f` never opens `f`, and bash creates no `f`
/// either (rc 1). A fd-sorted def list opens fd 1's target first, so `f` is
/// truncated (here: emptied) before the fd-2 open fails — a filesystem effect
/// of a redirection the command never applied. See
/// LESSONS: "POSIX redirect order is observable" (task #337); the integration
/// twin is
/// `tests/redirect.rs::an_earlier_failed_open_creates_no_later_target`.
#[test]
fn opens_run_in_source_order_and_stop_at_the_first_failure() {
    let (dir, _scratch) = scratch();
    std::fs::write(format!("{dir}/a1"), b"keep\n").unwrap();
    let cell = ForkCell::new(ShellState::new());
    let defs = vec![
        write_def_to(2, &format!("{dir}/nope/dir/e")),
        write_def_to(1, &format!("{dir}/a1")),
    ];
    let report = open_redirect_files(&defs, &cell).err().unwrap();
    assert!(
        matches!(report.current_context(), OpenRedirectError::Open),
        "the fd-2 open of a missing directory fails: {:?}",
        report.current_context()
    );
    assert_eq!(
        std::fs::read_to_string(format!("{dir}/a1")).unwrap(),
        "keep\n",
        "the later target is never opened, so it keeps its bytes"
    );
}

// --- the magic decision on the final expanded name (task #479) --------------

/// The decision runs once, on the final expanded name: a word whose expansion
/// produces the magic form flags the entry, and the fd is the **real pre-open**
/// (the pass owns it; its success is still judged against the pre-command fds —
/// divergence #482 keeps that rule).
#[test]
fn an_expanded_magic_word_flags_the_entry() {
    let cell = ForkCell::new(ShellState::new());
    set_var(&cell, "p", "/dev/fd/1");
    let entries = open_redirect_files(&[word_def("$p", false)], &cell).unwrap();
    let entry = entries.first().unwrap();
    assert_eq!(entry.magic, Some(("/dev/fd/", 1)));
    entry.fd.verify().unwrap();
}

/// The other arm of the decision: an ordinary word leaves the flag `None`,
/// even though its open runs through the very same pipeline.
#[test]
fn an_ordinary_word_leaves_the_flag_none() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    let abs = format!("{dir}/a1");
    let entries = open_redirect_files(&[write_def(&abs)], &cell).unwrap();
    let entry = entries.first().unwrap();
    assert_eq!(entry.magic, None);
    entry.fd.verify().unwrap();
}

/// The parser stores the word **without its quotes**, so the quoted spelling
/// flags exactly like the plain canonical one (the row bash already matched).
#[test]
fn a_quoted_magic_word_flags_the_entry() {
    let cell = ForkCell::new(ShellState::new());
    let entries = open_redirect_files(&[word_def("/dev/fd/1", true)], &cell).unwrap();
    assert_eq!(entries.first().unwrap().magic, Some(("/dev/fd/", 1)));
}

/// Escape pairs fold before the decision (POSIX 2.2.1: bash's tokenizer folds
/// them at tokenization), so the folded name is magic too.
#[test]
fn an_escape_folded_magic_word_flags_the_entry() {
    let cell = ForkCell::new(ShellState::new());
    let entries = open_redirect_files(&[write_def("/dev/fd/\\1")], &cell).unwrap();
    assert_eq!(entries.first().unwrap().magic, Some(("/dev/fd/", 1)));
}

/// Strict mode's absolute ban runs **before** the decision, on the word (fast
/// path) or every expanded field (substitution path), so no magic name can
/// exist under strict: the expanded magic word is the ban, and the relative
/// word's entry keeps the flag `None`.
#[test]
fn strict_bans_the_expanded_magic_word_before_the_flag() {
    let (dir, _scratch) = scratch();
    let cell = cell_with(&dir, STRICT);
    set_var(&cell, "m", "/dev/fd/1");
    let report = open_redirect_files(&[word_def("$m", false)], &cell)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::StrictAbsoluteTarget { .. }
    ));
    let entries = open_redirect_files(&[write_def("a1")], &cell_with(&dir, STRICT)).unwrap();
    assert_eq!(entries.first().unwrap().magic, None);
}

/// `set -C` on an **expanded** magic word whose fd is a **regular** file the
/// test owns: the pass's own pre-open judges the exclusive create against the
/// pre-command fds and refuses — the open-decides-success boundary, so the
/// magic reading does not move this refusal (fdshell policy). The harness's
/// fd 1 backs no unit test (LESSONS: "The noclobber refusal is bash's own stat
/// gate, not kernel leniency"): a pipe-backed magic word now **opens** (bash
/// parity — the rows below), so this one spells its word with an fd it owns.
#[test]
fn noclobber_refuses_an_expanded_magic_word() {
    let (dir, _scratch) = scratch();
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= NOCLOBBER;
    }
    let backed = sys::openat2::open(
        CString::new(format!("{dir}/a1")).unwrap().as_c_str(),
        O_RDONLY,
    )
    .unwrap();
    let word = format!("/dev/fd/{}", backed.as_raw());
    set_var(&cell, "p", &word);
    let report = open_redirect_files(&[word_def("$p", false)], &cell)
        .err()
        .unwrap();
    let OpenRedirectError::Noclobber { name } = report.current_context() else {
        panic!(
            "expected a noclobber refusal, got {:?}",
            report.current_context()
        );
    };
    assert_eq!(name.as_bytes().unwrap(), word.as_bytes());
}

// --- noclobber's stat gate on non-regular targets (task #480) ---------------
//
// bash refuses only **regular** files; every other existing target opens with
// the direction's own flags (rows measured 2026-11-10 against bash 5.3.9 —
// the kernel answers `EEXIST` to `O_CREAT|O_EXCL` for **all** of these types,
// so bash's own stat is the gate, not kernel leniency). A FIFO has no unit
// row: its open blocks without a reader. Each non-strict row has a strict
// twin through `cell_with(&dir, STRICT | NOCLOBBER)` with a relative word;
// the twin reaches the same target **type** through a symlink in the scratch
// dir (strict bans absolute words; the link bodies may be absolute — `statx`
// follows and `openat2` runs with `resolve: 0`, one consistent pair). The
// end-to-end rows live in `safe/fdshell/tests/redirect.rs`.

/// A non-strict cell with `noclobber` set (the pattern of the rows above).
fn noclobber_cell() -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().options |= NOCLOBBER;
    cell
}

/// The gate judges the symlink **through** the link (bash's `stat`, not
/// `lstat`): the refusal, and the file keeps its bytes — an `lstat` would
/// plain-open the link and truncate the existing file.
#[test]
fn noclobber_refuses_a_symlink_to_a_regular_file() {
    let (dir, _scratch) = scratch();
    std::fs::write(format!("{dir}/a1"), b"keep").unwrap();
    std::os::unix::fs::symlink("a1", format!("{dir}/link")).unwrap();
    let abs = format!("{dir}/link");
    let report = open_redirect_files(&[write_def(&abs)], &noclobber_cell())
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::Noclobber { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(format!("{dir}/a1")).unwrap(),
        "keep"
    );
}

#[test]
fn strict_noclobber_refuses_a_symlink_to_a_regular_file() {
    let (dir, _scratch) = scratch();
    std::fs::write(format!("{dir}/a1"), b"keep").unwrap();
    std::os::unix::fs::symlink("a1", format!("{dir}/link")).unwrap();
    let report = open_redirect_files(&[write_def("link")], &cell_with(&dir, STRICT | NOCLOBBER))
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::Noclobber { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(format!("{dir}/a1")).unwrap(),
        "keep"
    );
}

/// An existing **character device** opens, not refused (bash rc 0, measured):
/// the refusal is bash's stat gate, and the kernel's `EEXIST` is not.
#[test]
fn noclobber_opens_an_existing_character_device() {
    let (_dir, _scratch) = scratch();
    let fds = open_redirect_files(&[write_def("/dev/null")], &noclobber_cell()).unwrap();
    assert_eq!(fds.len(), 1, "the plain arm opens the character device");
}

#[test]
fn strict_noclobber_opens_a_symlink_to_the_character_device() {
    let (dir, _scratch) = scratch();
    std::os::unix::fs::symlink("/dev/null", format!("{dir}/lnull")).unwrap();
    let fds =
        open_redirect_files(&[write_def("lnull")], &cell_with(&dir, STRICT | NOCLOBBER)).unwrap();
    assert_eq!(fds.len(), 1, "the plain arm opens through the link");
}

/// A **pipe** reached through the magic form opens (the task's row: bash rc 0,
/// prints `e`). The test owns the pipe (`pipe2`, the pattern the crate's tests
/// use) — the harness's fd 1 is not a test's to assert on (LESSONS: "The
/// noclobber refusal is bash's own stat gate, not kernel leniency").
#[test]
fn noclobber_opens_a_pipe_reached_through_the_magic_form() {
    let (_dir, _scratch) = scratch();
    let (_rd, wr) = pipe2(0).unwrap();
    let word = format!("/dev/fd/{}", wr.as_raw());
    let fds = open_redirect_files(&[write_def(&word)], &noclobber_cell()).unwrap();
    assert_eq!(fds.len(), 1, "the plain arm opens the pipe");
}

#[test]
fn strict_noclobber_opens_a_symlink_to_the_pipe() {
    let (dir, _scratch) = scratch();
    let (_rd, wr) = pipe2(0).unwrap();
    std::os::unix::fs::symlink(
        format!("/proc/self/fd/{}", wr.as_raw()),
        format!("{dir}/lpipe"),
    )
    .unwrap();
    let fds =
        open_redirect_files(&[write_def("lpipe")], &cell_with(&dir, STRICT | NOCLOBBER)).unwrap();
    assert_eq!(fds.len(), 1, "the plain arm opens through the link");
}

/// A **directory** is not refused either: the plain arm opens it and the
/// kernel's own errno answers (bash: `Is a directory`, rc 1).
#[test]
fn noclobber_reports_the_errno_on_a_directory() {
    let (dir, _scratch) = scratch();
    std::fs::create_dir(format!("{dir}/sub")).unwrap();
    let abs = format!("{dir}/sub");
    let report = open_redirect_files(&[write_def(&abs)], &noclobber_cell())
        .err()
        .unwrap();
    assert!(matches!(report.current_context(), OpenRedirectError::Open));
}

#[test]
fn strict_noclobber_reports_the_errno_on_a_directory_via_symlink() {
    let (dir, _scratch) = scratch();
    std::fs::create_dir(format!("{dir}/sub")).unwrap();
    std::os::unix::fs::symlink("sub", format!("{dir}/ldir")).unwrap();
    let report = open_redirect_files(&[write_def("ldir")], &cell_with(&dir, STRICT | NOCLOBBER))
        .err()
        .unwrap();
    assert!(matches!(report.current_context(), OpenRedirectError::Open));
}

/// The exclusive-create arm, made deterministic by a **dangling symlink**: the
/// stat fails (bash's "the file was not present" arm) and the kernel's own
/// exclusivity check then hits the existing link (bash measures the same row —
/// refusal, nothing created — bash 5.3.9, measured 2026-11-10). The `EEXIST`
/// guard lives exactly here: the race the exclusive create exists for.
#[test]
fn noclobber_refuses_a_dangling_symlink() {
    let (dir, _scratch) = scratch();
    std::os::unix::fs::symlink("ghost", format!("{dir}/link")).unwrap();
    let abs = format!("{dir}/link");
    let report = open_redirect_files(&[write_def(&abs)], &noclobber_cell())
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::Noclobber { .. }
    ));
    assert!(
        !std::path::Path::new(&format!("{dir}/ghost")).exists(),
        "the refusal happens before the create"
    );
}

#[test]
fn strict_noclobber_refuses_a_dangling_symlink() {
    let (dir, _scratch) = scratch();
    std::os::unix::fs::symlink("ghost", format!("{dir}/ldang")).unwrap();
    let report = open_redirect_files(&[write_def("ldang")], &cell_with(&dir, STRICT | NOCLOBBER))
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::Noclobber { .. }
    ));
    assert!(
        !std::path::Path::new(&format!("{dir}/ghost")).exists(),
        "the refusal happens before the create"
    );
}
