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

/// POSIX XCU 2.95 takes the operand "in the same way as the `mode` operand … for
/// `chmod`" (task #554), and a symbolic mode is applied to the complement of the
/// current mask. bash 5.3.9 measures each row below in a fresh child, printing nothing
/// for a mode operand; every row pins its own initial mask for that reason.
#[test]
fn a_symbolic_mode_is_applied_to_the_complement_of_the_mask() {
    for (script, want) in [
        ("umask 022; umask u=rwx; umask", "0022\n"),
        ("umask 022; umask a=rwx; umask", "0000\n"),
        ("umask 022; umask u=rwx,g=,o=; umask", "0077\n"),
        ("umask 0; umask g-w; umask", "0020\n"),
        ("umask 022; umask u=g; umask", "0222\n"),
        ("umask 022; umask u=rwx,g=r-w+rx; umask", "0022\n"),
        ("umask 0; umask a-rwx,u=X; umask", "0677\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// `-S` is POSIX's option ("Produce symbolic output"): a letter per bit the mask
/// leaves clear, the whole line from bash 5.3.9's answer.
#[test]
fn s_prints_the_symbolic_form() {
    for (script, want) in [
        ("umask 0; umask -S", "u=rwx,g=rwx,o=rwx\n"),
        ("umask 022; umask -S", "u=rwx,g=rx,o=rx\n"),
        ("umask 0777; umask -S", "u=,g=,o=\n"),
        ("umask 0644; umask -S", "u=x,g=wx,o=wx\n"),
        ("umask 0111; umask -S", "u=rw,g=rw,o=rw\n"),
        ("umask 0240; umask -S", "u=rx,g=wx,o=rwx\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// `-p` and `-S` together, in either order and in either spelling — bash's
/// `internal_getopt` takes clusters and repeats, and `-p` puts its reusable prefix in
/// front of the symbolic line too (`umask -S …`, measured).
#[test]
fn p_and_s_report_together() {
    for script in [
        "umask 022; umask -p -S",
        "umask 022; umask -S -p",
        "umask 022; umask -Sp",
        "umask 022; umask -pS",
        "umask 022; umask -p -p -S -S",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            ("umask -S u=rwx,g=rx,o=rx\n", 0),
            "{script}: stderr={err}"
        );
    }
}

/// Decision D4 of task #554: `-S` with a mode operand prints the symbolic form of the
/// mask it just wrote — POSIX says a mask operand writes nothing, bash prints, and
/// fdshell follows bash (`POSIX_COMPATIBILITY.md`). The `-p` prefix stays off that
/// line, which is the no-operand report's.
#[test]
fn s_with_a_mode_operand_prints_the_result() {
    for (script, want) in [
        ("umask 022; umask -S 077; umask", "u=rwx,g=,o=\n0077\n"),
        ("umask 022; umask -S u=rwx", "u=rwx,g=rx,o=rx\n"),
        ("umask 022; umask -p -S 077", "u=rwx,g=,o=\n"),
        ("umask 022; umask -S u=rwx,g=,o=", "u=rwx,g=,o=\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// A lone `--` ends the options, so what follows is the operand — and a bare `-` is an
/// operand too (its operator is that `-`, so it is bash's no-op clause). bash 5.3.9
/// measured, matching.
#[test]
fn the_double_dash_ends_the_options() {
    for (script, want) in [
        ("umask 022; umask -- -x; umask", "0133\n"),
        ("umask 022; umask --; umask", "0022\n0022\n"),
        ("umask 022; umask -; umask", "0022\n"),
        ("umask 022; umask -- u=g; umask", "0222\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// An option letter that is not `p` or `S` is reported — bash names the letter and
/// answers its usage error at rc 2, fdshell names the word at the shell-wide rc 1
/// (the rc-1-vs-2 family, `POSIX_COMPATIBILITY.md`), with the accepted options named
/// so the script can be fixed (STYLE §4.7).
#[test]
fn an_unknown_option_is_reported() {
    for (script, want) in [
        ("umask 022; umask -x", "invalid umask option '-x'"),
        ("umask 022; umask -Sq", "invalid umask option '-Sq'"),
        ("umask 022; umask -P", "invalid umask option '-P'"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err}");
        assert!(err.contains(want), "{script}: stderr={err}");
    }
}

/// The symbolic grammar's two errors, at bash's characters: a clause needs an operator
/// right after its `who` letters (bash's `%c` of the end of the word prints as the
/// empty one), and an action run ends at a letter it does not know. rc 1 in both shells.
#[test]
fn a_bad_symbolic_mode_names_its_character() {
    for (script, want) in [
        ("umask u+q", "invalid umask symbolic mode character 'q'"),
        ("umask u", "invalid umask symbolic mode operator ''"),
        ("umask u=rwx,", "invalid umask symbolic mode operator ''"),
        ("umask ub", "invalid umask symbolic mode operator 'b'"),
        ("umask ,", "invalid umask symbolic mode operator ','"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err}");
        assert!(err.contains(want), "{script}: stderr={err}");
    }
}

/// Task #472's decisions, unchanged: a second operand is fdshell's error where bash
/// ignores it silently (and a flag after the operand *is* that second operand), and a
/// digit-first word stays the octal grammar — `8` is the octal error, not a symbolic
/// clause.
#[test]
fn the_operand_policy_of_task_472_stands() {
    for (script, want) in [
        ("umask 022 extra", "takes at most one argument"),
        ("umask 022 -p", "takes at most one argument"),
        ("umask u=rwx extra", "takes at most one argument"),
        ("umask u=rwx -S", "takes at most one argument"),
        ("umask 8", "not a valid octal number"),
        ("umask 0o078", "not a valid octal number"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: stderr={err}");
        assert!(err.contains(want), "{script}: stderr={err}");
    }
}

/// The quote bytes change nothing on the new spellings either (family #335): the
/// keyword, the flags and the operand are all read as word **text**, so each quoted
/// row answers exactly like its unquoted twin.
#[test]
fn the_quote_bytes_of_the_new_spellings_are_transparent() {
    for (script, want) in [
        ("umask 022; 'umask' 'u=rwx'; umask", "0022\n"),
        ("umask 022; umask '-S'", "u=rwx,g=rx,o=rx\n"),
        ("umask 022; umask '-p' '-S'", "umask -S u=rwx,g=rx,o=rx\n"),
        ("umask '022'; 'umask'", "0022\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}
