#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::ffi::CString;
use std::ops::Deref;
use std::path::PathBuf;
use sys::fcntl::O_RDONLY;
use sys::fsverity::{
    FS_VERITY_HASH_ALG_SHA256, FS_VERITY_HASH_ALG_SHA512, FS_VERITY_METADATA_TYPE_DESCRIPTOR,
    FS_VERITY_METADATA_TYPE_MERKLE_TREE, FS_VERITY_METADATA_TYPE_SIGNATURE,
};
use sys::openat2::open;

/// The content the e2e builtin tests pin; its file digest is a pure function of
/// (content, algo, block size, salt), stable across kernels.
const CONTENT: &[u8] = b"fdshell-verity-e2e\n";

const SHA256_DIGEST: &str = "512eb3c1830461d829fbe3f6b07b2c40bc49114586b3389dad43f8b195abf57b";
const SHA512_DIGEST: &str = "e296f654d46935c2afbd9b7976d08e3299fce789d7572aea31b1372d5d3f6029cf89cf602efbf1c677d952a41463a1549a1c342abd07636faca4651d15f3e976";

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it
/// (LESSONS: "pid + counter is collision-free only if"). Removal is
/// `.ok()`-tolerant: teardown never panics, and it runs on a failing test's
/// unwind, which is exactly the test that used to leak.
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

/// A scratch dir; each test gets its own (pid + counter, per
/// LESSONS: "pid + counter is collision-free only if"), removed when the test's
/// guard drops. The dir is *created exclusively* (`create_dir`, never
/// `create_dir_all`): a pid+counter name allocated by `create_dir_all` can be a
/// dir a killed run leaked and a recycled pid landed on, so it is inherited,
/// not allocated. Inheriting is what this family's EPERM flake is: a leaked
/// verity scratch dir holds a verity-enabled file, and the kernel refuses a
/// writable open of a verity inode with `EPERM`, permanently.
fn scratch() -> Scratch {
    loop {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fdshell-verity-{}-{}", std::process::id(), c));
        match std::fs::create_dir(&dir) {
            // The name is taken (a killed run's leak, a recycled pid, a
            // pid-namespace twin): take the next counter, never inherit.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Ok(()) => return Scratch(dir),
            Err(e) => panic!("cannot create a private scratch dir {dir:?}: {e}"),
        }
    }
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

/// Read `len` bytes of metadata item `item` at `offset`; returns the bytes
/// actually read (a `pread()`-like short read).
fn read_meta(
    fd: &sys::LocalFd,
    item: u64,
    offset: u64,
    len: usize,
) -> Result<Vec<u8>, sys::SyscallError> {
    let mut buf = vec![0u8; len];
    let n = fd.read_verity_metadata_into(item, offset, &mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

/// The >= 6.13 descriptor is a fixed 256 bytes: a header
/// (version, hash_algorithm, log_blocksize, salt_size, reserved, data_size),
/// then the 64-byte root-hash area, salt, and reserved zeros. The root hash is
/// pinned — a pure function of (content, algo, block size, salt).
#[test]
fn read_metadata_descriptor_sha256() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let d = read_meta(&fd, FS_VERITY_METADATA_TYPE_DESCRIPTOR, 0, 256).unwrap();
    assert_eq!(d.len(), 256);
    assert_eq!(d[0], 1, "version");
    assert_eq!(d[1], 1, "sha256");
    assert_eq!(d[2], 12, "log2(4096)");
    assert_eq!(d[3], 0, "no salt");
    assert_eq!(&d[4..8], &[0; 4], "reserved");
    assert_eq!(
        u64::from_le_bytes(d[8..16].try_into().unwrap()),
        19,
        "data_size"
    );
    assert_eq!(
        to_hex(&d[16..48]),
        "bfa4acf46954515a2601f0ba6af1598e75724cde045e7263fb2b9b5a007800f3"
    );
    assert!(
        d[48..].iter().all(|&b| b == 0),
        "root-hash tail + salt + reserved"
    );
}

/// A single data block has no Merkle-tree blocks of its own: the tree item is
/// empty (the root hash lives in the descriptor).
#[test]
fn read_metadata_tree_single_block_is_empty() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let t = read_meta(&fd, FS_VERITY_METADATA_TYPE_MERKLE_TREE, 0, 4096).unwrap();
    assert!(t.is_empty());
}

/// The signature item of an unsigned verity file is absent: `ENODATA`.
#[test]
fn read_metadata_signature_unsigned_is_enodata() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let e = read_meta(&fd, FS_VERITY_METADATA_TYPE_SIGNATURE, 0, 4096).unwrap_err();
    assert_eq!(e.errno(), libc::ENODATA);
}

/// Reading past the end of an item returns 0 bytes (`pread()`-like).
#[test]
fn read_metadata_past_eof_is_zero() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let d = read_meta(&fd, FS_VERITY_METADATA_TYPE_DESCRIPTOR, 1 << 20, 256).unwrap();
    assert!(d.is_empty());
}

/// A zero-length read returns 0 bytes.
#[test]
fn read_metadata_zero_length_is_zero() {
    let dir = scratch();
    let fd = open_file(&dir, "f", CONTENT);
    fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096).unwrap();
    let d = read_meta(&fd, FS_VERITY_METADATA_TYPE_DESCRIPTOR, 0, 0).unwrap();
    assert!(d.is_empty());
}

/// Reading metadata of a file that is not verity fails `ENODATA`.
#[test]
fn read_metadata_non_verity_is_enodata() {
    let dir = scratch();
    let fd = open_file(&dir, "plain", CONTENT);
    let e = read_meta(&fd, FS_VERITY_METADATA_TYPE_DESCRIPTOR, 0, 256).unwrap_err();
    assert_eq!(e.errno(), libc::ENODATA);
}

/// Seed every scratch name this binary can hand out with a **verity** file,
/// then prove `scratch()` lands on a clean private dir. This is the reported
/// EPERM flake, planted deterministically (no load loop): the flake is the
/// write of `f` inside an *inherited* dir, because a verity inode refuses
/// `O_WRONLY|O_CREAT|O_TRUNC` with `EPERM` — forever.
#[test]
fn scratch_dir_is_exclusive() {
    const WINDOW: u64 = 32; // > the 14 scratch() call sites in this binary
    let base = std::env::temp_dir();
    let pid = std::process::id();
    let start = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
    let mut seeds: Vec<Scratch> = Vec::new();
    for c in start..start + WINDOW {
        let dir = base.join(format!("fdshell-verity-{pid}-{c}"));
        if std::fs::create_dir(&dir).is_err() {
            continue; // a name some other holder owns: not ours to seed
        }
        let owned = Scratch(dir);
        let fd = open_file(&owned, "f", CONTENT);
        match fd.enable_verity(FS_VERITY_HASH_ALG_SHA256, 4096) {
            Ok(()) => seeds.push(owned),
            // No fs-verity on this host/fs: the landmine cannot be planted.
            Err(e) if matches!(e.errno(), libc::ENOTTY | libc::EOPNOTSUPP) => return,
            Err(e) => panic!("seed enable failed: {e}"),
        }
    }
    let dir = scratch();
    assert!(
        !seeds.iter().any(|s| **s == *dir),
        "scratch() inherited a seeded dir: {}",
        dir.display()
    );
    // Pre-fix the assertion above fires; this is the exact reported panic site
    // (`std::fs::write` -> EPERM) one step deeper.
    open_file(&dir, "f", CONTENT);
}
