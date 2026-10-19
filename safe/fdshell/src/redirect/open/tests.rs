#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::redirect::OpenRedirectError;
use crate::options::{NOCLOBBER, STRICT};
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource, open_redirect_files};
use crate::state::ShellState;
use sys::fcntl::O_DIRECTORY;

static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// A scratch dir with `a1 a2` (unit-test cwd is the repo root, so the pattern
/// below is absolute).
fn scratch() -> String {
    let c = COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-redir-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a1", "a2"] {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    String::from(dir.to_str().unwrap())
}

#[test]
fn multi_match_target_is_ambiguous_redirect() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
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
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn single_match_target_opens_the_match() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
    let path = ShortCStr::from_vec(format!("{dir}/a1").into_bytes()).unwrap();
    let def = RedirectDef {
        export_to: 1,
        extra_to: None,
        direction: RedirectDirection::Write,
        source: RedirectSource::path(path, Vec::new()),
    };
    let fds = open_redirect_files(&[def], &cell).unwrap();
    assert_eq!(fds.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
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
    let dir = scratch();
    let abs = format!("{dir}/a1");
    let cell = ForkCell::new(ShellState::new());
    let fds = open_redirect_files(&[combined_def(&abs)], &cell).unwrap();
    assert_eq!(
        fds.len(),
        1,
        "one operator, one open: fd 2 is a clone, not a second open"
    );
    assert!(
        fds.first().unwrap().as_raw() >= 3,
        "the open sits above the def's highest target (fd 2)",
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same under strict mode: the combined target is opened once through the
/// pinned `%CWD` base fd.
#[test]
fn strict_combined_def_opens_the_target_once() {
    let dir = scratch();
    std::fs::remove_file(format!("{dir}/a1")).unwrap();
    let fds = open_redirect_files(&[combined_def("a1")], &cell_with(&dir, STRICT)).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&format!("{dir}/a1")).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `noclobber` gates the combined `&>` exactly like the plain `>`: its
/// direction is `Write`, so an existing target is refused and untouched.
#[test]
fn noclobber_rejects_existing_combined_target() {
    let dir = scratch();
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
    let _ = std::fs::remove_dir_all(&dir);
}

/// The fresh target under `noclobber` is created **once**: a second exclusive
/// open of the same path would hit the file the first open just created and
/// report `Noclobber` (the two-open bug, task #251).
#[test]
fn noclobber_creates_fresh_combined_target() {
    let dir = scratch();
    std::fs::remove_file(format!("{dir}/a1")).unwrap();
    let abs = format!("{dir}/a1");
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options |= NOCLOBBER;
    }
    let fds = open_redirect_files(&[combined_def(&abs)], &cell).unwrap();
    assert_eq!(fds.len(), 1, "the fresh target is opened once");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Strict opens a relative target through the pinned `%CWD` fd: the word is
/// globbed and opened at the base, so the file appears in the pinned dir.
#[test]
fn strict_relative_target_opens_through_the_base_fd() {
    let dir = scratch();
    std::fs::remove_file(format!("{dir}/a1")).unwrap();
    let fds = open_redirect_files(&[write_def("a1")], &cell_with(&dir, STRICT)).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&format!("{dir}/a1")).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Strict bans an absolute target before the glob walk (the filesystem-root
/// resolution the ban exists for); the same word opens non-strict.
#[test]
fn strict_absolute_target_is_banned() {
    let dir = scratch();
    let abs = format!("{dir}/a1");
    let report = open_redirect_files(&[write_def(&abs)], &cell_with(&dir, STRICT))
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::StrictAbsoluteTarget { .. }
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A relative pattern is globbed at the base fd, so two matches are bash's
/// ambiguous redirect even though the process CWD holds neither name.
#[test]
fn strict_relative_pattern_multi_match_is_ambiguous() {
    let dir = scratch();
    let report = open_redirect_files(&[write_def("a*")], &cell_with(&dir, STRICT))
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::AmbiguousRedirect { .. }
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `noclobber` under strict is the exclusive create through the base fd: an
/// existing target is refused, and the target keeps its content.
#[test]
fn strict_noclobber_rejects_existing_target() {
    let dir = scratch();
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
    let _ = std::fs::remove_dir_all(&dir);
}

/// Non-strict `noclobber` keeps its process-CWD exclusive create: an existing
/// target is refused with the same error strict mode raises.
#[test]
fn noclobber_rejects_existing_target() {
    let dir = scratch();
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
    let _ = std::fs::remove_dir_all(&dir);
}

/// `noclobber` under strict creates a *fresh* target through the base fd: the
/// refusal above is about the existing file, not about strict mode.
#[test]
fn strict_noclobber_creates_fresh_target() {
    let dir = scratch();
    let fds =
        open_redirect_files(&[write_def("fresh")], &cell_with(&dir, STRICT | NOCLOBBER)).unwrap();
    assert_eq!(fds.len(), 1);
    assert!(std::path::Path::new(&format!("{dir}/fresh")).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A strict noclobber open that fails with an errno other than `EEXIST` (here
/// `ENOENT`: the parent dir `sub/` is absent under the pinned base) propagates
/// the open failure with its own errno — `Noclobber` is only the `EEXIST`
/// refusal, so the guard on that errno must stay exact.
#[test]
fn strict_noclobber_propagates_non_eexist_open_failure() {
    let dir = scratch();
    let report = open_redirect_files(
        &[write_def("sub/ghost")],
        &cell_with(&dir, STRICT | NOCLOBBER),
    )
    .err()
    .unwrap();
    assert!(matches!(report.current_context(), OpenRedirectError::Open));
    let _ = std::fs::remove_dir_all(&dir);
}
