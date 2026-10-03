#![allow(clippy::unwrap_used)]

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::redirect::OpenRedirectError;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource, open_redirect_files};
use crate::state::ShellState;

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
        direction: RedirectDirection::Write,
        source: RedirectSource::path(path, Vec::new()),
    };
    let fds = open_redirect_files(&[def], &cell).unwrap();
    assert_eq!(fds.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
