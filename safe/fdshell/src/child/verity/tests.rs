#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use error_stack::Report;
use sys::fsverity::FsverityDigest;
use sys::{Origin, ShortCStr, Trace};

use crate::child::Ctx;
use crate::state::{FdVar, ShellState};

use super::dump::{MetadataType, parse_metadata_type, parse_uint};
use super::emit::{algo_name, line, to_hex, write_hex_rows};
use super::handle_verity;
use super::parse::{VerityConfig, verity_parse};

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

fn is_invalid(e: &Report<BuiltinError>, what: &'static str) -> bool {
    matches!(e.current_context(), BuiltinError::InvalidArgument(s) if *s == what)
}

fn parse(args: &[&str]) -> Result<VerityConfig, Report<BuiltinError>> {
    with_refs(args, verity_parse)
}

#[test]
fn query_form_defaults() {
    let cfg = parse(&["%f"]).unwrap();
    assert_eq!(cfg.var.as_bytes().unwrap(), b"f");
    assert!(!cfg.enable);
    assert_eq!(cfg.algo, sys::fsverity::FS_VERITY_HASH_ALG_SHA256);
    assert!(cfg.expected.is_none());
}

#[test]
fn enable_flag() {
    let cfg = parse(&["%f", "--enable"]).unwrap();
    assert!(cfg.enable);
    assert_eq!(cfg.algo, sys::fsverity::FS_VERITY_HASH_ALG_SHA256);
}

#[test]
fn algo_separate_word() {
    assert_eq!(
        parse(&["%f", "--enable", "--algo", "sha512"]).unwrap().algo,
        2
    );
    assert_eq!(
        parse(&["%f", "--enable", "--algo", "sha256"]).unwrap().algo,
        1
    );
}

#[test]
fn algo_inline() {
    assert_eq!(parse(&["%f", "--enable", "--algo=sha512"]).unwrap().algo, 2);
    assert_eq!(parse(&["%f", "--enable", "--algo=sha256"]).unwrap().algo, 1);
}

#[test]
fn digest_separate_word() {
    let cfg = parse(&["%f", "--digest", "512eb3"]).unwrap();
    assert_eq!(cfg.expected.as_deref().unwrap(), &[0x51, 0x2e, 0xb3]);
}

#[test]
fn digest_inline() {
    let cfg = parse(&["%f", "--digest=512eb3"]).unwrap();
    assert_eq!(cfg.expected.as_deref().unwrap(), &[0x51, 0x2e, 0xb3]);
}

#[test]
fn digest_is_case_insensitive_hex() {
    let lower = parse(&["%f", "--digest", "ABCD"])
        .unwrap()
        .expected
        .unwrap();
    let upper = parse(&["%f", "--digest", "abcd"])
        .unwrap()
        .expected
        .unwrap();
    assert_eq!(lower, upper);
    assert_eq!(lower, vec![0xab, 0xcd]);
}

#[test]
fn enable_and_digest_rejected() {
    let e = parse(&["%f", "--enable", "--digest", "512e"]).unwrap_err();
    assert!(is_invalid(&e, "--digest"));
}

#[test]
fn unknown_flag_rejected() {
    for args in [&["%f", "--bogus"][..], &["%f", "extra"][..]] {
        let e = parse(args).unwrap_err();
        assert!(is_invalid(&e, "flag"), "{args:?}");
    }
}

#[test]
fn unknown_algo_rejected() {
    let e = parse(&["%f", "--enable", "--algo", "md5"]).unwrap_err();
    assert!(is_invalid(&e, "algo"));
}

#[test]
fn algo_missing_value_rejected() {
    let e = parse(&["%f", "--enable", "--algo"]).unwrap_err();
    assert!(is_invalid(&e, "--algo"));
}

#[test]
fn bad_digest_rejected() {
    for args in [
        &["%f", "--digest", "5"][..],  // odd length
        &["%f", "--digest", ""][..],   // empty
        &["%f", "--digest", "zz"][..], // even length, non-hex char
        &["%f", "--digest", "0g"][..], // one valid, one non-hex nibble
    ] {
        let e = parse(args).unwrap_err();
        assert!(is_invalid(&e, "digest"), "{args:?}");
    }
}

#[test]
fn enable_takes_no_value() {
    let e = parse(&["%f", "--enable=yes"]).unwrap_err();
    assert!(is_invalid(&e, "--enable"));
}

#[test]
fn duplicate_enable_rejected() {
    let e = parse(&["%f", "--enable", "--enable"]).unwrap_err();
    assert!(is_invalid(&e, "--enable"));
}

#[test]
fn bare_path_rejected() {
    let e = parse(&["/bin/foo"]).unwrap_err();
    assert!(is_invalid(&e, "fd var"));
}

#[test]
fn double_percent_rejected() {
    let e = parse(&["%%"]).unwrap_err();
    assert!(is_invalid(&e, "fd var"));
}

#[test]
fn missing_var_rejected() {
    let e = parse(&[]).unwrap_err();
    assert!(matches!(
        e.current_context(),
        BuiltinError::MissingArgument("fd var")
    ));
}

#[test]
fn help() {
    let e = parse(&["--help"]).unwrap_err();
    assert!(matches!(e.current_context(), BuiltinError::Help));
}

// --- dump parsing (pure) ---------------------------------------------------

#[test]
fn dump_type_names() {
    assert_eq!(
        parse_metadata_type(b"tree").unwrap(),
        MetadataType::MerkleTree
    );
    assert_eq!(
        parse_metadata_type(b"merkle_tree").unwrap(),
        MetadataType::MerkleTree
    );
    assert_eq!(
        parse_metadata_type(b"descriptor").unwrap(),
        MetadataType::Descriptor
    );
    assert_eq!(
        parse_metadata_type(b"signature").unwrap(),
        MetadataType::Signature
    );
}

#[test]
fn dump_type_unknown_rejected() {
    let cases: [&[u8]; 3] = [b"bogus", b"root", b"TREE"];
    for v in cases {
        let e = parse_metadata_type(v).unwrap_err();
        assert!(is_invalid(&e, "dump type"), "{v:?}");
    }
}

#[test]
fn parse_uint_valid() {
    assert_eq!(parse_uint(b"0", "x").unwrap(), 0);
    assert_eq!(parse_uint(b"42", "x").unwrap(), 42);
    assert_eq!(parse_uint(b"18446744073709551615", "x").unwrap(), u64::MAX);
}

#[test]
fn parse_uint_rejected() {
    let cases: [&[u8]; 5] = [b"", b"abc", b"-1", b"18446744073709551616", b"1.5"];
    for v in cases {
        let e = parse_uint(v, "x").unwrap_err();
        assert!(is_invalid(&e, "x"), "{v:?}");
    }
}

#[test]
fn dump_spec_parsed() {
    let cfg = parse(&["%f", "--dump", "descriptor"]).unwrap();
    let d = cfg.dump.unwrap();
    assert_eq!(d.r#type, MetadataType::Descriptor);
    assert_eq!(d.offset, 0);
    assert!(d.length.is_none());
}

#[test]
fn dump_spec_offset_length() {
    let cfg = parse(&["%f", "--dump", "tree", "--offset", "8", "--length", "16"]).unwrap();
    let d = cfg.dump.unwrap();
    assert_eq!(d.r#type, MetadataType::MerkleTree);
    assert_eq!(d.offset, 8);
    assert_eq!(d.length, Some(16));
}

#[test]
fn dump_inline() {
    let cfg = parse(&["%f", "--dump=descriptor"]).unwrap();
    assert_eq!(cfg.dump.unwrap().r#type, MetadataType::Descriptor);
}

#[test]
fn dump_and_enable_rejected() {
    let e = parse(&["%f", "--dump", "descriptor", "--enable"]).unwrap_err();
    assert!(is_invalid(&e, "--dump"));
}

#[test]
fn dump_and_digest_rejected() {
    let e = parse(&["%f", "--dump", "descriptor", "--digest", "512e"]).unwrap_err();
    assert!(is_invalid(&e, "--dump"));
}

#[test]
fn offset_without_dump_rejected() {
    let e = parse(&["%f", "--offset", "8"]).unwrap_err();
    assert!(is_invalid(&e, "--offset"));
}

#[test]
fn length_without_dump_rejected() {
    let e = parse(&["%f", "--length", "8"]).unwrap_err();
    assert!(is_invalid(&e, "--length"));
}

#[test]
fn duplicate_dump_rejected() {
    let e = parse(&["%f", "--dump", "descriptor", "--dump", "tree"]).unwrap_err();
    assert!(is_invalid(&e, "--dump"));
}

#[test]
fn duplicate_offset_rejected() {
    let e = parse(&[
        "%f",
        "--dump",
        "descriptor",
        "--offset",
        "8",
        "--offset",
        "16",
    ])
    .unwrap_err();
    assert!(is_invalid(&e, "--offset"));
}

#[test]
fn duplicate_length_rejected() {
    let e = parse(&[
        "%f",
        "--dump",
        "descriptor",
        "--length",
        "8",
        "--length",
        "16",
    ])
    .unwrap_err();
    assert!(is_invalid(&e, "--length"));
}

// --- emit (pure) -----------------------------------------------------------

#[test]
fn emit_line_none() {
    let got = line(None).unwrap();
    assert_eq!(got.as_bytes().unwrap(), b"enabled=no\n");
}

#[test]
fn emit_line_formats_digest() {
    let d = FsverityDigest {
        algorithm: 1,
        size: 32,
        digest: vec![0xab, 0xcd],
    };
    let got = line(Some(&d)).unwrap();
    assert_eq!(
        got.as_bytes().unwrap(),
        b"enabled=yes algo=sha256 digest=abcd\n"
    );
}

#[test]
fn emit_algo_name() {
    assert_eq!(algo_name(1), "sha256");
    assert_eq!(algo_name(2), "sha512");
    assert_eq!(algo_name(9), "unknown");
}

#[test]
fn emit_to_hex_lowercases() {
    assert_eq!(to_hex(&[0x0a, 0xff, 0x1b]), "0aff1b");
}

#[test]
fn hex_rows_full_row() {
    let data: Vec<u8> = (0..16).collect();
    let got = write_hex_rows(0, &data).unwrap();
    assert_eq!(
        got.as_bytes().unwrap(),
        b"00000000  00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f\n"
    );
}

#[test]
fn hex_rows_offset_and_padding() {
    let got = write_hex_rows(0x10, &[0xab, 0xcd]).unwrap();
    assert_eq!(
        got.as_bytes().unwrap(),
        b"00000010  ab cd __ __ __ __ __ __  __ __ __ __ __ __ __ __\n"
    );
}

#[test]
fn hex_rows_multiple_rows() {
    let data: Vec<u8> = (0..20).collect();
    let got = write_hex_rows(0, &data).unwrap();
    let lines: Vec<&[u8]> = got.as_bytes().unwrap().split(|&b| b == b'\n').collect();
    assert_eq!(lines.len(), 3, "2 rows + trailing empty");
    assert_eq!(
        lines.first().copied(),
        Some(b"00000000  00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f" as &[u8])
    );
    assert_eq!(
        lines.get(1).copied(),
        Some(b"00000010  10 11 12 13 __ __ __ __  __ __ __ __ __ __ __ __" as &[u8])
    );
}

// --- check (pure) ----------------------------------------------------------

fn digest(algo: u16, bytes: &[u8]) -> FsverityDigest {
    FsverityDigest {
        algorithm: algo,
        size: bytes.len() as u16,
        digest: bytes.to_vec(),
    }
}

#[test]
fn check_fails_closed_when_not_verity() {
    // A non-verity file measures to `None`: any expected digest must fail.
    assert_eq!(super::check(None, &[0xab]), 1);
}

#[test]
fn check_passes_on_match() {
    let d = digest(1, &[0xab, 0xcd]);
    assert_eq!(super::check(Some(&d), &[0xab, 0xcd]), 0);
}

#[test]
fn check_fails_on_mismatch() {
    let d = digest(1, &[0xab, 0xcd]);
    assert_eq!(super::check(Some(&d), &[0xab, 0xce]), 1);
    assert_eq!(super::check(Some(&d), &[0xab]), 1); // length mismatch
}

// --- handler (deterministic paths; fs-dependent success paths are e2e) -----

fn state_with_memfd() -> ShellState {
    let mut state = ShellState::new();
    let fd = sys::memfd::memfd_create().unwrap();
    state.fds.insert(
        c"f".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    state
}

fn run_handler(state: &ShellState, args: &[&str]) -> Result<i32, Report<BuiltinError>> {
    with_refs(args, |refs, origs| {
        handle_verity(&Ctx::new(c"verity".into(), refs, origs, state))
    })
}

/// A memfd is never verity-capable (tmpfs), so `measure` returns `ENOTTY`:
/// the handler surfaces it as a syscall error rather than a silent `enabled=no`.
/// (A verity-capable fs returns `ENODATA` → `enabled=no`; covered by e2e.)
#[test]
fn handler_unsupported_fs_is_syscall_error() {
    let state = state_with_memfd();
    let e = run_handler(&state, &["%f"]).unwrap_err();
    assert!(matches!(e.current_context(), BuiltinError::Syscall));
    let errno = e
        .downcast_ref::<sys::SyscallError>()
        .copied()
        .unwrap()
        .errno();
    assert_eq!(
        errno, 25,
        "expected ENOTTY from a memfd (tmpfs has no verity)"
    );
}

#[test]
fn handler_missing_var() {
    let state = ShellState::new();
    let e = run_handler(&state, &["%nope"]).unwrap_err();
    assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
}

#[test]
fn handler_help() {
    let state = ShellState::new();
    let e = run_handler(&state, &["--help"]).unwrap_err();
    assert!(matches!(e.current_context(), BuiltinError::Help));
}
