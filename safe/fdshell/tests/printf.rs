#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// `run` for outputs that are not valid UTF-8, as raw bytes.
fn run_bytes(script: &str) -> (Vec<u8>, Vec<u8>, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    (
        output.stdout,
        output.stderr,
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn printf_basic_format() {
    let (out, _err, code) = run(r#"printf "%s=%d" hi 42"#);
    assert_eq!(code, 0);
    assert_eq!(out, "hi=42");
}

#[test]
fn printf_with_explicit_newline() {
    // POSIX #4.1: outside quotes `\n` is the escape pair `n`, so the unquoted
    // format word is `%sn` — bash prints `hellon` for the same script, and the
    // quoted form `"%s\n"` (POSIX #4.2 keeps the pair for printf to interpret)
    // is what prints the newline.
    let (out, _err, code) = run(r"printf %s\n hello");
    assert_eq!(code, 0);
    assert_eq!(out, "hellon");
    let (out, _err, code) = run(r#"printf "%s\n" hello"#);
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn printf_reuses_format() {
    let (out, _err, code) = run(r#"printf "[%s] " a b"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a] [b] ");
}

#[test]
fn printf_without_format_prints_newline() {
    let (out, _err, code) = run("printf");
    assert_eq!(code, 0);
    assert_eq!(out, "\n");
}

#[test]
fn printf_builtin_prefix() {
    let (out, _err, code) = run(r#"builtin printf "%d" 7"#);
    assert_eq!(code, 0);
    assert_eq!(out, "7");
}

#[test]
fn printf_invalid_number_fails() {
    let (_out, err, code) = run(r#"printf "%d" abc"#);
    assert_ne!(code, 0);
    assert!(err.contains("number"), "stderr={err:?}");
}

#[test]
fn printf_in_conditional() {
    let (out, _err, code) = run(r#"if printf "%s" x >/dev/null; then printf ok; fi"#);
    assert_eq!(code, 0);
    assert_eq!(out, "ok");
}

// --- End-to-end bash-diff battery (POSIX/XCU conversion table) ---

#[test]
fn printf_b_interprets_escapes() {
    let (out, _err, code) = run(r#"printf "%b" "a\tb\n""#);
    assert_eq!(code, 0);
    assert_eq!(out, "a\tb\n");
}

// --- `%b`'s own escape rules (POSIX, values measured against bash 5.3.9) ---

#[test]
fn printf_b_zero_prefixed_octal() {
    // `\0` plus up to three further octal digits, unlike the format string's
    // three-digit rule; ASCII outputs only (the `run` helper decodes UTF-8).
    let (out, _err, code) = run(r"printf '%b' '\0101'");
    assert_eq!(code, 0);
    assert_eq!(out, "A");
    let (out, _err, code) = run(r"printf '%b' '\0040'");
    assert_eq!(code, 0);
    assert_eq!(out, " ");
}

#[test]
fn printf_b_backslash_c_stops_output() {
    // `\c` stops the operand, the remaining operands and the rest of the
    // format; a `\c` in the format string itself is passed through, as bash
    // measures it.
    let (out, _err, code) = run(r"printf 'a\cb'; echo X");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\cbX\n");
    let (out, _err, code) = run(r"printf 'x%by\n' 'a\cb'; echo after");
    assert_eq!(code, 0);
    assert_eq!(out, "xaafter\n");
    let (out, _err, code) = run(r"printf 'A%bz%s\n' 'x\cy' tail");
    assert_eq!(code, 0);
    assert_eq!(out, "Ax");
    let (out, _err, code) = run(r"printf '%b' '\c'; echo rc=$?");
    assert_eq!(code, 0);
    assert_eq!(out, "rc=0\n");
    let (out, _err, code) = run(r"printf '%b' 'a\c' extra");
    assert_eq!(code, 0);
    assert_eq!(out, "a");
}

#[test]
fn printf_b_precision_after_expansion() {
    // The precision truncates the converted string, not the raw operand.
    let (out, _err, code) = run(r"printf '%.2b' 'A\101B'");
    assert_eq!(code, 0);
    assert_eq!(out, "AA");
    let (out, _err, code) = run(r"printf '%.1b' 'ab\cde'");
    assert_eq!(code, 0);
    assert_eq!(out, "a");
}

#[test]
fn printf_b_padding_survives_the_stop() {
    // The padding due for the converted prefix is emitted before `\c` halts
    // the rest of the format.
    let (out, _err, code) = run(r"printf '[%10b]' 'a\cb'");
    assert_eq!(code, 0);
    assert_eq!(out, "[         a");
}

// --- The `\x` hex escape, on both surfaces (values measured against bash 5.3.9) ---
//
// Format words are single-quoted throughout: outside quotes `\x` is the shell's
// escape pair `x` (POSIX #4.1), which would take the decoder out of the frame.

#[test]
fn printf_hex_escape_in_format() {
    let (out, err, code) = run(r"printf '\x41\n'");
    assert_eq!((out.as_str(), err.as_str(), code), ("A\n", "", 0));
    let (out, err, code) = run(r"printf '\x4\n'");
    assert_eq!((out.as_str(), err.as_str(), code), ("\u{4}\n", "", 0));
    let (out, err, code) = run(r"printf '\xA\n'");
    assert_eq!((out.as_str(), err.as_str(), code), ("\n\n", "", 0));
    let (out, err, code) = run_bytes(r"printf '\xff\n'");
    assert_eq!(
        (out.as_slice(), err.as_slice(), code),
        (&b"\xff\n"[..], &b""[..], 0)
    );
}

#[test]
fn printf_hex_escape_takes_two_digits_at_most() {
    // The cap is bash's, not ISO C's open-ended `\xhh`.
    let (out, err, code) = run(r"printf '\x414\n'");
    assert_eq!((out.as_str(), err.as_str(), code), ("A4\n", "", 0));
}

#[test]
fn printf_b_expands_hex_escapes() {
    // The argument path shares the scanner, so the same rule applies there —
    // and the precision counts the decoded bytes.
    let (out, _err, code) = run(r"printf '%b\n' 'A\x42B'");
    assert_eq!((out.as_str(), code), ("ABB\n", 0));
    let (out, _err, code) = run(r"printf '%b %b\n' '\x41' '\x42'");
    assert_eq!((out.as_str(), code), ("A B\n", 0));
    let (out, _err, code) = run(r"printf '%.2b\n' 'A\x41B'");
    assert_eq!((out.as_str(), code), ("AA\n", 0));
}

#[test]
fn printf_uppercase_hex_escape_stays_literal() {
    // Lowercase `\x` is the escape; `\X` is not.
    let (out, err, code) = run(r"printf '\X41\n'");
    assert_eq!((out.as_str(), err.as_str(), code), ("\\X41\n", "", 0));
}

#[test]
fn printf_percent_s_leaves_the_hex_escape_literal() {
    // Only the format string and `%b` run the escape scanner.
    let (out, err, code) = run(r"printf '%s\n' '\x41'");
    assert_eq!((out.as_str(), err.as_str(), code), ("\\x41\n", "", 0));
}

#[test]
fn printf_hex_escape_without_a_digit_reports_and_continues() {
    // bash's diagnostic, printed once per occurrence, and rc stays 0.
    let (out, err, code) = run(r"printf '\xZZ\n'");
    assert_eq!((out.as_str(), code), ("\\xZZ\n", 0));
    assert!(err.contains("missing hex digit for \\x"), "stderr={err:?}");
    let (out, err, code) = run(r"printf '\xZ %s\n' a b");
    assert_eq!((out.as_str(), code), ("\\xZ a\n\\xZ b\n", 0));
    assert_eq!(
        err.matches("missing hex digit for \\x").count(),
        2,
        "stderr={err:?}"
    );
    let (out, err, code) = run(r"printf '%b %b\n' '\xZ' '\xZ'");
    assert_eq!((out.as_str(), code), ("\\xZ \\xZ\n", 0));
    assert_eq!(
        err.matches("missing hex digit for \\x").count(),
        2,
        "stderr={err:?}"
    );
}

#[test]
fn printf_reuses_format_until_args_exhausted() {
    // One arg for two `%s`: the second is empty, the format is not re-run.
    let (out, _err, code) = run(r#"printf "%s %s\n" one"#);
    assert_eq!(code, 0);
    assert_eq!(out, "one \n");
}

#[test]
fn printf_left_justifies_width() {
    let (out, _err, code) = run(r#"printf "%-5s|%s\n" a b"#);
    assert_eq!(code, 0);
    assert_eq!(out, "a    |b\n");
}

#[test]
fn printf_q_escapes_metacharacters() {
    let (out, _err, code) = run(r#"printf "%q" "a b""#);
    assert_eq!(code, 0);
    assert_eq!(out, "a\\ b");
}

#[test]
fn printf_float_width_and_precision() {
    let (out, _err, code) = run(r#"printf "%5.2f" 3.14159"#);
    assert_eq!(code, 0);
    assert_eq!(out, " 3.14");
}

#[test]
fn printf_exponent_precision() {
    let (out, _err, code) = run(r#"printf "%.3e" 123456.7"#);
    assert_eq!(code, 0);
    assert_eq!(out, "1.235e+05");
}

#[test]
fn printf_g_picks_scientific() {
    let (out, _err, code) = run(r#"printf "%g" 1234567"#);
    assert_eq!(code, 0);
    assert_eq!(out, "1.23457e+06");
}

#[test]
fn printf_hexfloat() {
    let (out, _err, code) = run(r#"printf "%a" 1.5"#);
    assert_eq!(code, 0);
    assert_eq!(out, "0xcp-3");
}

#[test]
fn printf_int_accepts_hex_input() {
    let (out, _err, code) = run(r#"printf "%d" 0x1f"#);
    assert_eq!(code, 0);
    assert_eq!(out, "31");
}

#[test]
fn printf_int_trailing_junk_prints_prefix() {
    let (out, err, code) = run(r#"printf "%d" 42x"#);
    assert_eq!(code, 1);
    assert_eq!(out, "42");
    assert!(err.contains("invalid number"), "stderr={err:?}");
}

#[test]
fn printf_int_overflow_clamps_to_i64_max() {
    let (out, err, code) = run(r#"printf "%d" 99999999999999999999"#);
    assert_eq!(code, 1);
    assert_eq!(out, "9223372036854775807");
    assert!(err.contains("out of range"), "stderr={err:?}");
}

#[test]
fn printf_unknown_conversion_stops_with_error() {
    let (out, _err, code) = run(r#"printf "%z" a"#);
    assert_eq!(code, 1);
    assert!(out.is_empty());
}

#[test]
fn printf_trailing_percent_stops_with_error() {
    let (out, _err, code) = run(r#"printf "a%""#);
    assert_eq!(code, 1);
    assert!(out.is_empty());
}

#[test]
fn printf_star_width_from_argument() {
    let (out, _err, code) = run(r#"printf "%*d" 5 7"#);
    assert_eq!(code, 0);
    assert_eq!(out, "    7");
}

// --- `%%` is the builtin's escape, not the shell's (task #155) ---
//
// Every expectation here is measured against bash 5.3.9: the shell must pass
// both `%` bytes through to argv so the printf rules below apply.

#[test]
fn printf_percent_percent_is_a_literal_percent() {
    let (out, _err, code) = run("printf '%%'");
    assert_eq!(code, 0);
    assert_eq!(out, "%");
}

#[test]
fn printf_double_percent_mid_format() {
    let (out, _err, code) = run("printf 'a%%b'");
    assert_eq!(code, 0);
    assert_eq!(out, "a%b");
}

#[test]
fn printf_double_percent_before_a_conversion_letter() {
    // `%%o` is a literal `%` followed by the letter `o`, not the `%o`
    // conversion: bash prints the two characters and consumes no argument.
    let (out, _err, code) = run("printf '%%o'");
    assert_eq!(code, 0);
    assert_eq!(out, "%o");
    let (out, _err, code) = run("printf '%%d' 7");
    assert_eq!(code, 0);
    assert_eq!(out, "%d");
}

#[test]
fn printf_double_percent_at_the_end() {
    let (out, _err, code) = run("printf '%s%%' 7");
    assert_eq!(code, 0);
    assert_eq!(out, "7%");
}

#[test]
fn printf_double_percent_with_escape_pair() {
    // Quoted `%%\n`: printf sees both `%` and the `\n` pair it must interpret.
    let (out, _err, code) = run(r"printf '100%%\n'");
    assert_eq!(code, 0);
    assert_eq!(out, "100%\n");
    let (out, _err, code) = run(r"printf '100%%done'");
    assert_eq!(code, 0);
    assert_eq!(out, "100%done");
}

#[test]
fn printf_double_percent_survives_as_argument() {
    // `%%` as a plain argument is data: bash prints it verbatim.
    let (out, _err, code) = run("printf '%s' '%%'");
    assert_eq!(code, 0);
    assert_eq!(out, "%%");
}

#[test]
fn printf_double_percent_with_bound_fd_name() {
    // Task #252's quoting gate, now implemented: the `%` arm is gated on the
    // trigger byte's mask bit, so `printf '%%s' x` with fd `s` bound keeps the
    // format whole — `%%` reaches the printf builtin, which interprets it — and
    // fdshell prints `%s` at rc 0, exactly as bash (measured 2026-11, bash
    // 5.3.9). An unquoted fd reference in a non-format word still substitutes
    // (README "File descriptor variables").
    let open = "builtin openat2 --flags O_RDONLY /dev/null %>%s; ";
    let (out, err, code) = run(&format!("{open}printf '%%s' x"));
    assert_eq!((out.as_str(), code), ("%s", 0), "stderr={err:?}");
    let (out, _err, code) = run(&format!("{open}printf '%s' %s"));
    assert_eq!(code, 0);
    assert!(
        !out.is_empty() && out.bytes().all(|b| b.is_ascii_digit()),
        "stdout={out:?}"
    );
}
