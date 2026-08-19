#![allow(clippy::unwrap_used)]
use super::assign_origin;
use crate::state::ShellState;
use alloc::collections::VecDeque;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, Position, ShortCStr, Trace};

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn var(origin: Origin) -> ImportedStr {
    ImportedStr::new(
        ShortCStr::from(c"v"),
        Trace::at(Position::new(1, 1), origin),
    )
}

fn from(v: &str) -> ShortCStr {
    ShortCStr::from_vec(v.as_bytes().to_vec()).unwrap_or_default()
}

#[test]
fn literal_keeps_line_origin() {
    let cell = cell();
    let origin = assign_origin(&from("hello"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::Stdin);
}

#[test]
fn lone_var_is_transitive() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .strings
            .insert(from("src"), var(Origin::CliArgument(2)));
    }
    let origin = assign_origin(&from("$src"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::CliArgument(2));
}

#[test]
fn braced_var_is_transitive() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .strings
            .insert(from("src"), var(Origin::File(from("s.sh"))));
    }
    let origin = assign_origin(&from("${src}"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::File(from("s.sh")));
}

#[test]
fn unset_var_keeps_line_origin() {
    let cell = cell();
    let origin = assign_origin(&from("$missing"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::Stdin);
}

#[test]
fn positional_is_transitive() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        let mut positional = VecDeque::new();
        positional.push_back(ImportedStr::shell(ShortCStr::from(c"sh")));
        positional.push_back(var(Origin::CliArgument(3)));
        state.positional = positional;
    }
    let origin = assign_origin(&from("$1"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::CliArgument(3));
}

#[test]
fn status_is_shell() {
    let cell = cell();
    let origin = assign_origin(&from("$?"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::Shell);
}

#[test]
fn tilde_is_home_env_var() {
    let cell = cell();
    let origin = assign_origin(&from("~/x"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::EnvVar(ShortCStr::from(c"HOME")));
}

#[test]
fn dollar_paren_is_command_output() {
    let cell = cell();
    let origin = assign_origin(&from("$(echo hi)"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::CommandOutput);
}

#[test]
fn backtick_is_command_output() {
    let cell = cell();
    let origin = assign_origin(&from("`echo hi`"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::CommandOutput);
}

#[test]
fn var_in_longer_word_keeps_line_origin() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.strings.insert(from("src"), var(Origin::Stdin));
    }
    let origin = assign_origin(&from("x$src"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::Stdin);
}

#[test]
fn braced_single_char_is_transitive() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.strings.insert(from("a"), var(Origin::CliArgument(2)));
    }
    let origin = assign_origin(&from("${a}"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::CliArgument(2));
}

#[test]
fn unterminated_brace_keeps_line_origin() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.strings.insert(from("a"), var(Origin::CliArgument(2)));
    }
    let origin = assign_origin(&from("${ab"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::Stdin);
}

#[test]
fn underscore_in_name_is_transitive() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .strings
            .insert(from("a_b"), var(Origin::CliArgument(2)));
    }
    let origin = assign_origin(&from("$a_b"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::CliArgument(2));
}

#[test]
fn trailing_non_digit_keeps_line_origin() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        let mut positional = VecDeque::new();
        positional.push_back(ImportedStr::shell(ShortCStr::from(c"sh")));
        positional.push_back(var(Origin::CliArgument(3)));
        state.positional = positional;
    }
    let origin = assign_origin(&from("$1abc"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::Stdin);
}

#[test]
fn second_positional_uses_index_value() {
    let cell = cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        let mut positional = VecDeque::new();
        positional.push_back(ImportedStr::shell(ShortCStr::from(c"sh")));
        positional.push_back(var(Origin::CliArgument(3)));
        positional.push_back(var(Origin::CliArgument(4)));
        state.positional = positional;
    }
    let origin = assign_origin(&from("$2"), Origin::Stdin, &cell).unwrap();
    assert_eq!(origin, Origin::CliArgument(4));
}
