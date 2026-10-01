#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use sys::shellfd::TAG_MAX;

/// Drive `memfd_exec` with the given args and hand the received handle to `f`:
/// socketpair → `set_capture_active` → `export` + `try_clone` (the established
/// shape for a `*_exec` that exports a handle, see `tests/eventfd.rs`) →
/// `memfd_exec` → `recv_fd`. No scratch dir: a memfd touches no files on disk.
fn with_exec<F: FnOnce(sys::LocalFd)>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    let cfg = builtins::memfd::parse::memfd_parse(&refs).unwrap();

    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    builtins::memfd::memfd_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = receiver.recv_fd(&mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"memfd");

    f(fd);
}

/// No `--seal` leaves the memfd unsealed: an empty regular file that accepts
/// writes. A memfd must not be sealed unless asked.
#[test]
fn memfd_exec_sends_an_unsealed_empty_memfd() {
    with_exec(&[], |fd| {
        let st = fd.fstat().unwrap();
        assert_eq!(st.mode & sys::stat::S_IFMT, sys::stat::S_IFREG);
        assert_eq!(st.size, 0);
        fd.write(b"x").unwrap();
    });
}

/// `--size` grows the file and `--seal` applies before the handle is exported:
/// the received memfd is 4096 bytes and write / grow / shrink each fail with
/// EPERM. Under the `seals != 0`→`==` inversion the mask is never applied, so
/// the EPERM asserts fail.
#[test]
fn memfd_exec_seals_before_exporting_the_handle() {
    with_exec(
        &[
            "--name", "secret", "--size", "4096", "--seal", "SHRINK", "--seal", "GROW", "--seal",
            "WRITE",
        ],
        |fd| {
            let st = fd.fstat().unwrap();
            assert_eq!(st.mode & sys::stat::S_IFMT, sys::stat::S_IFREG);
            assert_eq!(st.size, 4096);
            let e = fd.write(b"x").unwrap_err();
            assert_eq!(e.errno(), sys::errno::EPERM);
            let e = fd.ftruncate(8192).unwrap_err();
            assert_eq!(e.errno(), sys::errno::EPERM);
            let e = fd.ftruncate(2048).unwrap_err();
            assert_eq!(e.errno(), sys::errno::EPERM);
        },
    );
}

/// `--size` above `i64::MAX` is the only `memfd_exec` error path: the size
/// cannot be an `ftruncate` length, so it is rejected before the handle is
/// exported.
#[test]
fn memfd_exec_rejects_a_size_above_i64_max() {
    let owned: Vec<CString> = ["--size", "18446744073709551615"]
        .iter()
        .map(|s| CString::new(*s).unwrap())
        .collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    let cfg = builtins::memfd::parse::memfd_parse(&refs).unwrap();

    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let e = builtins::memfd::memfd_exec(&cfg, &shell_sock).unwrap_err();
    assert!(
        matches!(e.current_context(), BuiltinError::InvalidArgument(s) if *s == "size"),
        "expected InvalidArgument(\"size\"), got {}",
        e.current_context()
    );
}
