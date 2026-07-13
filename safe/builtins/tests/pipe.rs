#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use sys::fcntl::{O_DIRECT, O_NONBLOCK};
use sys::shellfd::TAG_MAX;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| match builtins::pipe::parse::pipe_parse(a) {
        Err(e) => {
            let ctx = e.current_context();
            match (ctx, expected) {
                (BuiltinError::Help, BuiltinError::Help) => {}
                (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                _ => panic!("unexpected error: {ctx}"),
            }
        }
        _ => panic!("expected Err"),
    });
}

fn assert_invalid_arg(args: &[&str]) {
    assert_err(args, BuiltinError::InvalidArgument("x"));
}

fn assert_ok<F: Fn(&builtins::pipe::parse::PipeConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::pipe::parse::pipe_parse(a) {
        Ok(cfg) => f(&cfg),
        _ => panic!("expected Ok"),
    });
}

#[test]
fn help_long() {
    assert_err(&["--help"], BuiltinError::Help);
}

#[test]
fn help_short() {
    assert_err(&["-h"], BuiltinError::Help);
}

#[test]
fn empty_args() {
    assert_ok(&[], |cfg| assert_eq!(cfg.flags, 0));
}

#[test]
fn unexpected_arg() {
    assert_invalid_arg(&["x"])
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
    assert_invalid_arg(&["--flags"])
}

#[test]
fn flags_bad() {
    assert_invalid_arg(&["--flags", "O_CLOEXEC"])
}

#[test]
fn flags_bad_name() {
    assert_invalid_arg(&["--flags", "nope"])
}

#[test]
fn test_pipe_exec() {
    // Create a socket to use as the shell fd (fd 3)
    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export_to(3).unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    shell_a.try_close().unwrap();

    builtins::pipe::pipe_exec(0, &shell_sock).unwrap();

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

    rd.try_close().unwrap();
    wr.try_close().unwrap();
    receiver.try_close().unwrap();
}
