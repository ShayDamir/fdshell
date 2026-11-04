#![allow(clippy::unwrap_used)]
use alloc::vec;
use alloc::vec::Vec;

use sys::ImportedStr;
use sys::ShortCStr;

use crate::state::ShellState;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

fn shell(b: &[u8]) -> ImportedStr {
    ImportedStr::shell(s(b))
}

/// Elements of `name` as the store + bridge rule answers them (bash, task #422).
fn elements(state: &ShellState, name: &[u8]) -> Vec<ShortCStr> {
    state.elements_of(&s(name))
}

fn set_scalar(state: &mut ShellState, name: &[u8], value: &[u8]) {
    state.set_var(s(name), shell(value));
}

#[test]
fn a_name_nothing_holds_has_no_elements() {
    let state = ShellState::new();
    assert_eq!(elements(&state, b"a"), Vec::<ShortCStr>::new());
    assert!(!state.holds_str_array(&s(b"a")));
}

#[test]
fn set_writes_sequential_indices() {
    let mut state = ShellState::new();
    state.set_str_array(&s(b"a"), vec![shell(b"1"), shell(b"2")]);
    assert_eq!(elements(&state, b"a"), vec![s(b"1"), s(b"2")]);
    assert!(state.holds_str_array(&s(b"a")));
}

#[test]
fn an_empty_array_stays_empty() {
    // bash: `m=(); echo "${#m[@]}"` → `0`.
    let mut state = ShellState::new();
    state.set_str_array(&s(b"m"), Vec::new());
    assert_eq!(elements(&state, b"m"), Vec::<ShortCStr>::new());
    assert!(state.holds_str_array(&s(b"m")));
}

#[test]
fn the_scalar_is_index_zero() {
    // bash: `a=hello; echo "${a[0]}"` → `hello`, `${#a[@]}` → `1`.
    let mut state = ShellState::new();
    set_scalar(&mut state, b"a", b"hello");
    assert_eq!(elements(&state, b"a"), vec![s(b"hello")]);
}

#[test]
fn an_append_to_a_scalar_lands_after_the_bridge() {
    // bash: `a=hello; a+=(x); echo "${a[@]}"` → `hello x`.
    let mut state = ShellState::new();
    set_scalar(&mut state, b"a", b"hello");
    state.append_str_array(&s(b"a"), vec![shell(b"x")]);
    assert_eq!(elements(&state, b"a"), vec![s(b"hello"), s(b"x")]);
}

#[test]
fn an_append_starts_at_the_next_index() {
    // bash: `m=(1); m+=(3); echo "${m[@]}"` → `1 3`.
    let mut state = ShellState::new();
    state.set_str_array(&s(b"m"), vec![shell(b"1")]);
    state.append_str_array(&s(b"m"), vec![shell(b"3")]);
    assert_eq!(elements(&state, b"m"), vec![s(b"1"), s(b"3")]);
}

#[test]
fn a_sparse_store_keeps_bash_order() {
    // bash: `a=(); a[5]=x` answers `${a[@]}` = `x`, `${#a[@]}` = `1`.
    let mut state = ShellState::new();
    state.set_str_array(&s(b"a"), Vec::new());
    state.set_str_element(&s(b"a"), 5, shell(b"x"));
    assert_eq!(elements(&state, b"a"), vec![s(b"x")]);
}

#[test]
fn an_element_write_makes_the_name_an_array() {
    // bash: `a[1]=x; echo "${a[1]}|${a[@]}"` → `x|x`, so the store answers both.
    let mut state = ShellState::new();
    state.set_str_element(&s(b"a"), 1, shell(b"x"));
    assert_eq!(state.str_element(&s(b"a"), 1), Some(s(b"x")).as_ref());
    assert_eq!(elements(&state, b"a"), vec![s(b"x")]);
    assert!(state.holds_str_array(&s(b"a")));
}

#[test]
fn the_store_wins_index_zero_over_the_scalar() {
    // bash: `a=1; a[0]=x; echo "${a[@]}"` → `x` — the scalar is only the
    // fallback while the store holds no index 0.
    let mut state = ShellState::new();
    set_scalar(&mut state, b"a", b"1");
    state.set_str_element(&s(b"a"), 0, shell(b"x"));
    assert_eq!(elements(&state, b"a"), vec![s(b"x")]);
    assert_eq!(state.var_value(&s(b"a")), Some(s(b"x")).as_ref());
}

#[test]
fn a_plain_write_to_a_stored_name_writes_element_zero() {
    // bash: `a=(1 2); a=scalar; echo "${a[@]}"` → `scalar 2`.
    let mut state = ShellState::new();
    state.set_str_array(&s(b"a"), vec![shell(b"1"), shell(b"2")]);
    state.set_str_element(&s(b"a"), 0, shell(b"scalar"));
    assert_eq!(elements(&state, b"a"), vec![s(b"scalar"), s(b"2")]);
    assert_eq!(state.var_value(&s(b"a")), Some(s(b"scalar")).as_ref());
}

#[test]
fn the_environ_scalar_is_the_bridge_too() {
    // bash: `export FOO=bar; echo "${FOO[@]}"` → `bar`.
    let mut state = ShellState::new();
    state.environ.push((s(b"FOO"), s(b"bar")));
    assert_eq!(elements(&state, b"FOO"), vec![s(b"bar")]);
}

#[test]
fn unset_takes_the_store_entry() {
    // bash: `a=(1 2); unset a; echo "${#a[@]}"` → `0`.
    let mut state = ShellState::new();
    state.set_str_array(&s(b"a"), vec![shell(b"1"), shell(b"2")]);
    state.unset_var(&s(b"a"));
    assert!(!state.holds_str_array(&s(b"a")));
    assert_eq!(elements(&state, b"a"), Vec::<ShortCStr>::new());
}

#[test]
fn a_name_only_in_the_store_is_a_variable_to_unset() {
    // `unset`'s no-option rule reads `is_variable` first, and the array store is
    // one of the stores `unset_var` owns — so `a=(1 2); unset a` (bash: `${#a[@]}`
    // = `0`) must not fall through to the function store.
    let mut state = ShellState::new();
    state.set_str_array(&s(b"a"), vec![shell(b"1"), shell(b"2")]);
    state.unset_name(&s(b"a"));
    assert!(!state.holds_str_array(&s(b"a")));
    assert_eq!(elements(&state, b"a"), Vec::<ShortCStr>::new());
}

#[test]
fn a_plain_name_reads_element_zero() {
    // bash: `a=(1 2); echo "$a"` → `1`; an append to a name that holds nothing
    // starts at index 0, so the store answers the plain name too.
    let mut state = ShellState::new();
    state.set_str_array(&s(b"a"), vec![shell(b"1"), shell(b"2")]);
    assert_eq!(state.var_value(&s(b"a")), Some(s(b"1")).as_ref());
    state.append_str_array(&s(b"b"), vec![shell(b"x")]);
    assert_eq!(state.var_value(&s(b"b")), Some(s(b"x")).as_ref());
}

#[test]
fn a_stored_name_without_index_zero_falls_back_to_the_scalar() {
    // bash: `a=hello; a+=(x); echo "$a"` → `hello` — the bridge's index 0 is
    // the scalar, so the plain-name lookup answers it too.
    let mut state = ShellState::new();
    set_scalar(&mut state, b"a", b"hello");
    state.append_str_array(&s(b"a"), vec![shell(b"x")]);
    assert_eq!(state.var_value(&s(b"a")), Some(s(b"hello")).as_ref());
}

#[test]
fn str_element_reads_the_store_only() {
    // No bridge here: the caller decides the out-of-range reading (task #422).
    let mut state = ShellState::new();
    set_scalar(&mut state, b"a", b"hello");
    assert_eq!(state.str_element(&s(b"a"), 0), None);
    state.set_str_element(&s(b"a"), 0, shell(b"x"));
    assert_eq!(state.str_element(&s(b"a"), 0), Some(s(b"x")).as_ref());
}
