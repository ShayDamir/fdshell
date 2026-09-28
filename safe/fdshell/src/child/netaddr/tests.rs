#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;

use crate::child::netaddr::{self, Address, NetArgs, SocketType};

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    f(&refs)
}

fn parsed(args: &[&str]) -> NetArgs {
    with_refs(args, |refs| netaddr::parse_net_args(refs).unwrap())
}

fn fails(args: &[&str], is_expected: impl Fn(&BuiltinError) -> bool) {
    with_refs(args, |refs| {
        let e = netaddr::parse_net_args(refs).unwrap_err();
        assert!(is_expected(e.current_context()), "unexpected error: {e:?}");
    });
}

#[test]
fn parse_defaults_to_stream_uds_path() {
    let cfg = parsed(&["sock"]);
    assert_eq!(cfg.ty, SocketType::Stream);
    match cfg.addr {
        Address::UdsPath(p) => assert_eq!(p.as_bytes().unwrap(), b"sock"),
        other => panic!("expected UdsPath, got {other:?}"),
    }
    assert_eq!(cfg.backlog, None);
}

#[test]
fn parse_abstract_address_strips_at() {
    match parsed(&["@my-server"]).addr {
        Address::UdsAbstract(n) => assert_eq!(n.as_bytes().unwrap(), b"my-server"),
        other => panic!("expected UdsAbstract, got {other:?}"),
    }
}

#[test]
fn parse_type_dgram() {
    assert_eq!(parsed(&["--type", "dgram", "x"]).ty, SocketType::Dgram);
}

#[test]
fn parse_type_stream() {
    assert_eq!(parsed(&["--type", "stream", "x"]).ty, SocketType::Stream);
}

#[test]
fn parse_inet_pair() {
    match parsed(&["--bind", "127.0.0.1", "--port", "1234"]).addr {
        Address::Inet { addr, port } => {
            assert_eq!(addr.as_bytes().unwrap(), b"127.0.0.1");
            assert_eq!(port, 1234);
        }
        other => panic!("expected Inet, got {other:?}"),
    }
}

#[test]
fn parse_backlog_value() {
    assert_eq!(parsed(&["--backlog", "5", "x"]).backlog, Some(5));
}

#[test]
fn parse_uds_max_length_is_ok() {
    let name = "a".repeat(netaddr::SUN_PATH_MAX);
    let cfg = parsed(&[&name]);
    match cfg.addr {
        Address::UdsPath(p) => assert_eq!(p.as_bytes().unwrap().len(), netaddr::SUN_PATH_MAX),
        other => panic!("expected UdsPath, got {other:?}"),
    }
}

fn is_what(e: &BuiltinError, what: &'static str) -> bool {
    matches!(e, BuiltinError::InvalidArgument(s) if *s == what)
}

#[test]
fn parse_missing_address() {
    fails(&[], |e| {
        matches!(e, BuiltinError::MissingArgument("address"))
    });
}

#[test]
fn parse_bad_type() {
    fails(&["--type", "bogus", "x"], |e| is_what(e, "type"));
}

#[test]
fn parse_bind_without_port() {
    fails(&["--bind", "127.0.0.1"], |e| is_what(e, "port"));
}

#[test]
fn parse_port_without_bind() {
    fails(&["--port", "80"], |e| is_what(e, "bind"));
}

#[test]
fn parse_positional_conflicts_with_bind() {
    fails(&["sock", "--bind", "127.0.0.1", "--port", "80"], |e| {
        is_what(e, "address")
    });
}

#[test]
fn parse_positional_conflicts_with_port_alone() {
    // A positional address with `--port` but no `--bind` is still a mix.
    fails(&["sock", "--port", "80"], |e| is_what(e, "address"));
}

#[test]
fn parse_path_too_long() {
    let name = "a".repeat(netaddr::SUN_PATH_MAX + 1);
    fails(&[&name], |e| is_what(e, "address"));
}

#[test]
fn parse_empty_abstract_name() {
    fails(&["@"], |e| is_what(e, "address"));
}

#[test]
fn parse_port_out_of_range() {
    fails(&["--bind", "127.0.0.1", "--port", "65536"], |e| {
        is_what(e, "port")
    });
}

#[test]
fn parse_backlog_out_of_range() {
    for v in ["-1", "2147483648"] {
        fails(&["--backlog", v, "x"], |e| is_what(e, "backlog"));
    }
}

#[test]
fn parse_unknown_flag() {
    fails(&["--nuclear", "x"], |e| is_what(e, "flag"));
}

#[test]
fn parse_help() {
    fails(&["--help"], |e| matches!(e, BuiltinError::Help));
}

#[test]
fn parse_duplicate_flags() {
    for args in [
        &["--type", "stream", "--type", "dgram", "x"][..],
        &["--backlog", "1", "--backlog", "2", "x"][..],
        &["--bind", "1.2.3.4", "--bind", "1.2.3.4", "--port", "1"][..],
    ] {
        fails(args, |e| matches!(e, BuiltinError::InvalidArgument(_)));
    }
}
