#![allow(clippy::unwrap_used)]

use super::*;
use crate::parse::{BuiltinPrefix, CommandLine};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

fn word(text: &str) -> ShortCStr {
    ShortCStr::from_vec(Vec::from(text.as_bytes())).unwrap()
}

fn make_cmdline(args: &[&str], prefix: BuiltinPrefix) -> CommandLine {
    CommandLine {
        prefix,
        command: word("set"),
        command_mask: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: args.iter().map(|s| word(s)).collect(),
        args_mask: vec![vec![]; args.len()],
        args_quoted: vec![false; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn options_of(cell: &ForkCell<ShellState>) -> u32 {
    cell.borrow().unwrap().options
}

/// `run_short_flags` for a one-word `set` line.
fn run(word_text: &str, cell: &ForkCell<ShellState>) -> Result<bool, Report<CmdError>> {
    let line = format!("set {word_text}");
    run_in(line.as_bytes(), word_text, cell)
}

fn run_in(
    line: &[u8],
    word_text: &str,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let cmd = make_cmdline(&[word_text], BuiltinPrefix::None);
    run_short_flags(&word(word_text), line, &cmd, cell)
}

#[test]
fn cluster_bits_follow_the_word_order() {
    let (bits, enable) = short_flags(&word("-eu")).unwrap().unwrap();
    assert_eq!(bits, vec![crate::options::ERREXIT, crate::options::NOUNSET]);
    assert!(enable, "`-` sets");

    let (bits, enable) = short_flags(&word("+ef")).unwrap().unwrap();
    assert_eq!(bits, vec![crate::options::ERREXIT, crate::options::NOGLOB]);
    assert!(!enable, "`+` clears");
}

#[test]
fn single_flag_words_still_parse() {
    assert_eq!(
        short_flags(&word("-e")).unwrap().unwrap(),
        (vec![crate::options::ERREXIT], true)
    );
    assert_eq!(
        short_flags(&word("+v")).unwrap().unwrap(),
        (vec![crate::options::VERBOSITY], false)
    );
    assert_eq!(
        short_flags(&word("-x")).unwrap().unwrap().0,
        vec![crate::options::XTRACE],
        "`x` is a cluster flag (bash: set -xf)"
    );
}

#[test]
fn table_short_flags_are_cluster_flags() {
    // The cluster's accepted bytes are the option table's short flags, so
    // `-C` (noclobber) and `-i` (ignoreeof) ride it like `e u f v x` do.
    assert_eq!(
        short_flags(&word("-C")).unwrap().unwrap(),
        (vec![crate::options::NOCLOBBER], true),
        "`set -C` is `set -o noclobber`"
    );
    assert_eq!(
        short_flags(&word("+C")).unwrap().unwrap(),
        (vec![crate::options::NOCLOBBER], false),
        "`+` clears the same bit"
    );
    assert_eq!(
        short_flags(&word("-Ci")).unwrap().unwrap().0,
        vec![crate::options::NOCLOBBER, crate::options::IGNOREEOF],
        "bits follow the word order"
    );
}

#[test]
fn duplicate_flag_bytes_stay_repeated() {
    assert_eq!(
        short_flags(&word("-ee")).unwrap().unwrap().0,
        vec![crate::options::ERREXIT, crate::options::ERREXIT],
    );
}

#[test]
fn unknown_cluster_byte_is_an_error_naming_the_word() {
    for text in [
        "-z",
        "-ez",
        "-e+u",
        "-abc",
        "-F",
        "-c",
        "-aC",
        "--stdout-capture-limit",
    ] {
        let err = short_flags(&word(text)).unwrap_err();
        assert!(
            matches!(
                err.current_context(),
                CmdError::ShellOptionUnknown("set", _)
            ),
            "{text}: {err:?}"
        );
        assert!(
            format!("{err:?}").contains(text),
            "error must name the whole word {text}: {err:?}"
        );
    }
}

#[test]
fn signless_empty_and_lone_sign_words_are_not_clusters() {
    for text in ["", "-", "+", "a", "ab"] {
        assert_eq!(
            short_flags(&word(text)).unwrap(),
            None,
            "{text:?} must fall through to the external lookup"
        );
    }
}

#[test]
fn cluster_error_applies_no_flag() {
    let cell = make_cell();
    assert!(run("-ez", &cell).is_err());
    assert_eq!(
        options_of(&cell) & crate::options::ERREXIT,
        0,
        "bash validates the whole cluster before applying any flag"
    );
    assert!(run("-Cz", &cell).is_err());
    assert_eq!(
        options_of(&cell) & crate::options::NOCLOBBER,
        0,
        "the new bytes obey the same validate-first rule"
    );
}

#[test]
fn cluster_clears_only_the_named_bits() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.options = crate::options::ERREXIT | crate::options::NOGLOB | crate::options::NOUNSET;
    }
    assert!(run("+ef", &cell).unwrap());
    assert_eq!(options_of(&cell), crate::options::NOUNSET);
}

#[test]
fn cluster_sets_every_named_bit_with_zero_status() {
    let cell = make_cell();
    assert!(run("-eu", &cell).unwrap());
    let state = cell.borrow().unwrap();
    let want = crate::options::ERREXIT | crate::options::NOUNSET;
    assert_eq!(state.options & want, want);
    assert_eq!(state.last_status.exit_code(), 0);
}

#[test]
fn noclobber_and_ignoreeof_clusters_set_their_bits_with_zero_status() {
    for (text, want) in [
        ("-C", crate::options::NOCLOBBER),
        ("-i", crate::options::IGNOREEOF),
        ("-Ci", crate::options::NOCLOBBER | crate::options::IGNOREEOF),
    ] {
        let cell = make_cell();
        assert!(run(text, &cell).unwrap(), "{text} is a cluster");
        let state = cell.borrow().unwrap();
        assert_eq!(state.options & want, want, "{text} sets every named bit");
        assert_eq!(
            state.last_status.exit_code(),
            0,
            "{text} ends with status 0"
        );
    }
}

#[test]
fn noclobber_and_ignoreeof_flags_clear_only_their_bits() {
    let all = crate::options::NOCLOBBER | crate::options::IGNOREEOF | crate::options::ERREXIT;
    for (text, cleared) in [
        ("+C", crate::options::NOCLOBBER),
        ("+i", crate::options::IGNOREEOF),
        ("+Ci", crate::options::NOCLOBBER | crate::options::IGNOREEOF),
    ] {
        let cell = make_cell();
        {
            let mut state = cell.borrow_mut().unwrap();
            state.options = all;
        }
        assert!(run(text, &cell).unwrap(), "{text} is a cluster");
        assert_eq!(
            options_of(&cell),
            all & !cleared,
            "{text} clears only its own bits"
        );
    }
}

#[test]
fn plain_word_returns_false_and_touches_nothing() {
    let cell = make_cell();
    let before = options_of(&cell);
    let cmd = make_cmdline(&["a", "b"], BuiltinPrefix::None);
    assert!(!run_short_flags(&word("a"), b"set a b", &cmd, &cell).unwrap());
    assert_eq!(options_of(&cell), before);
}

#[test]
fn builtin_keyword_is_rejected_for_a_valid_cluster() {
    let cell = make_cell();
    let cmd = make_cmdline(&["-e"], BuiltinPrefix::Builtin);
    let err = run_short_flags(&word("-e"), b"builtin set -e", &cmd, &cell).unwrap_err();
    assert!(
        matches!(
            err.current_context(),
            CmdError::BuiltinKeywordNotSupported { .. }
        ),
        "{err:?}"
    );
}

#[test]
fn unknown_flag_outranks_the_builtin_keyword_error() {
    let cell = make_cell();
    let cmd = make_cmdline(&["-z"], BuiltinPrefix::Builtin);
    let err = run_short_flags(&word("-z"), b"builtin set -z", &cmd, &cell).unwrap_err();
    assert!(
        matches!(
            err.current_context(),
            CmdError::ShellOptionUnknown("set", _)
        ),
        "the form test is the parse: {err:?}"
    );
}
