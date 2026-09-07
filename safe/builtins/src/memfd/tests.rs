#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::CStr;

use error_stack::Report;

use crate::error::BuiltinError;

use super::parse::{MemfdConfig, memfd_parse};

/// Hold the [`CString`]s alive while parsing, since the returned config borrows
/// them; matches the pattern in `child/flock/tests.rs`.
fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&CStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    f(&refs)
}

fn is_invalid(e: Result<MemfdConfig<'_>, Report<BuiltinError>>, what: &'static str) -> bool {
    matches!(e.unwrap_err().current_context(), BuiltinError::InvalidArgument(s) if *s == what)
}

#[test]
fn creates_anonymous_memfd_by_default() {
    with_refs(&[], |refs| {
        let cfg = memfd_parse(refs).unwrap();
        assert!(cfg.name.is_none());
        assert!(cfg.size.is_none());
        assert_eq!(cfg.seals, 0);
    });
}

#[test]
fn help_on_help_flag() {
    with_refs(&["--help"], |refs| {
        assert!(matches!(
            memfd_parse(refs).unwrap_err().current_context(),
            BuiltinError::Help
        ));
    });
}

#[test]
fn name_accepts_valid() {
    with_refs(&["--name", "secret"], |refs| {
        assert_eq!(
            memfd_parse(refs).unwrap().name.unwrap().to_bytes(),
            b"secret"
        );
    });
    with_refs(&["--name=secret"], |refs| {
        assert_eq!(
            memfd_parse(refs).unwrap().name.unwrap().to_bytes(),
            b"secret"
        );
    });
    // Exactly 14 bytes is the largest allowed name; a `>=` bound rejects it.
    with_refs(&["--name", "0123456789abcd"], |refs| {
        assert_eq!(
            memfd_parse(refs).unwrap().name.unwrap().to_bytes(),
            b"0123456789abcd"
        );
    });
}

#[test]
fn name_rejects_slash() {
    with_refs(&["--name", "/bad"], |refs| {
        assert!(is_invalid(memfd_parse(refs), "name"));
    });
}

#[test]
fn name_rejects_too_long() {
    with_refs(&["--name", "exceeds_fourteen_bytes"], |refs| {
        assert!(is_invalid(memfd_parse(refs), "name"));
    });
}

/// Exactly 15 bytes is the smallest rejected name; the exact reject boundary
/// catches a `>` bound that is off by one.
#[test]
fn name_rejects_fifteen_bytes() {
    with_refs(&["--name", "0123456789abcde"], |refs| {
        assert!(is_invalid(memfd_parse(refs), "name"));
    });
}

#[test]
fn size_parses_decimal() {
    with_refs(&["--size", "1024"], |refs| {
        assert_eq!(memfd_parse(refs).unwrap().size, Some(1024));
    });
}

#[test]
fn size_rejects_non_numeric() {
    with_refs(&["--size", "abc"], |refs| {
        assert!(is_invalid(memfd_parse(refs), "size"));
    });
}

/// `--size 0` is valid: a zero-sized (empty) memfd.
#[test]
fn size_accepts_zero() {
    with_refs(&["--size", "0"], |refs| {
        assert_eq!(memfd_parse(refs).unwrap().size, Some(0));
    });
}

/// The `--key=value` equals form is accepted for the flag-with-value options.
#[test]
fn size_and_seal_accept_equals_form() {
    with_refs(&["--size=1024"], |refs| {
        assert_eq!(memfd_parse(refs).unwrap().size, Some(1024));
    });
    with_refs(&["--seal=SHRINK"], |refs| {
        assert_eq!(
            memfd_parse(refs).unwrap().seals,
            sys::memfd::F_SEAL_SHRINK as u32
        );
    });
}

#[test]
fn seal_accumulates_selected_flags() {
    with_refs(&["--seal", "SHRINK", "--seal", "WRITE"], |refs| {
        let cfg = memfd_parse(refs).unwrap();
        assert_eq!(
            cfg.seals,
            sys::memfd::F_SEAL_SHRINK as u32 | sys::memfd::F_SEAL_WRITE as u32
        );
    });
}

#[test]
fn seal_exercises_all_known_flags() {
    // Each arm is a distinct bit; deleting any one leaves that bit unset and
    // fails here, so the GROW/SEAL/FUTURE_WRITE/EXEC arms are covered.
    with_refs(
        &[
            "--seal",
            "GROW",
            "--seal",
            "SEAL",
            "--seal",
            "FUTURE_WRITE",
            "--seal",
            "EXEC",
        ],
        |refs| {
            assert_eq!(
                memfd_parse(refs).unwrap().seals,
                sys::memfd::F_SEAL_GROW as u32
                    | sys::memfd::F_SEAL_SEAL as u32
                    | sys::memfd::F_SEAL_FUTURE_WRITE as u32
                    | sys::memfd::F_SEAL_EXEC as u32
            );
        },
    );
}

#[test]
fn seal_rejects_unknown() {
    with_refs(&["--seal", "NOPE"], |refs| {
        assert!(is_invalid(memfd_parse(refs), "seal"));
    });
}

#[test]
fn rejects_unknown_flag() {
    with_refs(&["--bogus"], |refs| {
        assert!(is_invalid(memfd_parse(refs), "flag"));
    });
}

#[test]
fn rejects_positional_arg() {
    with_refs(&["foo"], |refs| {
        assert!(is_invalid(memfd_parse(refs), "arg"));
    });
}
