//! Pins for `report_stray_closer`'s own answers (task #576, its first mutants
//! map): every suggestion arm names its own block, and the `time` prefix moves
//! the caret behind itself. The integration layer pins only the message lines,
//! so these rows are what notice an arm swap or a shifted operator; each exact
//! render kills the mutants of one site (CATCHING_MUTANTS rule 2).

use super::*;
use crate::debug::install_debug_hooks;
use alloc::format;
use alloc::string::String;

/// One pin: the helper's own `{:?}` with the debug hooks installed (the shape
/// `debug/tests.rs` pins the same way; the source-location line moves with the
/// file, so it is part of the expected string).
fn render(line: &[u8], pos: usize) -> String {
    install_debug_hooks();
    format!("{:?}", report_stray_closer(line, pos))
}

/// The `fi` arm (kills its delete-arm mutant, and the `+`→`-` mutant of
/// `pos + skip + word.len()` at the `keyword_kind` call — with a zero prefix
/// that mutant computes `0 - 2` and the row panics).
#[cfg(debug_assertions)]
#[test]
fn the_stray_fi_suggestion_names_the_if_block() {
    assert_eq!(
        render(b"fi", 0),
        "\u{1b}[1mstray 'fi' — it closes no open block\u{1b}[22m\n\
          ├╴at safe/fdshell/src/error/parse/position.rs:87:5\n\
          ├╴Suggestion: a stray 'fi' needs an open `if` block: open one, or drop the word\n\
          ├╴fi\n\
          ╰╴^^"
    );
}

/// The `done` arm (kills its delete-arm mutant; `+`→`-` panics here too).
#[cfg(debug_assertions)]
#[test]
fn the_stray_done_suggestion_names_the_loop_block() {
    assert_eq!(
        render(b"done", 0),
        "\u{1b}[1mstray 'done' — it closes no open block\u{1b}[22m\n\
          ├╴at safe/fdshell/src/error/parse/position.rs:87:5\n\
          ├╴Suggestion: a stray 'done' needs an open `while`/`until`/`for` block: open one, or drop the word\n\
          ├╴done\n\
          ╰╴^~~^"
    );
}

/// The `esac` arm (kills its delete-arm mutant; `+`→`-` panics here too).
#[cfg(debug_assertions)]
#[test]
fn the_stray_esac_suggestion_names_the_case_block() {
    assert_eq!(
        render(b"esac", 0),
        "\u{1b}[1mstray 'esac' — it closes no open block\u{1b}[22m\n\
          ├╴at safe/fdshell/src/error/parse/position.rs:87:5\n\
          ├╴Suggestion: a stray 'esac' needs an open `case` block: open one, or drop the word\n\
          ├╴esac\n\
          ╰╴^"
    );
}

/// The brace arm (kills its delete-arm mutant; `+`→`-` panics here too).
#[cfg(debug_assertions)]
#[test]
fn the_stray_brace_suggestion_names_the_group_block() {
    assert_eq!(
        render(b"}", 0),
        "\u{1b}[1mstray '}' — it closes no open block\u{1b}[22m\n\
          ├╴at safe/fdshell/src/error/parse/position.rs:87:5\n\
          ├╴Suggestion: a stray '}' needs an open '{ …; }` group: open one, or drop the word\n\
          ├╴}\n\
          ╰╴^"
    );
}

/// A `time`-prefixed line: the caret and the keyword run both ride the prefix
/// skip (POSIX's `time` word is transparent to the recognition table, so the
/// report names the keyword *behind* it). Kills the `pos + skip` mutants at
/// both call sites: both `-` mutants compute `0 - 5` and panic, and the
/// caret-assignment `+`→`*` lands the caret at column 0. The two `+`→`*`
/// mutants of the `keyword_kind` argument die in
/// [`a_quoted_keyword_run_keeps_the_word_data_suggestion`], which feeds the
/// helper the quoted run the scan never marks (rule 19).
#[cfg(debug_assertions)]
#[test]
fn the_time_prefix_moves_the_caret_behind_itself() {
    assert_eq!(
        render(b"time fi", 0),
        "\u{1b}[1mstray 'fi' — it closes no open block\u{1b}[22m\n\
          ├╴at safe/fdshell/src/error/parse/position.rs:87:5\n\
          ├╴Suggestion: a stray 'fi' needs an open `if` block: open one, or drop the word\n\
          ├╴time fi\n\
          ╰╴     ^^"
    );
}

/// A quoted keyword **run** (the `fi` inside `x"fi ;`) is data, so the helper's
/// own answer is the catch-all arm. The scan never marks such a word, so no
/// scan-level row reaches the `_ =>` arm — this pin feeds the helper exactly
/// that input (rule 19) and pins the answer. It kills the two `+`→`*` mutants
/// of the `keyword_kind` argument: each moves the window before the keyword,
/// `span_start` then fails to align (`line[1]` is a quote, `line[0]` is `x`),
/// the gate answers through `None => false`, and the `fi` row's suggestion
/// shows instead — a different `Suggestion:` line (rule 5).
#[cfg(debug_assertions)]
#[test]
fn a_quoted_keyword_run_keeps_the_word_data_suggestion() {
    assert_eq!(
        render(b"x\"fi ;", 2),
        "\u{1b}[1mstray 'fi' — it closes no open block\u{1b}[22m\n\
          ├╴at safe/fdshell/src/error/parse/position.rs:87:5\n\
          ├╴Suggestion: write a closing keyword only where a block of its own kind is open\n\
          ├╴x\"fi ;\n\
          ╰╴  ^^"
    );
}
