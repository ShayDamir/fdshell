#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::ffi::CString;
use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use sys::fcntl::{O_RDONLY, O_WRONLY};
use sys::memfd::memfd_create;
use sys::openat2::open;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch dir; each test gets its own (pid + counter, per LESSONS).
fn scratch() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-ficlone-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cstr(p: &std::path::Path) -> CString {
    CString::new(p.to_str().unwrap()).unwrap()
}

/// Write `data` to `dir/name` and open it `O_RDONLY` (the source form
/// `FICLONE` needs: readable).
fn open_file(dir: &std::path::Path, name: &str, data: &[u8]) -> sys::LocalFd {
    let path = dir.join(name);
    std::fs::write(&path, data).unwrap();
    open(cstr(&path).as_c_str(), O_RDONLY).unwrap()
}

/// `FICLONE` with the fs-verity skip discipline for non-reflink hosts:
/// `EOPNOTSUPP` (95) / `ENOTTY` (25) print and return `false` (skip); any
/// other errno is a hard failure.
fn clone_or_skip(src: &sys::LocalFd, dst: &sys::LocalFd) -> bool {
    match src.ficlone_to(dst) {
        Ok(()) => true,
        Err(e) => {
            let errno = e.errno();
            if errno == libc::EOPNOTSUPP || errno == libc::ENOTTY {
                eprintln!("ficlone unsupported on this host (errno {errno}); skipping");
                false
            } else {
                panic!("ficlone failed with unexpected errno {errno}");
            }
        }
    }
}

/// A 5000 B source grows a 1 B destination to 5000 B with byte-identical
/// content.
#[test]
fn reflink_grows_dst_and_shares_content() {
    let dir = scratch();
    let src_data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    let src = open_file(&dir, "src", &src_data);
    let dst_path = dir.join("dst");
    std::fs::write(&dst_path, b"x").unwrap();
    let dst = open(cstr(&dst_path).as_c_str(), O_WRONLY).unwrap();
    if !clone_or_skip(&src, &dst) {
        return;
    }
    assert_eq!(std::fs::read(&dst_path).unwrap(), src_data);
}

/// A write to the cloned destination leaves the source untouched (CoW).
#[test]
fn reflink_is_copy_on_write() {
    let dir = scratch();
    let src_data: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    let src = open_file(&dir, "src", &src_data);
    let dst_path = dir.join("dst");
    std::fs::write(&dst_path, b"").unwrap();
    let dst = open(cstr(&dst_path).as_c_str(), O_WRONLY).unwrap();
    if !clone_or_skip(&src, &dst) {
        return;
    }
    let mut w = std::fs::OpenOptions::new()
        .write(true)
        .open(&dst_path)
        .unwrap();
    w.seek(SeekFrom::Start(100)).unwrap();
    w.write_all(&[0xAB]).unwrap();
    assert_eq!(std::fs::read(dir.join("src")).unwrap(), src_data);
    let dst_after = std::fs::read(&dst_path).unwrap();
    assert_eq!(dst_after.len(), 4096);
    assert_eq!(dst_after[100], 0xAB);
}

/// Cloning a 100 B source over a non-empty 4000 B destination is refused
/// `EINVAL` (no shrinking); skip on a non-reflink host.
#[test]
fn shrink_nonempty_src_is_einval() {
    let dir = scratch();
    let src = open_file(&dir, "src", &[0u8; 100]);
    let dst_path = dir.join("dst");
    std::fs::write(&dst_path, vec![0u8; 4000]).unwrap();
    let dst = open(cstr(&dst_path).as_c_str(), O_WRONLY).unwrap();
    match src.ficlone_to(&dst) {
        Ok(()) => panic!("cloning a smaller source into a non-empty dst must fail"),
        Err(e) => {
            let errno = e.errno();
            if errno == libc::EINVAL {
                return;
            }
            assert!(
                errno == libc::EOPNOTSUPP || errno == libc::ENOTTY,
                "unexpected errno {errno}"
            );
            eprintln!("ficlone unsupported on this host (errno {errno}); skipping");
        }
    }
}

/// An empty source is a no-op: `Ok(())` and the destination size untouched.
#[test]
fn empty_source_leaves_dst_untouched() {
    let dir = scratch();
    let src = open_file(&dir, "src", b"");
    let dst_path = dir.join("dst");
    std::fs::write(&dst_path, vec![0u8; 4000]).unwrap();
    let dst = open(cstr(&dst_path).as_c_str(), O_WRONLY).unwrap();
    if !clone_or_skip(&src, &dst) {
        return;
    }
    assert_eq!(std::fs::metadata(&dst_path).unwrap().len(), 4000);
}

/// A memfd source is on its own tmpfs instance: `EXDEV` on any host.
#[test]
fn cross_filesystem_is_exdev() {
    let dir = scratch();
    let src = memfd_create().unwrap();
    let dst_path = dir.join("dst");
    std::fs::write(&dst_path, b"").unwrap();
    let dst = open(cstr(&dst_path).as_c_str(), O_WRONLY).unwrap();
    let e = src.ficlone_to(&dst).unwrap_err();
    assert_eq!(e.errno(), libc::EXDEV);
}

/// A destination not open for writing is refused `EBADF` (checked before
/// the filesystem, so deterministic on any host).
#[test]
fn read_only_dst_is_ebadf() {
    let dir = scratch();
    let src = open_file(&dir, "src", b"data");
    let dst_path = dir.join("dst");
    std::fs::write(&dst_path, b"").unwrap();
    let dst = open(cstr(&dst_path).as_c_str(), O_RDONLY).unwrap();
    let e = src.ficlone_to(&dst).unwrap_err();
    assert_eq!(e.errno(), libc::EBADF);
}

/// A source not open for reading is refused `EBADF` (checked before the
/// filesystem, so deterministic on any host).
#[test]
fn write_only_src_is_ebadf() {
    let dir = scratch();
    let src_path = dir.join("src");
    std::fs::write(&src_path, b"data").unwrap();
    let src = open(cstr(&src_path).as_c_str(), O_WRONLY).unwrap();
    let dst_path = dir.join("dst");
    std::fs::write(&dst_path, b"").unwrap();
    let dst = open(cstr(&dst_path).as_c_str(), O_WRONLY).unwrap();
    let e = src.ficlone_to(&dst).unwrap_err();
    assert_eq!(e.errno(), libc::EBADF);
}
