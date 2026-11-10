#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// The bare report ends with a newline: fdshell and bash 5.3.9 (measured) both
/// write exactly `0022\n`. The leading `umask 022` pins the mask, so the row
/// does not depend on the environment.
#[test]
fn the_bare_report_ends_with_a_newline() {
    let (out, err, code) = run("umask 022; umask");
    assert_eq!((out.as_str(), code), ("0022\n", 0), "stderr={err}");
}

/// `-p` alone prints the reusable `umask MODE` form — `umask 0022\n` in bash
/// (measured) and here.
#[test]
fn p_prints_the_reusable_form() {
    let (out, err, code) = run("umask 022; umask -p");
    assert_eq!((out.as_str(), code), ("umask 0022\n", 0), "stderr={err}");
}

/// With `-p` **and** a mode nothing is printed (bash measured, POSIX XCU 2.95:
/// no output when a mode operand is given) — `-p` only selects the **form** of
/// the report, and the report runs only without an operand. The printed line
/// below is the separate, operand-less `umask`.
#[test]
fn p_with_a_mode_applies_silently() {
    let (out, err, code) = run("umask -p 022; umask");
    assert_eq!((out.as_str(), code), ("0022\n", 0), "stderr={err}");
}

/// Options may repeat (XBD Utility Syntax Guidelines): `umask -p -p` is the
/// no-operand report in the `-p` form, `umask 0022\n` in bash (measured) and
/// here — the mask pinned first so the row does not read the environment.
#[test]
fn p_may_repeat() {
    let (out, err, code) = run("umask 022; umask -p -p");
    assert_eq!((out.as_str(), code), ("umask 0022\n", 0), "stderr={err}");
}

/// Decision D3 of task #472: a second operand stays a parse error (`umask takes
/// at most one argument`, rc 1) — POSIX defines a single optional `mode`
/// operand, so extras are undefined, and the rejection catches `umask 022 077`.
/// bash 5.3.9 (measured) silently ignores the extra operand at rc 0.
#[test]
fn a_second_operand_is_reported_where_bash_ignores_it() {
    for script in ["umask 022 extra", "umask 022 -p"] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err}");
        assert!(
            err.contains("takes at most one argument"),
            "{script}: stderr={err}"
        );
    }
}

/// Decision D4 of task #472: the `0o` prefix is fdshell's own extension — an
/// accepted `0o22` sets the mask silently at rc 0, where bash 5.3.9 (measured)
/// answers `umask: 0o22: octal number out of range` at rc 1. The setter path is
/// what `tests/mkdirat.rs` &co. lean on; it must not regress.
#[test]
fn the_zero_o_prefix_stays_fdshells_extension() {
    let (out, err, code) = run("umask 0o22");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
}

/// Parity row: `umask 022` applies the mask with no output, rc 0, in bash
/// (measured) and here.
#[test]
fn setting_a_mode_prints_nothing() {
    let (out, err, code) = run("umask 022");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
}

/// The quote bytes change nothing (family #335): the keyword and its operand are
/// read as word **text**, so both quoted spellings answer as the unquoted row —
/// `umask 0022\n` at rc 0, exactly as bash 5.3.9 measures `umask 022; umask '-p'`
/// (and `'umask' -p`).
#[test]
fn the_quote_bytes_of_the_keyword_and_of_p_are_transparent() {
    for script in ["umask 022; 'umask' -p", "umask 022; umask '-p'"] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            ("umask 0022\n", 0),
            "{script}: stderr={err}"
        );
    }
}
