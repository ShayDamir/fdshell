#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use sys::env::{getcwd, getenv};

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[test]
fn getenv_returns_set_value() {
    // SAFETY: each nextest test runs in its own process, so mutating the
    // process environment here cannot race with other tests.
    unsafe { std::env::set_var("FDSHELL_TEST_VAR", "hello_env") };
    // A stub returning None (or an empty Some) would fail this exact-value check.
    let v = getenv(c"FDSHELL_TEST_VAR");
    assert_eq!(v.unwrap().as_bytes().unwrap(), b"hello_env");
    // SAFETY: same process-isolation reasoning as above.
    unsafe { std::env::remove_var("FDSHELL_TEST_VAR") };
}

#[test]
fn getenv_unset_returns_none() {
    // SAFETY: same process-isolation reasoning as above.
    unsafe { std::env::remove_var("FDSHELL_DEFINITELY_UNSET_VAR") };
    let v = getenv(c"FDSHELL_DEFINITELY_UNSET_VAR");
    assert!(v.is_none());
}

/// At a shallow CWD the result is the exact current directory, byte for byte.
#[test]
fn getcwd_shallow_returns_cwd() {
    let expected = std::env::current_dir().unwrap();
    let actual = getcwd().unwrap();
    assert_eq!(actual, expected.to_str().unwrap().as_bytes());
}

/// A deleted CWD fails the initial 4 KiB stack attempt with ENOENT — a
/// non-too-small errno is returned as-is, no growth helps.
#[test]
fn getcwd_deleted_cwd_fails_enoent() {
    let original = std::env::current_dir().unwrap();
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("fdshell-env-{}-{}", std::process::id(), n));
    std::fs::create_dir(&root).unwrap();
    std::env::set_current_dir(&root).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let err = match getcwd() {
        Ok(_) => panic!("getcwd succeeded on a deleted CWD"),
        Err(e) => e,
    };
    assert_eq!(err.syscall(), "getcwd");
    assert_eq!(err.errno(), libc::ENOENT);
    std::env::set_current_dir(&original).unwrap();
}

/// One directory level's name: zero-padded index, `name_len` bytes wide.
fn deep_name(i: usize, name_len: usize) -> String {
    format!("{i:0name_len$}")
}

/// A scratch root with `levels` nested dirs; the CWD is left at the deepest
/// dir. An absolute mkdir/chdir argument is capped at PATH_MAX, so the tree
/// is built one relative component at a time. Removed best-effort on drop,
/// so a panicking test still gets its tree back out of /tmp.
struct DeepTree {
    root: PathBuf,
    original: PathBuf,
    levels: usize,
    name_len: usize,
}

impl DeepTree {
    fn new(levels: usize, name_len: usize) -> Self {
        let original = std::env::current_dir().unwrap();
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("fdshell-env-{}-{}", std::process::id(), n));
        std::fs::create_dir(&root).unwrap();
        // Build the guard before descending so a mid-descent panic still
        // runs the best-effort cleanup.
        let tree = Self {
            root,
            original,
            levels,
            name_len,
        };
        std::env::set_current_dir(&tree.root).unwrap();
        for i in 0..levels {
            let name = deep_name(i, name_len);
            std::fs::create_dir(&name).unwrap();
            std::env::set_current_dir(&name).unwrap();
        }
        tree
    }
}

impl Drop for DeepTree {
    fn drop(&mut self) {
        // Best-effort: descend to the deepest level that still exists, then
        // chdir("..") + rmdir per level — every path argument is one relative
        // component (a full deep path would exceed PATH_MAX), and every error
        // is ignored so cleanup always runs to completion.
        let _ = std::env::set_current_dir(&self.root);
        let mut depth = 0;
        for i in 0..self.levels {
            if std::env::set_current_dir(deep_name(i, self.name_len)).is_err() {
                break;
            }
            depth = i + 1;
        }
        for i in (0..depth).rev() {
            let _ = std::env::set_current_dir("..");
            let _ = std::fs::remove_dir(deep_name(i, self.name_len));
        }
        let _ = std::env::set_current_dir(&self.original);
        let _ = std::fs::remove_dir(&self.root);
    }
}

/// A CWD past 4 KiB (100 levels of 50-char names, ~5.1 KB) is returned in
/// full — the pre-fix fixed 4 KiB buffer failed here.
#[test]
fn getcwd_returns_deep_path_over_4k() {
    const LEVELS: usize = 100;
    const NAME_LEN: usize = 50;
    let _tree = DeepTree::new(LEVELS, NAME_LEN);
    let expected = std::env::current_dir().unwrap();
    let path = expected.to_str().unwrap();
    assert!(path.len() > 4096, "path is only {} bytes", path.len());
    assert_eq!(getcwd().unwrap(), path.as_bytes());
}

/// A CWD ~10.2 KB (200 levels of 50-char names) does not fit the first
/// 8 KiB heap buffer, so it pins the growth loop: the loop's
/// `buffer_too_small(e)` match guard → `false` mutant stops growing after
/// the first heap attempt and would fail here.
#[test]
fn getcwd_returns_deep_path_over_8k() {
    const LEVELS: usize = 200;
    const NAME_LEN: usize = 50;
    let _tree = DeepTree::new(LEVELS, NAME_LEN);
    let expected = std::env::current_dir().unwrap();
    let path = expected.to_str().unwrap();
    assert!(path.len() > 8192, "path is only {} bytes", path.len());
    assert_eq!(getcwd().unwrap(), path.as_bytes());
}

/// A CWD ~768 KB (3000 levels of 255-char names) sits in the (512 KiB, 1 MiB)
/// band — it needs the full 1 MiB buffer. It pins the cap boundary: the `>`
/// → `>=` / `==` mutants on the cap check make the effective cap 512 KiB and
/// would fail here. The over-cap test cannot pin this, since it asserts
/// `Err`, which any earlier-terminating variant also satisfies.
#[test]
fn getcwd_returns_deep_path_under_cap() {
    const LEVELS: usize = 3000;
    const NAME_LEN: usize = 255;
    let _tree = DeepTree::new(LEVELS, NAME_LEN);
    let expected = std::env::current_dir().unwrap();
    let path = expected.to_str().unwrap();
    assert!(path.len() > 512 * 1024, "path is only {} bytes", path.len());
    assert!(path.len() < 1024 * 1024, "path is {} bytes", path.len());
    assert_eq!(getcwd().unwrap(), path.as_bytes());
}

/// A CWD past the 1 MiB retry cap (4097 levels of 255-char names, ~1.05 MB)
/// fails with the last too-small error instead of allocating forever.
#[test]
fn getcwd_over_cap_returns_error() {
    const LEVELS: usize = 4097;
    const NAME_LEN: usize = 255;
    let _tree = DeepTree::new(LEVELS, NAME_LEN);
    let err = match getcwd() {
        Ok(_) => panic!("getcwd succeeded past the 1 MiB cap"),
        Err(e) => e,
    };
    assert_eq!(err.syscall(), "getcwd");
    let errno = err.errno();
    assert!(
        errno == libc::ERANGE || errno == libc::ENAMETOOLONG,
        "errno {errno}"
    );
}
