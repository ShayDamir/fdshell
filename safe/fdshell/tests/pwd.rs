#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::PathBuf;
use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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
        let root = std::env::temp_dir().join(format!("fdshell-pwd-{}-{}", std::process::id(), n));
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

/// `pwd` prints a CWD whose absolute path is past 4 KiB (100 levels of
/// 50-char names, ~5.1 KB) — the pre-fix fixed 4 KiB `getcwd` buffer failed
/// here. The binary is spawned with `.current_dir(".")` so the child
/// inherits this process's deep CWD; passing the deep path itself to the
/// spawn would exceed PATH_MAX.
#[test]
fn pwd_prints_deep_cwd() {
    const LEVELS: usize = 100;
    const NAME_LEN: usize = 50;
    let _tree = DeepTree::new(LEVELS, NAME_LEN);
    let out = Command::new(BIN)
        .current_dir(".")
        .args(["-c", "pwd"])
        .output()
        .unwrap();
    let expected = std::env::current_dir().unwrap();
    let path = expected.to_str().unwrap();
    assert!(path.len() > 4096, "path is only {} bytes", path.len());
    assert!(
        out.status.success(),
        "stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), format!("{path}\n"));
}

/// A deleted CWD: `pwd` exits with the errno (ENOENT = 2) and prints
/// nothing — `handle_builtin_error`'s `BuiltinError::Syscall` arm returns
/// the errno silently (the established `readlink` convention), so the exit
/// code is the user-visible error. The binary is spawned with
/// `.current_dir(".")` so the child inherits this process's (deleted) CWD.
#[test]
fn pwd_deleted_cwd_exits_errno_silently() {
    let original = std::env::current_dir().unwrap();
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("fdshell-pwd-{}-{}", std::process::id(), n));
    std::fs::create_dir(&root).unwrap();
    std::env::set_current_dir(&root).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let out = Command::new(BIN)
        .current_dir(".")
        .args(["-c", "pwd"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(out.stderr.is_empty());
    std::env::set_current_dir(&original).unwrap();
}
