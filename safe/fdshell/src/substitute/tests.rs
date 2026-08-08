#![allow(clippy::unwrap_used)]
use alloc::collections::VecDeque;
use alloc::format;

use hashbrown::HashMap;

use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

use super::substitute_arg;

fn dummy_cell() -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"hello"), ShortCStr::from(c"world"));
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"empty"), ShortCStr::from(c""));
    cell.borrow_mut().unwrap().strings.insert(
        ShortCStr::from(c"multi_word"),
        ShortCStr::from(c"two words"),
    );
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"var"), ShortCStr::from(c"value"));
    cell.borrow_mut().unwrap().last_bg_pid = Some(12345);
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
        .insert(ShortCStr::from(c"_my_var"), ShortCStr::from(c"underscore"));
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
        .insert(ShortCStr::from(c"hello"), ShortCStr::from(c"world"));
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
    let mut positional: alloc::collections::VecDeque<ShortCStr> =
        alloc::collections::VecDeque::new();
    for i in 0..n {
        positional.push_back(match i {
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
        });
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
    cell.borrow_mut()
        .unwrap()
        .set_positional([c"only"].into_iter().map(ShortCStr::from).collect());
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
