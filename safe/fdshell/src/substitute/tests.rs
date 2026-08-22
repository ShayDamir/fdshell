#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use alloc::{collections::VecDeque, format, vec, vec::Vec};

use hashbrown::HashMap;

use sys::ExportedFd;
use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::state::{FdVar, ShellState};

use super::substitute_arg;

fn fdvar(fd: sys::LocalFd) -> FdVar {
    FdVar {
        fd,
        trace: sys::Trace::boundary(sys::Origin::Shell),
    }
}

fn is_(value: &'static core::ffi::CStr) -> ImportedStr {
    ImportedStr::shell(value.into())
}

fn dummy_cell() -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"hello"), is_(c"world"));
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"empty"), is_(c""));
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"multi_word"), is_(c"two words"));
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"var"), is_(c"value"));
    cell.borrow_mut().unwrap().last_bg_pid = Some(sys::Pid::from_raw(12345));
    cell
}

#[test]
fn dollar_substitutes_matching_var() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$hello");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"world");
}

#[test]
fn dollar_unknown_var_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$nope");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"$nope");
}

fn env_cell() -> ForkCell<ShellState> {
    let cell = dummy_cell();
    cell.borrow_mut().unwrap().environ = Vec::new();
    cell
}

#[test]
fn dollar_resolves_inherited_env_var() {
    let cell = env_cell();
    cell.borrow_mut()
        .unwrap()
        .environ
        .push((ShortCStr::from(c"FOO"), ShortCStr::from(c"bar")));
    let arg = ShortCStr::from(c"$FOO");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"bar");
}

#[test]
fn shell_string_shadows_inherited_env_var() {
    let cell = env_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"FOO"), is_(c"shell"));
    cell.borrow_mut()
        .unwrap()
        .environ
        .push((ShortCStr::from(c"FOO"), ShortCStr::from(c"bar")));
    let arg = ShortCStr::from(c"$FOO");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"shell");
}

#[test]
fn brace_resolves_inherited_env_var() {
    let cell = env_cell();
    cell.borrow_mut()
        .unwrap()
        .environ
        .push((ShortCStr::from(c"FOO"), ShortCStr::from(c"bar")));
    let arg = ShortCStr::from(c"${FOO}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"bar");
}

#[test]
fn brace_len_of_inherited_env_var() {
    let cell = env_cell();
    cell.borrow_mut()
        .unwrap()
        .environ
        .push((ShortCStr::from(c"FOO"), ShortCStr::from(c"bar")));
    let arg = ShortCStr::from(c"${#FOO}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"3");
}

#[test]
fn brace_len_shadows_inherited_env_var() {
    let cell = env_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"FOO"), is_(c"x"));
    cell.borrow_mut()
        .unwrap()
        .environ
        .push((ShortCStr::from(c"FOO"), ShortCStr::from(c"bar")));
    let arg = ShortCStr::from(c"${#FOO}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"1");
}

#[test]
fn dollar_double_dollar_is_pid() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$$");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    let pid_str = format!("{}", cell.borrow().unwrap().shell_pid);
    assert_eq!(res.as_bytes().unwrap(), pid_str.as_bytes());
}

#[test]
fn dollar_at_end_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a$");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a$");
}

#[test]
fn dollar_in_middle_of_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"prefix.$hello/suffix");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"prefix.world/suffix");
}

#[test]
fn dollar_then_percent_handled_separately() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$%");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"$%");
}

#[test]
fn dollar_empty_value_produces_nothing() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"x$empty y");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"x y");
}

#[test]
fn dollar_multi_word_value() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"echo $multi_word");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"echo two words");
}

#[test]
fn dollar_followed_by_non_ident_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$.");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"$.");
}

#[test]
fn combined_percent_and_dollar() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$var and %var");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value and %var");
}

#[test]
fn dollar_underscore_var() {
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"_my_var"), is_(c"underscore"));
    let arg = ShortCStr::from(c"$_my_var");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"underscore");
}

#[test]
fn brace_substitutes_matching_var() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${hello}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"world");
}

#[test]
fn brace_bang_indirect_expands_target() {
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"p"), is_(c"var"));
    let arg = ShortCStr::from(c"${!p}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value");
}

#[test]
fn brace_bang_indirect_unset_name_is_literal() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${!nope}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${!nope}");
}

#[test]
fn brace_bang_indirect_unset_target_shows_target() {
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"q"), is_(c"missing"));
    let arg = ShortCStr::from(c"${!q}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${missing}");
}

#[test]
fn brace_bang_indirect_empty_target_is_empty() {
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"e"), is_(c"empty"));
    let arg = ShortCStr::from(c"${!e}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn brace_unknown_var_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${nope}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${nope}");
}

#[test]
fn brace_empty_name_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${}");
}

#[test]
fn brace_no_closing_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${hello");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${hello");
}

#[test]
fn brace_hash_no_closing_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#hello");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${#hello");
}

#[test]
fn param_dash_colon_unset_gives_word() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${nope:-d}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"d");
}

#[test]
fn param_dash_colon_set_gives_value() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${var:-d}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value");
}

#[test]
fn param_dash_colon_empty_gives_word() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${empty:-d}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"d");
}

#[test]
fn param_plus_colon_set_gives_word() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${var:+alt}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"alt");
}

#[test]
fn param_plus_colon_unset_gives_empty() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${nope:+alt}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn param_plus_colon_empty_gives_empty() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${empty:+alt}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn param_assign_colon_sets_and_reuses() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${Z:=v} [$Z]");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"v [v]");
    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get(&ShortCStr::from(c"Z"))
            .unwrap()
            .value
            .as_bytes()
            .unwrap(),
        b"v"
    );
}

#[test]
fn param_assign_colon_empty_resets_to_word() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${empty:=w}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"w");
    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get(&ShortCStr::from(c"empty"))
            .unwrap()
            .value
            .as_bytes()
            .unwrap(),
        b"w"
    );
}

#[test]
fn param_assign_colon_set_keeps_value() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${var:=other}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value");
    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get(&ShortCStr::from(c"var"))
            .unwrap()
            .value
            .as_bytes()
            .unwrap(),
        b"value"
    );
}

#[test]
fn param_question_colon_set_gives_value() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${var:?err}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value");
}

#[test]
fn param_question_colon_unset_errors_with_word() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${nope:?boom}");
    let mut cache = HashMap::new();
    let err = substitute_arg(&arg, &mut cache, &cell).unwrap_err();
    assert!(matches!(
        err.current_context(),
        crate::error::resolve::ResolveError::ParamNullOrNotSet { word, .. }
            if word.eq_bytes(b"boom")
    ));
}

#[test]
fn param_question_colon_empty_errors() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${empty:?boom}");
    let mut cache = HashMap::new();
    let err = substitute_arg(&arg, &mut cache, &cell).unwrap_err();
    assert!(matches!(
        err.current_context(),
        crate::error::resolve::ResolveError::ParamNullOrNotSet { word, .. }
            if word.eq_bytes(b"boom")
    ));
}

#[test]
fn param_question_colon_unset_uses_default_message() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${nope:?}");
    let mut cache = HashMap::new();
    let err = substitute_arg(&arg, &mut cache, &cell).unwrap_err();
    assert!(matches!(
        err.current_context(),
        crate::error::resolve::ResolveError::ParamNullOrNotSet { word, .. }
            if word.eq_bytes(b"parameter null or not set")
    ));
}

#[test]
fn param_operator_on_environ_var_counts_set() {
    let cell = env_cell();
    cell.borrow_mut().unwrap().environ =
        vec![(ShortCStr::from(c"ENVV"), ShortCStr::from(c"envval"))];
    let arg = ShortCStr::from(c"${ENVV:-d}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"envval");
}

#[test]
fn param_operator_scan_skips_non_operator_colon() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${var:x:-w}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"w");
}

#[test]
fn brace_inside_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a${hello}b");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"aworldb");
}

#[test]
fn tilde_expands_to_home() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"~");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), home.as_bytes());
}

#[test]
fn tilde_slash_expands() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"~/foo");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), format!("{}/foo", home).as_bytes());
}

#[test]
fn tilde_user_remains_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"~nobody/bar");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"~nobody/bar");
}

#[test]
fn tilde_mid_word_untouched() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a~");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a~");
}

#[test]
fn dollar_bang_returns_last_bg_pid() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$!");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"12345");
}

#[test]
fn dollar_bang_no_bg_returns_empty() {
    let s_cell = ForkCell::new(ShellState::new());
    s_cell
        .borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"hello"), is_(c"world"));
    s_cell.borrow_mut().unwrap().last_bg_pid = None;
    let arg = ShortCStr::from(c"$!");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &s_cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn dollar_bang_in_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"job=$! done");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"job=12345 done");
}

#[test]
fn brace_hash_known_var_returns_length() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#hello}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"5");
}

#[test]
fn brace_hash_empty_var_returns_zero() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#empty}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"0");
}

#[test]
fn brace_hash_unknown_var_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#nope}");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${#nope}");
}

#[test]
fn brace_hash_in_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"len=${#hello} end");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"len=5 end");
}

fn positional_cell() -> ForkCell<ShellState> {
    positional_cell_with_n(3)
}

fn positional_cell_with_n(n: usize) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    let mut positional: alloc::collections::VecDeque<ImportedStr> =
        alloc::collections::VecDeque::new();
    for i in 0..n {
        positional.push_back(ImportedStr::shell(match i {
            0 => ShortCStr::from(c"arg0"),
            1 => ShortCStr::from(c"arg1"),
            2 => ShortCStr::from(c"arg2"),
            3 => ShortCStr::from(c"arg3"),
            4 => ShortCStr::from(c"arg4"),
            5 => ShortCStr::from(c"arg5"),
            6 => ShortCStr::from(c"arg6"),
            7 => ShortCStr::from(c"arg7"),
            8 => ShortCStr::from(c"arg8"),
            9 => ShortCStr::from(c"arg9"),
            10 => ShortCStr::from(c"arg10"),
            11 => ShortCStr::from(c"arg11"),
            12 => ShortCStr::from(c"arg12"),
            13 => ShortCStr::from(c"arg13"),
            14 => ShortCStr::from(c"arg14"),
            15 => ShortCStr::from(c"arg15"),
            16 => ShortCStr::from(c"arg16"),
            17 => ShortCStr::from(c"arg17"),
            18 => ShortCStr::from(c"arg18"),
            19 => ShortCStr::from(c"arg19"),
            _ => ShortCStr::from(c"argx"),
        }));
    }
    cell.borrow_mut().unwrap().set_positional(positional);
    cell
}

#[test]
fn dollar_hash_returns_positional_count() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$#");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"3");
}

#[test]
fn dollar_hash_empty_positional() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(VecDeque::new());
    let arg = ShortCStr::from(c"$#");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"0");
}

#[test]
fn dollar_at_expands_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$@");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg0 arg1 arg2");
}

#[test]
fn dollar_star_expands_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$*");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg0 arg1 arg2");
}

#[test]
fn dollar_at_single_positional() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"only"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let arg = ShortCStr::from(c"$@");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"only");
}

#[test]
fn dollar_at_empty_positional() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(VecDeque::new());
    let arg = ShortCStr::from(c"$@");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn dollar_zero_is_first_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$0");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg0");
}

#[test]
fn dollar_one_is_second_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$1");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg1");
}

#[test]
fn dollar_n_is_third_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$2");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg2");
}

#[test]
fn dollar_n_out_of_range_is_empty() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$9");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn dollar_positional_in_text() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$0-$1-$2");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg0-arg1-arg2");
}

#[test]
fn dollar_multi_digit_index() {
    let cell = positional_cell_with_n(15);
    let arg = ShortCStr::from(c"$10");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg10");
}

#[test]
fn dollar_multi_digit_index_in_text() {
    let cell = positional_cell_with_n(20);
    let arg = ShortCStr::from(c"$1-$10-$19");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg1-arg10-arg19");
}

#[test]
fn percent_double_percent_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a%%b");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a%b");
}

#[test]
fn percent_single_percent_unknown_fd_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"%unknown_fd");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"%unknown_fd");
}

#[test]
fn percent_fd_cache_hit_returns_same_value() {
    let cell = ForkCell::new(ShellState::new());
    // Open /dev/null to get a real FD (should be fd 3 after stdin/stdout/stderr)
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .fds
        .insert(ShortCStr::from(c"testfd"), fdvar(dev_null));

    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();

    // First call — should lookup FD and cache it
    let arg1 = ShortCStr::from(c"%testfd");
    let res1 = substitute_arg(&arg1, &mut cache, &cell).unwrap();

    // Second call — should hit cache and return same value
    let arg2 = ShortCStr::from(c"%testfd");
    let res2 = substitute_arg(&arg2, &mut cache, &cell).unwrap();
    assert_eq!(res1.as_bytes().unwrap(), res2.as_bytes().unwrap());

    // Third call with surrounding text — still hits cache
    let arg3 = ShortCStr::from(c"prefix-%testfd-suffix");
    let res3 = substitute_arg(&arg3, &mut cache, &cell).unwrap();
    let fd_str = alloc::string::String::from_utf8_lossy(res1.as_bytes().unwrap());
    let expected = format!("prefix-{fd_str}-suffix");
    assert_eq!(res3.as_bytes().unwrap(), expected.as_bytes());

    // Verify cache contains the exported FD
    assert!(cache.contains_key(&ShortCStr::from(c"testfd")));
}

#[test]
fn percent_leading_underscore_name_resolves() {
    let cell = ForkCell::new(ShellState::new());
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .fds
        .insert(ShortCStr::from(c"_fd"), fdvar(dev_null));
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let arg = ShortCStr::from(c"%_fd");
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert!(res.as_bytes().unwrap().iter().all(|b| b.is_ascii_digit()));
}

#[test]
fn percent_hyphen_after_percent_is_literal() {
    let cell = ForkCell::new(ShellState::new());
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .fds
        .insert(ShortCStr::from(c"-testfd"), fdvar(dev_null));
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let arg = ShortCStr::from(c"%-testfd");
    let res = substitute_arg(&arg, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"%-testfd");
}

// Mutant-catching tests for substitute/mod.rs (MISSED 25-31)
#[test]
fn dollar_at_fq_true_expands_separate_args() {
    // Regression: quoted "$@" must expand to separate arguments (not joined)
    // With fq=true and "$@": correct → one word per positional (N elements)
    // Bug (before fix): fq was always false, so "$@" joined args into 1 element
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"arg0", c"arg1", c"arg2"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_fq = alloc::vec![true];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].as_bytes().unwrap(), b"arg0");
    assert_eq!(result[1].as_bytes().unwrap(), b"arg1");
    assert_eq!(result[2].as_bytes().unwrap(), b"arg2");
}

#[test]
fn dollar_at_unquoted_splits_on_ifs() {
    // Unquoted $@: each positional is word-split on IFS separately.
    // "a b" splits into "a" "b", "c" stays → 3 fields.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"a b", c"c"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_fq = alloc::vec![false];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert_eq!(result[1].as_bytes().unwrap(), b"b");
    assert_eq!(result[2].as_bytes().unwrap(), b"c");
}

#[test]
fn dollar_star_fq_true_joins_positional() {
    // Quoted "$*": one word joined by the first IFS byte (space by default).
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"arg0", c"arg1"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let args = alloc::vec![ShortCStr::from(c"$*")];
    let args_fq = alloc::vec![true];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"arg0 arg1");
}

#[test]
fn dollar_at_unquoted_custom_ifs_splits_per_positional() {
    // IFS=":": each positional is split on ":" separately; an injected space
    // join would leave "a b" intact and fail this test.
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = c":".into();
        state.set_positional(
            [c"a:b", c"c d"]
                .into_iter()
                .map(ShortCStr::from)
                .map(ImportedStr::shell)
                .collect(),
        );
    }
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_fq = alloc::vec![false];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert_eq!(result[1].as_bytes().unwrap(), b"b");
    assert_eq!(result[2].as_bytes().unwrap(), b"c d");
}

#[test]
fn dollar_at_unquoted_empty_ifs_keeps_positionals_separate() {
    // Empty IFS disables splitting; positionals must not be joined into one word.
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = ShortCStr::new();
        state.set_positional(
            [c"a b", c"c"]
                .into_iter()
                .map(ShortCStr::from)
                .map(ImportedStr::shell)
                .collect(),
        );
    }
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_fq = alloc::vec![false];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].as_bytes().unwrap(), b"a b");
    assert_eq!(result[1].as_bytes().unwrap(), b"c");
}

#[test]
fn dollar_star_unquoted_custom_ifs_splits_per_positional() {
    // Unquoted $* behaves like $@: per-positional IFS splitting.
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = c":".into();
        state.set_positional(
            [c"a:b"]
                .into_iter()
                .map(ShortCStr::from)
                .map(ImportedStr::shell)
                .collect(),
        );
    }
    let args = alloc::vec![ShortCStr::from(c"$*")];
    let args_fq = alloc::vec![false];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert_eq!(result[1].as_bytes().unwrap(), b"b");
}

#[test]
fn dollar_star_quoted_custom_ifs_joins_with_first_ifs_byte() {
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = c":".into();
        state.set_positional(
            [c"a", c"b"]
                .into_iter()
                .map(ShortCStr::from)
                .map(ImportedStr::shell)
                .collect(),
        );
    }
    let args = alloc::vec![ShortCStr::from(c"$*")];
    let args_fq = alloc::vec![true];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"a:b");
}

#[test]
fn dollar_star_quoted_empty_ifs_joins_with_nothing() {
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = ShortCStr::new();
        state.set_positional(
            [c"a", c"b"]
                .into_iter()
                .map(ShortCStr::from)
                .map(ImportedStr::shell)
                .collect(),
        );
    }
    let args = alloc::vec![ShortCStr::from(c"$*")];
    let args_fq = alloc::vec![true];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"ab");
}

#[test]
fn unquoted_var_with_spaces_splits_on_ifs() {
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$multi_word")];
    let args_fq = alloc::vec![false];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].as_bytes().unwrap(), b"two");
    assert_eq!(result[1].as_bytes().unwrap(), b"words");
}

#[test]
fn quoted_var_with_spaces_does_not_split() {
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$multi_word")];
    let args_fq = alloc::vec![true];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"two words");
}

#[test]
fn literal_arg_with_fq_true_not_routed_to_positional() {
    // A || mutant would route non-$@/$* FQ args into expand_positional_word
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"arg0", c"arg1"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let args = alloc::vec![ShortCStr::from(c"hello")];
    let args_fq = alloc::vec![true];
    let result = super::substitute_args(&args, &args_fq, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"hello");
}

#[test]
fn positional_join_has_spaces_between() {
    // Default IFS: separator is its first byte, a space.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"a", c"b", c"c"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let state = cell.borrow().unwrap();
    let result = super::positional::positional_join(&state.positional, &state.ifs).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"a b c");
}

#[test]
fn expand_positional_word_quoted_at_pushes_all_positional() {
    // Quoted "$@": one word per positional, unsplit.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"arg0", c"arg1", c"arg2"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let mut result = Vec::new();
    super::positional::expand_positional_word(false, true, &cell, &mut result).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].as_bytes().unwrap(), b"arg0");
    assert_eq!(result[1].as_bytes().unwrap(), b"arg1");
    assert_eq!(result[2].as_bytes().unwrap(), b"arg2");
}

#[test]
fn positional_join_single_element_no_space() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"only"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let state = cell.borrow().unwrap();
    let result = super::positional::positional_join(&state.positional, &state.ifs).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"only");
}

// Mutant-catching tests for substitute/paren.rs
#[test]
fn paren_single_level_expr() {
    let mut peek = b"echo hi)".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"echo hi".as_slice());
}

#[test]
fn paren_nested_expr_keeps_all_levels() {
    // depth += 1 mutant (-=, *=) truncates or corrupts nested captures.
    // Input mirrors what arg.rs passes: outer `$(` already consumed, so the
    // byte sequence ends with the `)` closing that outer level.
    let input = b"(a(b(c))d))";
    let mut peek = input.iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"(a(b(c))d)");
}

#[test]
fn paren_empty_inner() {
    let mut peek = b")".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert!(res.is_empty());
}

#[test]
fn paren_unclosed_is_error() {
    let mut peek = b"abc".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek);
    assert!(matches!(
        res.unwrap_err().current_context(),
        crate::error::resolve::ResolveError::UnclosedParen
    ));
}

#[test]
fn paren_paren_in_quotes_is_data() {
    let mut peek = b"echo \"a)b\")".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"echo \"a)b\"".as_slice());
}

#[test]
fn paren_open_paren_in_quotes_is_data() {
    let mut peek = b"a\"(b)c\")".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"a\"(b)c\"".as_slice());
}

#[test]
fn paren_escaped_quote_in_quotes_is_data() {
    let mut peek = b"echo \"a\\\"b)\")".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"echo \"a\\\"b)\"".as_slice());
}

#[test]
fn paren_unclosed_quote_is_error() {
    let mut peek = b"echo \"a".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek);
    assert!(matches!(
        res.unwrap_err().current_context(),
        crate::error::resolve::ResolveError::UnclosedParen
    ));
}

#[test]
fn paren_trailing_escape_in_quotes_is_error() {
    let mut peek = b"echo \"a\\".iter().copied().peekable();
    let res = super::paren::read_paren_expr(&mut peek);
    assert!(matches!(
        res.unwrap_err().current_context(),
        crate::error::resolve::ResolveError::UnclosedParen
    ));
}
