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
    // Accepted divergence, pinned (README + task #252): with fd `s` bound the
    // second `%` starts a name lookup, so the format becomes `%<fdnum>` —
    // printf reports it as a missing format character at rc 1 with empty
    // stdout, where bash's `printf '%%s' x` prints `%s` at rc 0. The master
    // collapsing reading (`printf '%%s' x` → `x`) is gone, and an unquoted fd
    // reference in a non-format word still substitutes.
    let open = "builtin openat2 --flags O_RDONLY /dev/null %>%s; ";
    let (out, err, code) = run(&format!("{open}printf '%%s' x"));
    assert_eq!(code, 1);
    assert!(out.is_empty(), "stdout={out:?}");
    assert!(err.contains("invalid format string"), "stderr={err:?}");
    let (out, _err, code) = run(&format!("{open}printf '%s' %s"));
    assert_eq!(code, 0);
    assert!(
        !out.is_empty() && out.bytes().all(|b| b.is_ascii_digit()),
        "stdout={out:?}"
    );
}
