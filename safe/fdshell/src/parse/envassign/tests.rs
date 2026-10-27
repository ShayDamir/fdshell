#![allow(clippy::unwrap_used)]
use super::*;
use crate::parse::token::tokenize;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// The tokens of a statement, exactly as the parse layer hands them here.
fn toks(line: &[u8]) -> Vec<Token> {
    tokenize(line).unwrap()
}

fn name_of(a: &Assign) -> String {
    String::from_utf8(a.name.as_bytes().unwrap().to_vec()).unwrap()
}

/// The names of the `NAME=value` words the scoped-prefix read takes from the
/// front of the statement (`prefix_len` + `collect_range`, the pair the command
/// path runs with).
fn prefixes(line: &[u8], want: &[&str]) {
    let tokens = toks(line);
    let got: Vec<String> = collect_range(&tokens, prefix_len(&tokens))
        .iter()
        .map(name_of)
        .collect();
    let want: Vec<String> = want.iter().map(|s| s.to_string()).collect();
    assert_eq!(got, want, "line {line:?}");
}

/// Every word of an all-assignment statement as names, or `None` when the
/// all-assignment read fails.
fn all_names(line: &[u8]) -> Option<Vec<String>> {
    collect_all(&toks(line)).map(|v| v.iter().map(name_of).collect())
}

/// One expected `Assign`, so the value/mask fields of a real word are built by
/// the production parser and only the names are compared.
fn assign(name: &str) -> Assign {
    let word = ShortCStr::from_vec(format!("{name}=1").into_bytes()).unwrap();
    let mask = vec![MaskBit::Unquoted; name.len() + 2];
    assign_of(&word, &mask, &[]).unwrap()
}

/// The statement bound of the scoped-prefix walk is the **tokenizer's** `;`
/// token, read through [`span_word`] (task #336): a quoted `";"` is word data,
/// so it bounds nothing and `X=1 ";"` keeps its prefix with the data word as the
/// command. Every row is paired with its unquoted twin, so a predicate that
/// drops the span gate is dead.
#[test]
fn the_statement_bound_is_the_unquoted_semicolon() {
    prefixes(b"X=1 cmd", &["X"]);
    prefixes(b"X=1 Y=2 cmd", &["X", "Y"]);
    // The real `;` bounds the statement, and the statement's last word is never
    // a prefix word: `X=1` alone owns the whole statement.
    prefixes(b"X=1; echo x", &[]);
    prefixes(b"X=1 ;", &[]);
    prefixes(b"X=1", &[]);
    // The quoted `";"` sits **inside** the statement, so the prefix survives and
    // the data word is the command (`X=1 ";"` runs the command `;`).
    prefixes(b"X=1 \";\" echo x", &["X"]);
    prefixes(b"X=1 \";\"", &["X"]);
    prefixes(b"X=1 ';' Y=2", &["X"]);
    // The data word is the command, so a second assignment behind it is its
    // argument, never a second prefix word.
    prefixes(b"X=1 \";\" Y=2 Z=3", &["X"]);
    // An escaped separator is a word too, and a quoted separator glued into a
    // value is one word (`X="1;2"`), read whole.
    prefixes(b"X=1 \\; echo x", &["X"]);
    prefixes(b"X=\"1;2\" cmd", &["X"]);
    // The gate needs a real word: `assign` is the parser's own reading of the
    // same shape, kept as the constructor the mask/spans contract pins.
    assert_eq!(name_of(&assign("X")), "X");
}

/// [`has_command_word`] answers on the same bound, so the quoted `";"` **is** a
/// command word and the all-assignment branch must not claim the statement.
#[test]
fn a_quoted_semicolon_is_a_command_word() {
    assert!(!has_command_word(&toks(b"X=1")));
    assert!(!has_command_word(&toks(b"X=1 Y=2")));
    // The real `;` bounds the scan: the assignment is all the statement owns.
    assert!(!has_command_word(&toks(b"X=1; echo x")));
    // The quoted and escaped spellings are word data inside the statement.
    assert!(has_command_word(&toks(b"X=1 \";\"")));
    assert!(has_command_word(&toks(b"X=1 ';' echo x")));
    assert!(has_command_word(&toks(b"X=1 \\; echo x")));
    assert!(has_command_word(&toks(b"X=1 \";\" Y=2")));
}

/// [`collect_all`] walks to the same bound: the quoted `";"` is no
/// `NAME=value` word, so the all-assignment read fails and the statement goes
/// down the command path — where bash's `sh: ;: command not found` lives.
#[test]
fn collect_all_stops_only_at_the_real_semicolon() {
    assert_eq!(all_names(b"X=1"), Some(vec!["X".to_string()]));
    assert_eq!(
        all_names(b"X=1 Y=2"),
        Some(vec!["X".to_string(), "Y".to_string()])
    );
    assert_eq!(all_names(b"X=1 ; Y=2"), Some(vec!["X".to_string()]));
    assert_eq!(all_names(b"X=1 \";\""), None);
    assert_eq!(all_names(b"X=1 ';' Y=2"), None);
}
