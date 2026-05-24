#![cfg_attr(test, allow(clippy::unwrap_used))]

use core::ffi::CStr;
use std::ffi::CString;
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

#[test]
fn help_long() { assert_err(&["--help"], 0); }

#[test]
fn help_short() { assert_err(&["-h"], 0); }

#[test]
fn empty_args() { assert_err(&[], 0); }

#[test]
fn unexpected_arg() { assert_err(&["x"], 22); }

#[test]
fn test_pipe_exec() {
    let mut pair = [0; 2];
    sys::net::socketpair(&mut pair).unwrap();
    if pair[0] != 3 {
        sys::fd::dup2(pair[0], 3).unwrap();
        let _ = sys::fd::close(pair[0]);
    }
    let receiver = pair[1];

    builtins::pipe::pipe_exec().unwrap();

    let mut buf_a = [0u8; TAG_MAX];
    let mut buf_b = [0u8; TAG_MAX];
    let (fd_a, tag_a) = sys::shellfd::recv_fd(receiver, &mut buf_a).unwrap();
    let (fd_b, tag_b) = sys::shellfd::recv_fd(receiver, &mut buf_b).unwrap();

    let (rd, wr) = match (tag_a.to_bytes(), tag_b.to_bytes()) {
        (b"rd", b"wr") => (fd_a, fd_b),
        (b"wr", b"rd") => (fd_b, fd_a),
        _ => panic!("unexpected tags"),
    };

    sys::rw::write(wr, b"hello").unwrap();
    let mut buf = [0u8; 5];
    let n = sys::rw::read(rd, &mut buf).unwrap() as usize;
    assert_eq!(n, 5);
    assert_eq!(buf, *b"hello");

    sys::fd::close(rd).unwrap();
    sys::fd::close(wr).unwrap();
    sys::fd::close(receiver).unwrap();
    if pair[0] != 3 {
        sys::fd::close(3).unwrap();
    }
}
