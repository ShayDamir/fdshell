#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use sys::ShortCStr;

use crate::state::ShellState;

use super::substitute_arg;

fn dummy_state() -> ShellState {
    let mut s = ShellState::new();
    s.strings
        .insert(ShortCStr::from(c"hello"), ShortCStr::from(c"world"));
    s.strings
        .insert(ShortCStr::from(c"empty"), ShortCStr::from(c""));
    s.strings.insert(
        ShortCStr::from(c"multi_word"),
        ShortCStr::from(c"two words"),
    );
    s.strings
        .insert(ShortCStr::from(c"var"), ShortCStr::from(c"value"));
    s
}

#[test]
fn dollar_substitutes_matching_var() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"$hello");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"world");
}

#[test]
fn dollar_unknown_var_is_literal() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"$nope");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"$nope");
}

#[test]
fn dollar_double_dollar_is_literal() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"$$");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"$");
}

#[test]
fn dollar_at_end_is_literal() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"a$");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"a$");
}

#[test]
fn dollar_in_middle_of_text() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"prefix.$hello/suffix");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"prefix.world/suffix");
}

#[test]
fn dollar_then_percent_handled_separately() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"$%");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"$%");
}

#[test]
fn dollar_empty_value_produces_nothing() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"x$empty y");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"x y");
}

#[test]
fn dollar_multi_word_value() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"echo $multi_word");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"echo two words");
}

#[test]
fn dollar_followed_by_non_ident_is_literal() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"$.");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"$.");
}

#[test]
fn combined_percent_and_dollar() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"$var and %var");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"value and %var");
}

#[test]
fn dollar_underscore_var() {
    let mut state = dummy_state();
    state
        .strings
        .insert(ShortCStr::from(c"_my_var"), ShortCStr::from(c"underscore"));
    let arg = ShortCStr::from(c"$_my_var");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"underscore");
}

#[test]
fn brace_substitutes_matching_var() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"${hello}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"world");
}

#[test]
fn brace_unknown_var_is_literal() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"${nope}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"${nope}");
}

#[test]
fn brace_empty_name_is_literal() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"${}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"${}");
}

#[test]
fn brace_no_closing_is_literal() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"${hello");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"${hello");
}

#[test]
fn brace_inside_text() {
    let state = dummy_state();
    let arg = ShortCStr::from(c"a${hello}b");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &state).unwrap();
    assert_eq!(res.as_bytes(), b"aworldb");
}
