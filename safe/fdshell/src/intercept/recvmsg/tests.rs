use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

use super::parse::{Parsed, parse};
use crate::error::cmd::CmdError;

fn run(words: &[&str]) -> Result<Parsed, Report<CmdError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    parse(&args)
}

macro_rules! assert_err {
    ($r:expr, $variant:pat) => {
        let Err(e) = $r else {
            panic!("expected {}, got Ok", stringify!($variant));
        };
        assert!(matches!(e.current_context(), $variant));
    };
}

#[test]
fn minimal_form() {
    let p = run(&["%s", "OUT"]).unwrap();
    assert_eq!(p.sock.as_bytes().unwrap(), b"%s");
    assert_eq!(p.var.as_bytes().unwrap(), b"OUT");
    assert!(p.cred.is_none());
    assert!(p.fd_slots.is_empty());
}

#[test]
fn raw_socket_and_slots() {
    let p = run(&["3", "OUT", "%f1", "%f2"]).unwrap();
    assert_eq!(p.sock.as_bytes().unwrap(), b"3");
    let slots: Vec<&[u8]> = p.fd_slots.iter().map(|v| v.as_bytes().unwrap()).collect();
    assert_eq!(slots, vec![b"%f1", b"%f2"]);
}

#[test]
fn cred_flag() {
    let p = run(&["--cred", "CRED", "%s", "OUT"]).unwrap();
    assert_eq!(p.cred.as_ref().unwrap().as_bytes().unwrap(), b"CRED");
    assert_eq!(p.sock.as_bytes().unwrap(), b"%s");
    assert_eq!(p.var.as_bytes().unwrap(), b"OUT");
}

#[test]
fn cred_with_slots_and_var() {
    // `--cred` consumed, then var must not start with `%`.
    let p = run(&["--cred", "C", "0", "OUT", "%f"]).unwrap();
    assert_eq!(p.cred.as_ref().unwrap().as_bytes().unwrap(), b"C");
    assert_eq!(p.sock.as_bytes().unwrap(), b"0");
    assert_eq!(p.var.as_bytes().unwrap(), b"OUT");
    assert_eq!(p.fd_slots.len(), 1);
}

#[test]
fn missing_sock() {
    assert_err!(run(&[]), CmdError::RecvmsgBadUsage);
}

#[test]
fn missing_var() {
    assert_err!(run(&["%s"]), CmdError::RecvmsgBadUsage);
}

#[test]
fn var_with_percent_is_error() {
    assert_err!(run(&["%s", "%out"]), CmdError::RecvmsgBadUsage);
}

#[test]
fn cred_var_with_percent_is_error() {
    assert_err!(
        run(&["--cred", "%c", "%s", "OUT"]),
        CmdError::RecvmsgBadUsage
    );
}

#[test]
fn cred_missing_value_is_error() {
    assert_err!(run(&["--cred"]), CmdError::RecvmsgBadUsage);
}

#[test]
fn slot_without_percent_is_error() {
    assert_err!(run(&["%s", "OUT", "plain"]), CmdError::RecvmsgBadUsage);
}

#[test]
fn map_recv_error_maps_excess_fds() {
    let report = super::io::map_recv_error(Report::new(sys::SyscallError::E2BIG("recvmsg")));
    assert!(matches!(
        report.current_context(),
        CmdError::RecvmsgTooManyFds
    ));
    // The syscall error is preserved in the chain.
    assert!(report.contains::<Report<sys::SyscallError>>());
}
