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
fn sock_only_is_empty_payload() {
    let p = run(&["%s"]).unwrap();
    assert_eq!(p.sock.as_bytes().unwrap(), b"%s");
    assert!(p.msg.is_none());
    assert!(p.msgfd.is_none());
    assert!(p.fd_vars.is_empty());
}

#[test]
fn raw_socket_number() {
    let p = run(&["3", "--msg", "hi"]).unwrap();
    assert_eq!(p.sock.as_bytes().unwrap(), b"3");
    assert_eq!(p.msg.as_deref(), Some(b"hi".as_slice()));
}

#[test]
fn msg_flag() {
    let p = run(&["%s", "--msg", "hello world"]).unwrap();
    assert_eq!(p.msg.as_deref(), Some(b"hello world".as_slice()));
}

#[test]
fn msgfd_flag() {
    let p = run(&["%s", "--msgfd", "%in", "4096"]).unwrap();
    assert_eq!(
        p.msgfd,
        Some((ShortCStr::from_vec(b"%in".to_vec()).unwrap(), 4096))
    );
}

#[test]
fn msgfd_zero_count() {
    let p = run(&["%s", "--msgfd", "%in", "0"]).unwrap();
    assert_eq!(p.msgfd.as_ref().unwrap().1, 0);
}

#[test]
fn fd_vars_in_order_with_duplicates() {
    let p = run(&["%s", "--fd", "%a", "--fd", "%b", "--fd", "%a"]).unwrap();
    let names: Vec<&[u8]> = p.fd_vars.iter().map(|v| v.as_bytes().unwrap()).collect();
    assert_eq!(names, vec![b"%a", b"%b", b"%a"]);
}

#[test]
fn interleaved_flags_keep_indexing() {
    // `--fd` at index 3 then `--msg` at 5: a mis-stepped index would drop or
    // eat the following flag.
    let p = run(&["%s", "--fd", "%a", "--msg", "x", "--fd", "%b"]).unwrap();
    assert_eq!(p.msg.as_deref(), Some(b"x".as_slice()));
    let names: Vec<&[u8]> = p.fd_vars.iter().map(|v| v.as_bytes().unwrap()).collect();
    assert_eq!(names, vec![b"%a", b"%b"]);
}

#[test]
fn msg_and_msgfd_conflict() {
    assert_err!(
        run(&["%s", "--msg", "a", "--msgfd", "%in", "1"]),
        CmdError::SendmsgPayloadConflict
    );
}

#[test]
fn msgfd_twice_conflicts() {
    assert_err!(
        run(&["%s", "--msgfd", "%a", "1", "--msgfd", "%b", "2"]),
        CmdError::SendmsgPayloadConflict
    );
}

#[test]
fn msgfd_then_msg_conflicts() {
    // Reverse order of `msg_and_msgfd_conflict`: the conflict is caught in the
    // `--msg` branch when a `--msgfd` was already given.
    assert_err!(
        run(&["%s", "--msgfd", "%in", "1", "--msg", "a"]),
        CmdError::SendmsgPayloadConflict
    );
}

#[test]
fn missing_sock() {
    assert_err!(run(&[]), CmdError::SendmsgBadSocket);
}

#[test]
fn unknown_flag() {
    assert_err!(run(&["%s", "--bogus"]), CmdError::SendmsgUsage);
}

#[test]
fn positional_after_sock_is_usage() {
    assert_err!(run(&["%s", "stray"]), CmdError::SendmsgUsage);
}

#[test]
fn msg_missing_value() {
    assert_err!(run(&["%s", "--msg"]), CmdError::SendmsgUsage);
}

#[test]
fn msgfd_missing_count() {
    assert_err!(run(&["%s", "--msgfd", "%in"]), CmdError::SendmsgUsage);
}

#[test]
fn msgfd_bad_count() {
    assert_err!(
        run(&["%s", "--msgfd", "%in", "12x"]),
        CmdError::SendmsgUsage
    );
}

#[test]
fn fd_missing_value() {
    assert_err!(run(&["%s", "--fd"]), CmdError::SendmsgUsage);
}

#[test]
fn fd_var_without_percent() {
    assert_err!(run(&["%s", "--fd", "plain"]), CmdError::SendmsgFds);
}

#[test]
fn passcred_flag() {
    let p = run(&["%s", "--passcred", "--msg", "x", "--fd", "%f"]).unwrap();
    assert!(p.passcred);
    assert_eq!(p.msg.as_deref(), Some(b"x".as_slice()));
    assert_eq!(p.fd_vars.len(), 1);
}

#[test]
fn passcred_alone_parses() {
    let p = run(&["0", "--passcred"]).unwrap();
    assert!(p.passcred);
    assert!(p.msg.is_none());
    assert!(p.fd_vars.is_empty());
}

#[test]
fn map_send_error_maps_empty_payload() {
    let report = super::map_send_error(Report::new(sys::SyscallError::EINVAL("sendmsg")));
    assert!(matches!(
        report.current_context(),
        CmdError::SendmsgEmptyPayload
    ));
    // The syscall error is preserved in the chain.
    assert!(report.contains::<Report<sys::SyscallError>>());
}

#[test]
fn map_send_error_maps_too_many_fds() {
    let report = super::map_send_error(Report::new(sys::SyscallError::E2BIG("sendmsg")));
    assert!(matches!(
        report.current_context(),
        CmdError::SendmsgTooManyFds
    ));
    assert!(report.contains::<Report<sys::SyscallError>>());
}
