#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use sys::{ImportedFd, ImportedFdError, ShortCStr};

#[test]
fn verify_internal_fd_rejected() {
    // Open /dev/null with O_CLOEXEC — CLOEXEC is set, so verify() must reject it.
    // SAFETY: `/dev/null` is a valid path; O_RDONLY|O_CLOEXEC are valid flags.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd with CLOEXEC clear — wait, it has CLOEXEC set,
    // so from_raw invariant is violated. But we're testing that verify() catches this.
    let d = unsafe { ImportedFd::from_raw(fd) };
    let result = d.verify();
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::InternalFd)
    ));
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn from_shortcstr_stdin() {
    let short = ShortCStr::from_vec(b"0".to_vec()).expect("test");
    let fd = ImportedFd::from_shortcstr(&short).expect("test");
    assert_eq!(fd.as_raw(), 0);
}

#[test]
fn from_shortcstr_invalid_number() {
    let short = ShortCStr::from_vec(b"notanumber".to_vec()).expect("test");
    let result = ImportedFd::from_shortcstr(&short);
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::NotANumber)
    ));
}

#[test]
fn from_shortcstr_negative() {
    let short = ShortCStr::from_vec(b"-1".to_vec()).expect("test");
    let result = ImportedFd::from_shortcstr(&short);
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::Negative)
    ));
}

#[test]
fn from_shortcstr_internal_fd_rejected() {
    // Open /dev/null with O_CLOEXEC — CLOEXEC is set, so verify() must reject it.
    // SAFETY: `/dev/null` is a valid path; O_RDONLY|O_CLOEXEC are valid flags.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(fd >= 0);
    let num = format!("{}", fd);
    let short = ShortCStr::from_vec(num.into_bytes()).expect("test");
    let result = ImportedFd::from_shortcstr(&short);
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::InternalFd)
    ));
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn write_str_to_dev_null() {
    // Open /dev/null without O_CLOEXEC — ImportedFd requires CLOEXEC clear.
    // SAFETY: `/dev/null` is a valid path; O_WRONLY is a valid flag.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd with CLOEXEC clear (O_CLOEXEC not passed).
    let imported = unsafe { ImportedFd::from_raw(fd) };

    let s = ShortCStr::from_vec(b"hello\n".to_vec()).expect("test");
    let result = imported.write_str(&s);
    assert!(result.is_ok());

    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn write_str_empty() {
    // Open /dev/null without O_CLOEXEC — ImportedFd requires CLOEXEC clear.
    // SAFETY: `/dev/null` is a valid path; O_WRONLY is a valid flag.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd with CLOEXEC clear (O_CLOEXEC not passed).
    let imported = unsafe { ImportedFd::from_raw(fd) };

    let s = ShortCStr::from_vec(vec![]).expect("test");
    let result = imported.write_str(&s);
    assert!(result.is_ok());

    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn display_formats_raw() {
    // Open /dev/null without O_CLOEXEC — ImportedFd requires CLOEXEC clear.
    // SAFETY: `/dev/null` is a valid path; O_RDONLY is a valid flag.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd with CLOEXEC clear (O_CLOEXEC not passed).
    let imported = unsafe { ImportedFd::from_raw(fd) };
    let formatted = format!("{}", imported);
    assert_eq!(formatted, fd.to_string());
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn read_from_pipe_returns_data() {
    let mut fds: [i32; 2] = [0; 2];
    // SAFETY: pipe() with valid array.
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(fds[0] >= 0 && fds[1] >= 0);

    let payload = b"hello world";
    // SAFETY: fds[1] is a valid fd, buf is valid slice.
    let written = unsafe {
        libc::write(
            fds[1],
            payload.as_ptr().cast(),
            payload.len() as libc::size_t,
        )
    };
    assert!(written > 0);

    // SAFETY: fds[0] is a valid fd with CLOEXEC clear (pipe default).
    let imported = unsafe { ImportedFd::from_raw(fds[0]) };

    let mut buf = [0u8; 128];
    let n = imported.read(&mut buf).expect("read");
    assert_eq!(n, payload.len());
    assert_eq!(&buf[..n], payload);

    // SAFETY: fds[1] is a valid open fd from the test above.
    unsafe { libc::close(fds[1]) };
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fds[0]) };
}

#[test]
fn write_to_pipe_returns_bytes() {
    let mut fds: [i32; 2] = [0; 2];
    // SAFETY: pipe() with valid array.
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(fds[0] >= 0 && fds[1] >= 0);

    // Write end becomes ImportedFd.
    // SAFETY: fds[1] is a valid fd with CLOEXEC clear.
    let imported = unsafe { ImportedFd::from_raw(fds[1]) };

    let payload = b"test data";
    let n = imported.write(payload).expect("write");
    assert_eq!(n, payload.len());

    // Read back to verify.
    let mut buf = [0u8; 128];
    // SAFETY: fds[0] is a valid fd.
    let read_n = unsafe { libc::read(fds[0], buf.as_mut_ptr().cast(), buf.len() as libc::size_t) };
    assert_eq!(read_n as usize, payload.len());

    // SAFETY: fds[0] is a valid open fd from the test above.
    unsafe { libc::close(fds[0]) };
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fds[1]) };
}

#[test]
fn write_all_completes_on_full_write() {
    let mut fds: [i32; 2] = [0; 2];
    // SAFETY: pipe() with valid array.
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(fds[0] >= 0 && fds[1] >= 0);

    // Write end becomes ImportedFd.
    // SAFETY: fds[1] is a valid fd with CLOEXEC clear.
    let imported = unsafe { ImportedFd::from_raw(fds[1]) };

    let payload = b"write_all test payload";
    imported.write_all(payload).expect("write_all");

    let mut buf = [0u8; 128];
    // SAFETY: fds[0] is a valid fd.
    let read_n = unsafe { libc::read(fds[0], buf.as_mut_ptr().cast(), buf.len() as libc::size_t) };
    assert_eq!(read_n as usize, payload.len());

    // SAFETY: fds[0] is a valid open fd from the test above.
    unsafe { libc::close(fds[0]) };
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fds[1]) };
}

#[test]
fn read_all_fills_buffer() {
    let mut fds: [i32; 2] = [0; 2];
    // SAFETY: pipe() with valid array.
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(fds[0] >= 0 && fds[1] >= 0);

    let payload = b"read_all test data here";
    // SAFETY: fds[1] is a valid fd.
    let written = unsafe {
        libc::write(
            fds[1],
            payload.as_ptr().cast(),
            payload.len() as libc::size_t,
        )
    };
    assert!(written > 0);

    // Close write end so read_all gets EOF.
    // SAFETY: fds[1] is a valid open fd from the test above.
    unsafe { libc::close(fds[1]) };

    // SAFETY: fds[0] is a valid fd with CLOEXEC clear.
    let imported = unsafe { ImportedFd::from_raw(fds[0]) };

    let mut buf = [0u8; 128];
    let n = imported.read_all(&mut buf).expect("read_all");
    assert_eq!(n, payload.len());
    assert_eq!(&buf[..n], payload);

    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fds[0]) };
}

#[test]
fn write_all_empty() {
    let mut fds: [i32; 2] = [0; 2];
    // SAFETY: pipe() with valid array.
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(fds[0] >= 0 && fds[1] >= 0);

    // Write end becomes ImportedFd.
    // SAFETY: fds[1] is a valid fd with CLOEXEC clear.
    let imported = unsafe { ImportedFd::from_raw(fds[1]) };

    let payload: &[u8] = &[];
    imported.write_all(payload).expect("write_all empty");

    // Close write end so reader gets EOF.
    // SAFETY: fds[1] is a valid open fd from the test above.
    unsafe { libc::close(fds[1]) };

    let mut buf = [0u8; 128];
    // SAFETY: fds[0] is a valid fd.
    let read_n = unsafe { libc::read(fds[0], buf.as_mut_ptr().cast(), buf.len() as libc::size_t) };
    assert_eq!(read_n, 0);

    // SAFETY: fds[0] is a valid open fd from the test above.
    unsafe { libc::close(fds[0]) };
}

#[test]
fn read_all_eof_returns_partial() {
    let mut fds: [i32; 2] = [0; 2];
    // SAFETY: pipe() with valid array.
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(fds[0] >= 0 && fds[1] >= 0);

    let payload = b"partial";
    // SAFETY: fds[1] is a valid fd.
    let written = unsafe {
        libc::write(
            fds[1],
            payload.as_ptr().cast(),
            payload.len() as libc::size_t,
        )
    };
    assert!(written > 0);

    // Close write end immediately so read_all hits EOF right away.
    // SAFETY: fds[1] is a valid open fd from the test above.
    unsafe { libc::close(fds[1]) };

    // SAFETY: fds[0] is a valid fd with CLOEXEC clear.
    let imported = unsafe { ImportedFd::from_raw(fds[0]) };

    let mut buf = [0u8; 128];
    let n = imported.read_all(&mut buf).expect("read_all eof");
    assert_eq!(n, payload.len());
    assert_eq!(&buf[..n], payload);

    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fds[0]) };
}

#[test]
fn write_str_to_dev_full_fails() {
    // SAFETY: `/dev/full` is a valid path; O_WRONLY is a valid flag.
    let fd = unsafe { libc::open(c"/dev/full".as_ptr(), libc::O_WRONLY) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd with CLOEXEC clear (O_CLOEXEC not passed).
    let imported = unsafe { ImportedFd::from_raw(fd) };

    let s = ShortCStr::from_vec(b"data".to_vec()).expect("test");
    let result = imported.write_str(&s);
    assert!(result.is_err());

    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn write_all_errors_when_send_buffer_cannot_accept_all() {
    // A non-blocking socket with a small send buffer makes the first write a
    // short write and the next one fail with EAGAIN, so write_all must return
    // an error. A `n == 0`→`n != 0` mutation would break after the first
    // (partial) write and return Ok, which this catches.
    let (a, _b) = sys::net::socketpair().unwrap();
    // dup so the fd has CLOEXEC clear (satisfying the ImportedFd invariant).
    // SAFETY: `a.as_raw()` is a valid open socket; dup returns a new fd or -1.
    let raw = unsafe { libc::dup(a.as_raw()) };
    assert!(raw >= 0);
    let sndbuf: libc::c_int = 64;
    // SAFETY: `raw` is a valid socket; SO_SNDBUF is a valid SOL_SOCKET option.
    unsafe {
        libc::setsockopt(
            raw,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            (&raw const sndbuf).cast(),
            core::mem::size_of_val(&sndbuf) as libc::socklen_t,
        )
    };
    // SAFETY: `raw` is a valid open fd with CLOEXEC clear (dup clears it).
    let imported = unsafe { ImportedFd::from_raw(raw) };
    let payload = vec![b'x'; 1024 * 1024];
    assert!(imported.write_all(&payload).is_err());
    // SAFETY: `raw` is the valid open fd created via dup above.
    unsafe { libc::close(raw) };
}

#[test]
fn read_all_fills_buffer_across_reads() {
    // Data arrives in two 256-byte chunks with a delay, so read_all must loop
    // past a partial fill to reach EOF. A `offset >= len`→`offset < len`
    // mutation would break after the first chunk and return 256, not 512.
    let mut fds: [i32; 2] = [0; 2];
    // SAFETY: pipe() with a valid 2-element array.
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert!(fds[0] >= 0 && fds[1] >= 0);
    let wr_fd = fds[1];
    // SAFETY: `fds[0]` is a valid open fd with CLOEXEC clear (raw pipe default).
    let imported = unsafe { ImportedFd::from_raw(fds[0]) };

    std::thread::scope(move |t| {
        t.spawn(move || {
            let first = [0xABu8; 256];
            let second = [0xCDu8; 256];
            // SAFETY: `wr_fd` is a valid fd; the buffers are valid slices.
            unsafe { libc::write(wr_fd, first.as_ptr().cast(), first.len() as libc::size_t) };
            std::thread::sleep(std::time::Duration::from_millis(20));
            // SAFETY: `wr_fd` is a valid fd; the buffer is a valid slice.
            unsafe { libc::write(wr_fd, second.as_ptr().cast(), second.len() as libc::size_t) };
        });
        t.spawn(move || {
            let mut buf = [0u8; 512];
            let n = imported.read_all(&mut buf).expect("read_all");
            assert_eq!(n, 512);
            assert_eq!(&buf[..256], &[0xABu8; 256]);
            assert_eq!(&buf[256..], &[0xCDu8; 256]);
        });
    });

    // SAFETY: both `wr_fd` and `fds[0]` are valid open fds from the pipe above.
    unsafe {
        libc::close(wr_fd);
        libc::close(fds[0]);
    }
}

#[test]
fn from_number_negative() {
    let result = ImportedFd::from_number(-1);
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::Negative)
    ));
}

#[test]
fn is_tty_pipe_is_false() {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid two-slot array for `pipe`.
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    // SAFETY: `fds[0]` is a valid open fd with CLOEXEC clear (pipe default).
    let rd = unsafe { ImportedFd::from_raw(fds[0]) };
    // SAFETY: `fds[1]` is a valid open fd with CLOEXEC clear (pipe default).
    let wr = unsafe { ImportedFd::from_raw(fds[1]) };
    assert!(!rd.is_tty().unwrap());
    assert!(!wr.is_tty().unwrap());
    // SAFETY: `fds[0]` and `fds[1]` are valid open fds created above.
    unsafe { libc::close(fds[0]) };
    // SAFETY: `fds[1]` is a valid open fd created above.
    unsafe { libc::close(fds[1]) };
}

#[test]
fn is_tty_dev_null_is_false() {
    // SAFETY: O_RDWR on /dev/null yields a valid open fd with CLOEXEC clear.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDWR) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd with CLOEXEC clear (opened above).
    let imported = unsafe { ImportedFd::from_raw(fd) };
    assert!(!imported.is_tty().unwrap());
    // SAFETY: `fd` is the valid open fd created above.
    unsafe { libc::close(fd) };
}

#[test]
fn is_tty_pty_slave_is_true() {
    let mut master = 0i32;
    let mut slave = 0i32;
    // SAFETY: null name/termios/winsize are the documented defaults for `openpty`.
    let ret = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            core::ptr::null_mut(),
            core::ptr::null(),
            core::ptr::null(),
        )
    };
    assert_eq!(ret, 0);
    // SAFETY: `slave` is a valid open fd with CLOEXEC clear (openpty default).
    let imported = unsafe { ImportedFd::from_raw(slave) };
    assert!(imported.is_tty().unwrap());
    // SAFETY: `master` and `slave` are valid open fds created above.
    unsafe { libc::close(master) };
    // SAFETY: `slave` is a valid open fd created above.
    unsafe { libc::close(slave) };
}
