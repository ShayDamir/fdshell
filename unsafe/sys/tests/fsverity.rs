#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::ffi::CString;
use sys::fcntl::O_RDONLY;
use sys::fsverity::{FS_VERITY_HASH_ALG_SHA256, FS_VERITY_HASH_ALG_SHA512};
use sys::openat2::open;

/// The content the e2e builtin tests pin; its file digest is a pure function of
/// (content, algo, block size, salt), stable across kernels.
const CONTENT: &[u8] = b"fdshell-verity-e2e\n";

const SHA256_DIGEST: &str = "512eb3c1830461d829fbe3f6b07b2c40bc49114586b3389dad43f8b195abf57b";
const SHA512_DIGEST: &str = "e296f654d46935c2afbd9b7976d08e3299fce789d7572aea31b1372d5d3f6029cf89cf602efbf1c677d952a41463a1549a1c342abd07636faca4651d15f3e976";

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir; each test gets its own (pid + counter, per LESSONS).
fn scratch() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-verity-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cstr(p: &std::path::Path) -> CString {
    CString::new(p.to_str().unwrap()).unwrap()
}

/// Write `data` to `dir/name` and open it `O_RDONLY` (the fd form `ENABLE`
/// needs: read-only on an inode the caller can write).
fn open_file(dir: &std::path::Path, name: &str, data: &[u8]) -> sys::LocalFd {
    let path = dir.join(name);
    std::fs::write(&path, data).unwrap();
    open(cstr(&path).as_c_str(), O_RDONLY).unwrap()
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `ENABLE` + `MEASURE` with sha256 returns the pinned file digest.
#[test]
fn sha256_enable_measure_roundtrip() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let d = fd.measure_verity().unwrap();
    assert_eq!(d.algorithm, 1);
    assert_eq!(d.size, 32);
    assert_eq!(to_hex(&d.digest), SHA256_DIGEST);
}

/// sha512 returns the pinned digest and reports algorithm 2, size 64.
#[test]
fn sha512_enable_measure_roundtrip() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA512, 4096).unwrap();
    let d = fd.measure_verity().unwrap();
    assert_eq!(d.algorithm, 2);
    assert_eq!(d.size, 64);
    assert_eq!(to_hex(&d.digest), SHA512_DIGEST);
}

/// The digest is a pure function of the content: equal content gives an equal
/// digest, different content gives a different one.
#[test]
fn digest_depends_on_content() {
    let dir = scratch();
    let a = open_file(&dir, "a", CONTENT);
    a.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let b = open_file(&dir, "b", CONTENT);
    b.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let c = open_file(&dir, "c", b"fdshell-verity-OTHER\n");
    c.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let da = a.measure_verity().unwrap();
    let db = b.measure_verity().unwrap();
    let dc = c.measure_verity().unwrap();
    assert_eq!(da.digest, db.digest);
    assert_ne!(da.digest, dc.digest);
}

/// Enabling verity twice on the same inode fails `EEXIST`.
#[test]
fn double_enable_is_eexist() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let e = fd
        .enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096)
        .unwrap_err();
    assert_eq!(e.errno(), libc::EEXIST);
}

/// An unknown hash algorithm is rejected `EINVAL`.
#[test]
fn unknown_algorithm_is_einval() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    let e = fd.enable_verity(99, 4096).unwrap_err();
    assert_eq!(e.errno(), libc::EINVAL);
}

/// The Merkle-tree block size must fit the fs: 1024 is accepted, 8192 rejected.
#[test]
fn block_size_bounds() {
    let dir = scratch();
    let ok = open_file(&dir, "ok", CONTENT);
    ok.enable_verity(FS_VERITY_HASH_ALG_SHA256, 1024).unwrap();
    let big = open_file(&dir, "big", CONTENT);
    let e = big
        .enable_verity(FS_VERITY_HASH_ALG_SHA256, 8192)
        .unwrap_err();
    assert_eq!(e.errno(), libc::EINVAL);
}

/// Measuring a file that is not verity fails `ENODATA`.
#[test]
fn measure_non_verity_is_enodata() {
    let dir = scratch();
    let fd = open_file(&dir, "plain", CONTENT);
    let e = fd.measure_verity().unwrap_err();
    assert_eq!(e.errno(), libc::ENODATA);
}
