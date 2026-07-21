#![allow(clippy::unwrap_used, clippy::expect_used)]

use sys::pipe::pipe2;
use sys::rw::write;
use sys::{LocalFd, LocalFdError};

#[test]
fn verify_rejects_no_cloexec() {
    // Open /dev/null without O_CLOEXEC — sys::openat2 always adds it, so raw
    // libc is the only way to exercise the NoCloexec verification path.
    // SAFETY: `/dev/null` is a valid path; O_RDONLY is a valid flag.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd — we intentionally opened without
    // O_CLOEXEC to verify that LocalFd::verify() catches this.
    let local = unsafe { LocalFd::from_raw(fd) };
    let result = local.verify();
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), LocalFdError::NoCloexec)
    ));
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn display_formats_raw() {
    let local = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let formatted = format!("{}", local);
    let raw = local.as_raw();
    assert_eq!(formatted, raw.to_string());
}

#[test]
fn read_returns_bytes() {
    let (rd, wr) = pipe2(0).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let data = b"hello world";
    write(&wr, data).unwrap();
    let mut buf = [0u8; 128];
    let n = rd.read(&mut buf).unwrap();
    assert_eq!(n, data.len());
    assert_eq!(&buf[..11], data);
}

#[test]
fn read_all_fills_buffer() {
    let (rd, wr) = pipe2(0).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let data = [0xABu8; 256];
    write(&wr, &data).unwrap();
    let mut buf = [0u8; 256];
    let n = rd.read_all(&mut buf).unwrap();
    assert_eq!(n, 256);
    assert_eq!(&buf, &data[..]);
}

#[test]
fn read_all_dev_null_returns_zero() {
    let local = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let mut buf = [0u8; 256];
    let n = local.read_all(&mut buf).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn read_all_partial() {
    let (rd, wr) = pipe2(0).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    std::thread::scope(move |t| {
        t.spawn(move || {
            let data = [0xABu8; 256];
            write(&wr, &data).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
            let data = [0xCDu8; 256];
            write(&wr, &data).unwrap();
        });
        t.spawn(move || {
            let mut buf = [0u8; 512];
            let n = rd.read_all(&mut buf).unwrap();
            assert_eq!(n, 512);
            assert_eq!(&buf[..256], &[0xABu8; 256]);
            assert_eq!(&buf[256..], &[0xCDu8; 256]);
        });
    });
}
