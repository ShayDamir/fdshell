#![allow(clippy::unwrap_used)]
use super::*;
use alloc::{format, vec};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). The pid-named script file lives inside this dir now, so the
/// `remove_file` teardown is the guard's job. Removal is `.ok()`-tolerant:
/// teardown never panics, and it runs on a failing test's unwind, which is
/// exactly the test that used to leak.
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

/// A pid + counter scratch dir named `fdshell_script_test-<pid>-<counter>`
/// (the old root-level `fdshell_script_test_<pid>.sh` file lives inside it
/// now), removed when the test's guard drops.
fn scratch() -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell_script_test-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// CLI level: the deque starts with the script path. `main.rs` pops that front
/// and seeds `$0` with it, so the script sees `$1..` = the words after the path.
#[test]
fn load_script_source_reads_file_and_sets_positional() {
    let dir = scratch();
    let path = dir.join("script.sh");
    std::fs::write(&path, b"builtin echo hello\n").unwrap();
    let path_str = sys::ShortCStr::from_vec(path.to_str().unwrap().as_bytes().to_vec()).unwrap();
    let parsed = CliArgs {
        dirfd: None,
        script_fd: None,
        script_origin: None,
        positional: vec![sys::ImportedStr::new(
            path_str.clone(),
            sys::Trace::boundary(sys::Origin::CliArgument(1)),
        )],
    };
    let res = load_script_source(&parsed).unwrap();
    let (content, pos, origin) = res.unwrap();
    assert_eq!(content, b"builtin echo hello\n");
    assert_eq!(pos.len(), 1);
    assert_eq!(origin, sys::Origin::File(path_str));
}
