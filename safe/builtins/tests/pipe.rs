#![cfg_attr(test, allow(clippy::unwrap_used))]

use core::ffi::CStr;
use std::ffi::CString;
use sys::errno::{EINVAL, HELP};
use sys::fcntl::{O_DIRECT, O_NONBLOCK};
use sys::shellfd::TAG_MAX;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], code: i32) {
    with_args(args, |a| match builtins::pipe::parse::pipe_parse(a) {
        Err(e) => assert_eq!(e, code),
        _ => panic!("expected Err({code})"),
    });
}

fn assert_ok<F: Fn(&builtins::pipe::parse::PipeConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::pipe::parse::pipe_parse(a) {
        Ok(cfg) => f(&cfg),
        _ => panic!("expected Ok"),
    });
}

#[test]
fn help_long() {
    assert_err(&["--help"], HELP);
}

#[test]
fn help_short() {
    assert_err(&["-h"], HELP);
}

#[test]
fn empty_args() {
    assert_ok(&[], |cfg| assert_eq!(cfg.flags, 0));
}

#[test]
fn unexpected_arg() {
    assert_err(&["x"], EINVAL);
}

#[test]
fn flags_nonblock() {
    assert_ok(&["--flags", "O_NONBLOCK"], |cfg| {
        assert_eq!(cfg.flags, O_NONBLOCK);
    });
}

#[test]
fn flags_direct() {
    assert_ok(&["--flags", "O_DIRECT"], |cfg| {
        assert_eq!(cfg.flags, O_DIRECT);
    });
}

#[test]
fn flags_repeated() {
    assert_ok(&["--flags", "O_NONBLOCK", "--flags", "O_DIRECT"], |cfg| {
        assert_eq!(cfg.flags, O_NONBLOCK | O_DIRECT);
    });
}

#[test]
fn flags_hex() {
    assert_ok(&["--flags", "0x4000"], |cfg| {
        assert_eq!(cfg.flags, O_DIRECT);
    });
}

#[test]
fn flags_empty_value() {
    assert_err(&["--flags"], EINVAL);
}

#[test]
fn flags_bad() {
    assert_err(&["--flags", "O_CLOEXEC"], EINVAL);
}

#[test]
fn flags_bad_name() {
    assert_err(&["--flags", "nope"], EINVAL);
}

#[test]
fn test_pipe_exec() {
    let _shellfd = sys::shellfd::reserve_shellfd().unwrap();
    let (a, b) = sys::net::socketpair().unwrap();
    a.verify().unwrap();
    b.verify().unwrap();
    a.dup_to(sys::shellfd::SHELLFD).unwrap();
    a.close().unwrap();
    let receiver = b;
    sys::shellfd::set_capture_active(true);

    builtins::pipe::pipe_exec(0).unwrap();

    let mut buf_a = [0u8; TAG_MAX];
    let mut buf_b = [0u8; TAG_MAX];
    let pid = std::process::id() as i32;
    let (fd_a, tag_a) = sys::shellfd::recv_fd(&receiver, &mut buf_a, pid).unwrap();
    fd_a.verify().unwrap();
    let (fd_b, tag_b) = sys::shellfd::recv_fd(&receiver, &mut buf_b, pid).unwrap();
    fd_b.verify().unwrap();

    let (rd, wr) = match (tag_a.to_bytes(), tag_b.to_bytes()) {
        (b"rd", b"wr") => (fd_a, fd_b),
        (b"wr", b"rd") => (fd_b, fd_a),
        _ => panic!("unexpected tags"),
    };

    sys::rw::write(&wr, b"hello").unwrap();
    let mut buf = [0u8; 5];
    let n = sys::rw::read(&rd, &mut buf).unwrap() as usize;
    assert_eq!(n, 5);
    assert_eq!(buf, *b"hello");

    rd.close().unwrap();
    wr.close().unwrap();
    receiver.close().unwrap();
}
