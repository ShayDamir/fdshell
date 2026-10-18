#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use alloc::{collections::VecDeque, format, vec, vec::Vec};

use hashbrown::HashMap;

use sys::ExportedFd;
use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::{FdVar, ShellState};

use super::arg::substitute_arg_marked;
use super::mask::Counting;
use super::origin::Origin;
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"world");
}

#[test]
fn dollar_unknown_var_is_empty() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$nope");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn nounset_unbound_var_errors() {
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"$nope");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { .. }
    ));
}

#[test]
fn nounset_bound_var_is_unaffected() {
    let cell = dummy_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"$hello");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"world");
}

#[test]
fn nounset_out_of_range_positional_errors() {
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"$1");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var }
            if var.as_bytes().unwrap() == b"1".as_slice()
    ));
}

#[test]
fn nounset_brace_var_errors() {
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"${nope}");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"nope"
    ));
}

#[test]
fn nounset_brace_length_errors() {
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"${#nope}");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"nope"
    ));
}

#[test]
fn nounset_empty_length_name_is_zero() {
    // `${#}` is the length of the empty name, so there is no parameter to be
    // unbound: bash prints `0` at rc 0 even under `set -u` (brace.rs guards it
    // before the nounset bail).
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"${#}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"0");
}

#[test]
fn nounset_indirect_target_unbound_errors() {
    // The message carries the whole `${!q}` body, as bash's `!q: unbound variable`.
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"q"), is_(c"missing"));
    let arg = ShortCStr::from(c"${!q}");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"!q"
    ));
}

#[test]
fn indirect_empty_name_errors() {
    // A name bound to the empty string has no target: bash reports
    // `: invalid variable name`, fdshell the same rc with its own message.
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"p"), is_(c""));
    let arg = ShortCStr::from(c"${!p}");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::InvalidIndirect { var } if var.is_empty()
    ));
}

#[test]
fn percent_question_substitutes_bound_fd() {
    let cell = dummy_cell();
    let (rd, _wr) = sys::pipe::pipe2(0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .set_fd_var(ShortCStr::from(c"?"), fdvar(rd));
    let arg = ShortCStr::from(c"%?");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    // A bound `?` substitutes to its fd number, not the literal `%?` or `?`.
    let bytes = res.as_bytes().unwrap();
    assert!(!bytes.is_empty());
    assert_ne!(bytes, b"?");
    assert_ne!(bytes, b"%?");
}

#[test]
fn percent_question_unbound_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"%?");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"%?");
}

#[test]
fn percent_named_fd_unbound_is_literal() {
    // The named form keeps its literal text when the fd is unbound: the fd
    // namespace is a fdshell extension, not a POSIX parameter (README).
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"%nosuchfd");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"%nosuchfd");
}

fn env_cell() -> ForkCell<ShellState> {
    let cell = dummy_cell();
    cell.borrow_mut().unwrap().environ = Vec::new();
    cell
}

/// The arm as the pipeline runs it: the expanded text, its mask, and the
/// splitting signal. The signal is the provenance list of task #222 — the
/// `expanded` bit of task #133 is superseded by it: the helper reports
/// `true` exactly when some byte of the text came from an expansion (the
/// tilde arm deliberately records none).
fn expand_marked(
    cell: &ForkCell<ShellState>,
    word: &ShortCStr,
    mask: &[bool],
) -> (ShortCStr, Vec<bool>, bool) {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    expand_marked_cached(cell, &mut cache, word, mask)
}

/// The same helper with a seeded fd-substitution cache (a `%name` pin needs
/// exact fd numbers).
fn expand_marked_cached(
    cell: &ForkCell<ShellState>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    word: &ShortCStr,
    mask: &[bool],
) -> (ShortCStr, Vec<bool>, bool) {
    let (text, out_mask, origin) = substitute_arg_marked(word, mask, cache, cell).unwrap();
    (text, out_mask, !origin.is_empty())
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"1");
}

#[test]
fn dollar_double_dollar_is_pid() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$$");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    let pid_str = format!("{}", cell.borrow().unwrap().shell_pid);
    assert_eq!(res.as_bytes().unwrap(), pid_str.as_bytes());
}

#[test]
fn dollar_at_end_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a$");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a$");
}

#[test]
fn dollar_in_middle_of_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"prefix.$hello/suffix");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"prefix.world/suffix");
}

#[test]
fn dollar_then_percent_handled_separately() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$%");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"$%");
}

#[test]
fn dollar_empty_value_produces_nothing() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"x$empty y");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"x y");
}

#[test]
fn dollar_multi_word_value() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"echo $multi_word");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"echo two words");
}

#[test]
fn dollar_followed_by_non_ident_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$.");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"$.");
}

#[test]
fn dollar_dash_expands_active_option_flags() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    // `expand_aliases` is on by default but has no short flag, so `$-` is empty.
    let (res, _) = substitute_arg(&ShortCStr::from(c"x$-y"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"xy");
    cell.borrow_mut().unwrap().options |= crate::options::NOCLOBBER;
    let (res, _) = substitute_arg(&ShortCStr::from(c"$-"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"C");
}

#[test]
fn backslash_dollar_is_literal_unexpanded() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&ShortCStr::from(c"a\\$_b"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a$_b");
    let (res, _) = substitute_arg(&ShortCStr::from(c"\\$hello"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"$hello");
}

#[test]
fn backslash_backslash_folds_to_single() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&ShortCStr::from(c"a\\\\b"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a\\b");
    // `\\` before a live `$var` keeps the variable expanded (bash).
    let (res, _) = substitute_arg(&ShortCStr::from(c"\\\\$hello"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"\\world");
}

#[test]
fn backslash_other_char_kept() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    // Quoted (mask all-`true`): POSIX #4.2 keeps the backslash of `\<char>`.
    let quoted = [true; 4];
    let (res, _) = substitute_arg(&ShortCStr::from(c"a\\nb"), &quoted, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a\\nb");
}

#[test]
fn unquoted_escape_pair_drops_the_backslash() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    // POSIX #4.1: the backslash is removed and the escaped byte is protected
    // (mask `true`), so `a\*` is a literal word, not a pattern.
    let (res, mask) = substitute_arg(&ShortCStr::from(c"a\\*b"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a*b");
    assert_eq!(mask, vec![false, true, false]);
}

#[test]
fn unquoted_escaped_space_is_one_word() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    let (res, mask) = substitute_arg(&ShortCStr::from(c"a\\ b"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a b");
    // The escaped space is protected, so IFS splitting never breaks the word.
    assert_eq!(mask, vec![false, true, false]);
}

#[test]
fn unquoted_escaped_newline_is_a_line_continuation() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    // Both bytes of `\<newline>` are dropped.
    let (res, mask) = substitute_arg(&ShortCStr::from(c"a\\\nb"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"ab");
    assert_eq!(mask, vec![false, false]);
}

#[test]
fn trailing_unquoted_backslash_is_kept() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    let (res, mask) = substitute_arg(&ShortCStr::from(c"a\\"), &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a\\");
    assert_eq!(mask, vec![false, false]);
}

#[test]
fn unquoted_pair_leaves_the_mask_aligned_for_the_next_byte() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    // Word text `a\*bc`: only the trailing `c` was inside double quotes. The
    // unquoted pair must consume exactly two mask bytes, so `c` reads its own
    // bit (the escaped-byte arm's `idx` bump).
    let (res, out_mask) = substitute_arg(
        &ShortCStr::from(c"a\\*bc"),
        &[false, false, false, false, true],
        &mut cache,
        &cell,
    )
    .unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a*bc");
    assert_eq!(out_mask, vec![false, true, false, true]);
}

#[test]
fn line_continuation_leaves_the_mask_aligned_for_the_next_byte() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    // Word text `a\<newline>b`: the `\<newline>` pair drops both bytes, and
    // the `b` that follows was quoted, so its own mask bit must be read.
    let (res, out_mask) = substitute_arg(
        &ShortCStr::from(c"a\\\nb"),
        &[false, false, false, true],
        &mut cache,
        &cell,
    )
    .unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"ab");
    assert_eq!(out_mask, vec![false, true]);
}

#[test]
fn combined_percent_and_dollar() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$var and %var");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"underscore");
}

#[test]
fn brace_substitutes_matching_var() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${hello}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value");
}

#[test]
fn brace_bang_indirect_unset_name_errors() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${!nope}");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::InvalidIndirect { var } if var.as_bytes().unwrap() == b"nope"
    ));
}

#[test]
fn brace_bang_indirect_unset_target_is_empty() {
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"q"), is_(c"missing"));
    let arg = ShortCStr::from(c"${!q}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn brace_bang_empty_name_is_literal() {
    // `${!}` has an empty indirect name: the same literal case as `${}`, and
    // rc 0 (bash expands it to empty, a documented divergence).
    let cell = env_cell();
    let arg = ShortCStr::from(c"${!}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${!}");
}

#[test]
fn brace_unknown_var_is_empty() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${nope}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn brace_empty_name_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${}");
}

#[test]
fn brace_no_closing_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${hello");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${hello");
}

#[test]
fn brace_hash_no_closing_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#hello");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"${#hello");
}

#[test]
fn param_dash_colon_unset_gives_word() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${nope:-d}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"d");
}

#[test]
fn param_dash_colon_set_gives_value() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${var:-d}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value");
}

#[test]
fn param_dash_colon_empty_gives_word() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${empty:-d}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"d");
}

#[test]
fn param_plus_colon_set_gives_word() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${var:+alt}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"alt");
}

#[test]
fn param_plus_colon_unset_gives_empty() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${nope:+alt}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn param_plus_colon_empty_gives_empty() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${empty:+alt}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn param_assign_colon_sets_and_reuses() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${Z:=v} [$Z]");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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

/// The bytes of a shell string variable: an assertion about where an
/// assignment landed (an empty expansion asserts nothing about the state).
fn var_of(state: &ShellState, name: &'static core::ffi::CStr) -> Vec<u8> {
    state
        .strings
        .get(&ShortCStr::from(name))
        .map(|v| v.value.as_bytes().unwrap().to_vec())
        .unwrap_or_default()
}

#[test]
fn colon_operators_resolve_indirect_names() {
    // `${!ind…}` reads the target `ind` names, the same rule the pattern family
    // uses (task #176): `:-` and `:?` take the target's value, `:+` the word,
    // and `:=` finds the target non-empty so it writes nothing.
    let cell = env_cell();
    bind(&cell, c"ind", c"v");
    bind(&cell, c"v", c"abcabc");
    let m = &[false; 12];
    assert_eq!(expand(&cell, c"${!ind:-z}", m), b"abcabc");
    assert_eq!(expand(&cell, c"${!ind:+z}", m), b"z");
    assert_eq!(expand(&cell, c"${!ind:?m}", m), b"abcabc");
    assert_eq!(expand(&cell, c"${!ind:=q}", m), b"abcabc");
    assert_eq!(expand(&cell, c"$v", m), b"abcabc");
}

#[test]
fn colon_assign_writes_the_indirect_target() {
    // The write goes to the parameter the reference points at: `x` gets the
    // word, no variable named `!ind` is ever made, and `ind` keeps its own
    // value. `set -u` keeps the assignment at rc 0 (the family's word supplies).
    let cell = env_cell();
    bind(&cell, c"ind", c"x");
    assert_eq!(expand(&cell, c"${!ind:=q}", &[false; 11]), b"q");
    {
        let state = cell.borrow().unwrap();
        assert_eq!(var_of(&state, c"x"), b"q");
        assert_eq!(var_of(&state, c"ind"), b"x");
        assert!(state.strings.get(&ShortCStr::from(c"!ind")).is_none());
    }
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    bind(&cell, c"ind", c"y");
    assert_eq!(expand(&cell, c"${!ind:=w}", &[false; 11]), b"w");
    assert_eq!(var_of(&cell.borrow().unwrap(), c"y"), b"w");
}

#[test]
fn colon_arms_stay_nounset_exempt_with_indirect_names() {
    // The target is unset: the family's word supplies the value, so `set -u`
    // never bails (`nounset_bails_a_pattern_parameter` is the pattern family).
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    bind(&cell, c"v", c"undef_target");
    let m = &[false; 12];
    assert_eq!(expand(&cell, c"${!v:-z}", m), b"z");
    assert_eq!(expand(&cell, c"${!v:+z}", m), b"");
    assert_eq!(expand(&cell, c"${!v:=q}", m), b"q");
    assert_eq!(var_of(&cell.borrow().unwrap(), c"undef_target"), b"q");
    // No colon arm reports `UnboundVariable` under `set -u` (bash: every colon
    // operator carries its own word); `:?` keeps its own bail.
    for arg in [
        c"${undef:-z}",
        c"${undef:+z}",
        c"${undef:=q}",
        c"${undef:?m}",
    ] {
        if let Err(e) = substitute_arg(&ShortCStr::from(arg), m, &mut HashMap::new(), &cell) {
            assert!(
                !matches!(e.current_context(), ResolveError::UnboundVariable { .. }),
                "{arg:?}"
            );
        }
    }
}

#[test]
fn colon_arms_bail_on_an_unresolvable_indirect_reference() {
    // A name that is unbound, or bound to the empty string, names no target:
    // the word does not rescue it, so every colon arm takes the same
    // `InvalidIndirect` bail as the pattern family.
    let cell = env_cell();
    let m = &[false; 12];
    let cases: [(&'static core::ffi::CStr, &[u8]); 8] = [
        (c"${!nope:-z}", b"nope"),
        (c"${!nope:+z}", b"nope"),
        (c"${!nope:?m}", b"nope"),
        (c"${!nope:=q}", b"nope"),
        (c"${!empty:-z}", b""),
        (c"${!empty:+z}", b""),
        (c"${!empty:?m}", b""),
        (c"${!empty:=q}", b""),
    ];
    for (arg, want) in cases {
        let e = substitute_arg(&ShortCStr::from(arg), m, &mut HashMap::new(), &cell).unwrap_err();
        assert!(
            matches!(
                e.current_context(),
                ResolveError::InvalidIndirect { var } if var.as_bytes().unwrap() == want
            ),
            "{arg:?}"
        );
    }
    // The two `:=` bails left no write behind.
    assert!(
        cell.borrow()
            .unwrap()
            .strings
            .get(&ShortCStr::from(c"nope"))
            .is_none()
    );
    assert!(
        cell.borrow()
            .unwrap()
            .strings
            .get(&ShortCStr::from(c"empty"))
            .is_some_and(|v| v.value.is_empty())
    );
    // The write-target lookup itself: a plain name writes itself, `!name`
    // writes the target its value names, the degenerate `!` writes the body it
    // was scanned as, and an unresolvable reference bails with no write.
    bind(&cell, c"ind", c"var");
    let state = cell.borrow().unwrap();
    let targets: [(&'static core::ffi::CStr, &[u8]); 3] =
        [(c"var", b"var"), (c"!ind", b"var"), (c"!", b"!")];
    for (name, want) in targets {
        assert_eq!(
            state
                .colon_target(&name.into())
                .unwrap()
                .as_bytes()
                .unwrap(),
            want,
            "{name:?}"
        );
    }
    assert!(matches!(
        state
            .colon_target(&c"!nope".into())
            .unwrap_err()
            .current_context(),
        ResolveError::InvalidIndirect { .. }
    ));
}

#[test]
fn colon_arms_treat_the_empty_indirect_name_as_unset() {
    // The scan name is exactly `!` (`${!:-z}`): the degenerate guard runs ahead
    // of the `!` dispatch, so the reference never reaches the `InvalidIndirect`
    // bail — bash gives `z` for `:-` and nothing for `:+`, rc 0 with `set -u`
    // too (the `?` and `=` forms are integration pins).
    let cell = env_cell();
    let m = &[false; 8];
    assert_eq!(expand(&cell, c"${!:-z}", m), b"z");
    assert_eq!(expand(&cell, c"${!:+z}", m), b"");
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    assert_eq!(expand(&cell, c"${!:-z}", m), b"z");
    assert_eq!(expand(&cell, c"${!:+z}", m), b"");
}

#[test]
fn param_question_colon_set_gives_value() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${var:?err}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"value");
}

#[test]
fn param_question_colon_unset_errors_with_word() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${nope:?boom}");
    let mut cache = HashMap::new();
    let err = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
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
    let err = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
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
    let err = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"envval");
}

#[test]
fn param_operator_scan_skips_non_operator_colon() {
    let cell = env_cell();
    let arg = ShortCStr::from(c"${var:x:-w}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"w");
}

#[test]
fn brace_inside_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a${hello}b");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"aworldb");
}

#[test]
fn tilde_expands_to_home() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"~");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), home.as_bytes());
}

#[test]
fn tilde_slash_expands() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"~/foo");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), format!("{}/foo", home).as_bytes());
}

#[test]
fn tilde_user_remains_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"~nobody/bar");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"~nobody/bar");
}

#[test]
fn tilde_mid_word_untouched() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a~");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a~");
}

// Tilde expansion must keep the quote mask aligned: the expansion inherits
// the tilde's bit, and later original bytes keep their own bits even though
// the expansion consumed input from the shared peekable. The tilde's own bit
// is `false`, because a leading `~` whose prefix is quoted never reaches the
// arm at all (the two tests below). The shape is constrained by task #281:
// a **quoted `/` in the prefix** makes the word a literal-tilde row, so an
// alignment test must quote a byte *after* the slash (`~/"a"`, not `~"/a"`),
// or it silently tests the gate instead of the alignment.
#[test]
fn tilde_expansion_keeps_mask_alignment() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"~/a");
    let mut cache = HashMap::new();
    let (res, mask_out) = substitute_arg(&arg, &[false, false, true], &mut cache, &cell).unwrap();
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), format!("{}/a", home).as_bytes());
    assert!(mask_out.iter().take(home.len()).all(|&q| !q));
    assert!(
        !mask_out.get(home.len()).copied().unwrap_or(true),
        "the `/` follows the expansion unquoted, so it keeps a `false` bit"
    );
    assert!(
        mask_out.get(home.len() + 1).copied().unwrap_or(false),
        "the byte after it must keep its own (quoted) bit"
    );
}

// A quoted `~` is a literal byte: POSIX removes the quotes after tilde
// expansion, and bash keeps `printf "[%s]" "~"` → `[~]` and
// `printf "[%s]" "~/x"` → `[~/x]`. The mask's first bit is the part of the
// gate #269 added, so the arm must not run at all — the word records no
// splittable byte. Task #281 widens the gate to the whole tilde-prefix.
#[test]
fn tilde_quoted_first_bit_is_literal() {
    let cell = dummy_cell();
    let (res, mask, expanded) = expand_marked(&cell, &ShortCStr::from(c"~"), &[true]);
    assert_eq!(res.as_bytes().unwrap(), b"~");
    assert_eq!(mask, vec![true]);
    assert!(!expanded, "a quoted `~` runs no expansion arm");
    let (res, mask, expanded) = expand_marked(&cell, &ShortCStr::from(c"~/x"), &[true, true, true]);
    assert_eq!(res.as_bytes().unwrap(), b"~/x");
    assert_eq!(mask, vec![true, true, true]);
    assert!(!expanded);
}

// The gate is **not** bit 0 alone (task #281, POSIX #2.6.1): a tilde expands
// only when no quoted character appears in the **tilde-prefix** — the word's
// bytes from `~` through the first `/` inclusive. Measured in bash 5.3.9 with
// `HOME=/h/o/m/e`, rc 0 in both shells on every row:
//
// ```text
// printf "[%s]" ~"/*"      → [~/*]        printf "[%s]" ~"/*"/sub → [~/*/sub]
// printf "[%s]" ~"/x"      → [~/x]        printf "[%s]" ~'/'x     → [~/x]
// printf "[%s]" ~"a"b      → [~ab]        printf "[%s]" ~a"b"     → [~ab]
// printf "[%s]" ~a/"x"     → [~a/x]       printf "[%s]" ~"root"/* → [~root/*]
// ```
//
// With no `/` the whole word is the prefix (`~"a"b`, `~a"b"`), and the
// terminating `/`'s own bit counts (`~"/*"`, `~"/x"`, `~'/'x`). `~\/x` →
// `~/x` and `~\*` → `~*` agree with bash **by construction**: the arm only
// fires when the byte after `~` is `/` or the end of the word, so a `\<char>`
// there is already unexpandable — POSIX counts `\X` as a quoted character and
// fdshell agrees on every observable row, so `prefix_quoted` deliberately has
// no backslash branch (unobservable without a `~user` lookup).
//
// Every expectation is literal, so these rows need no `HOME`; the `~` must
// keep its own unquoted bit through the copy loop, and a suppressed prefix
// records no splittable byte.
#[test]
fn quoted_tilde_prefix_stays_literal() {
    let cell = dummy_cell();
    for (word, mask) in [
        // `~"/*"`: the only quoted byte is the terminating `/`.
        (c"~/*", &[false, true, true][..]),
        // `~"/x"`, and the single-quote spelling `~'/'x`.
        (c"~/x", &[false, true, false][..]),
        // `~"/*"/sub`: the quote ends before the rest of the word.
        (c"~/*/sub", &[false, true, true, false][..]),
        // `~"a"b`: no `/` ⇒ the whole word is the prefix.
        (c"~ab", &[false, true, false][..]),
        // `~a"b"`: same, quoted after the unquoted user-name bytes.
        (c"~ab", &[false, false, true][..]),
        // `~root/*` from `~"root"/*`: quoted name, unquoted slash.
        (
            c"~root/*",
            &[false, true, true, true, true, false, false][..],
        ),
        // Agreement row, documented: the prefix is unquoted so the arm fires
        // and `a` is not `/`, so the tilde stays literal on its own rule.
        (c"~a/x", &[false, false, false, true][..]),
    ] {
        let arg = ShortCStr::from(word);
        let (res, mask_out, expanded) = expand_marked(&cell, &arg, mask);
        let text = res.as_bytes().unwrap();
        let want: Vec<bool> = (0..text.len())
            .map(|i| mask.get(i).copied().unwrap_or(false))
            .collect();
        assert_eq!(
            text,
            word.to_bytes(),
            "a quoted tilde-prefix is never expanded"
        );
        assert_eq!(mask_out, want, "{word:?}: every byte keeps its own bit");
        assert!(!expanded, "{word:?}: a quoted prefix runs no expansion arm");
    }
}

// The prefix ends at the first `/` **inclusive**, so a quoted byte *after* the
// slash leaves the tilde expandable: bash's `printf "[%s]" ~/"x"` prints
// `[<HOME>/x]` (measured with `HOME=/h/o/m/e`). Task #269 pinned this shape as
// `~/x` with the mask `[false, true, true]`, i.e. the word `~"/x"`, and task
// #281 measured that row in bash as `[~/x]`: a quoted `/` *is* a quoted
// character in the tilde-prefix. See `quoted_tilde_prefix_stays_literal`.
#[test]
fn tilde_quoted_after_slash_still_expands() {
    let cell = dummy_cell();
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(
        &ShortCStr::from(c"~/x"),
        &[false, false, true],
        &mut cache,
        &cell,
    )
    .unwrap();
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), format!("{}/x", home).as_bytes());
}

// Field splitting applies to the bytes an expansion produced (POSIX 2.6.5),
// so `substitute_args` feeds the provenance list to the splitter (task #222,
// superseding the `expanded` bit of task #133). A word whose arms produced
// no bytes has no splittable byte: `IFS=,; a=,; set -- $a` is one word whose
// expansion has exactly the raw word's length. The pins below fail if any
// arm (or the literal path) records the wrong span.
#[test]
fn literal_word_marks_no_expansion() {
    let cell = dummy_cell();
    let (res, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"a,b"), &[]);
    assert_eq!(res.as_bytes().unwrap(), b"a,b");
    assert!(!expanded, "a literal word performed no expansion");
}

#[test]
fn backslash_protected_delimiter_marks_no_expansion() {
    // `a\,b`: `\X` removes the backslash and protects the byte through the
    // mask (POSIX #4.1) — no expansion ran, and the protected delimiter never
    // delimits, so the splitter keeps the word whole on its own.
    let cell = dummy_cell();
    let (res, mask, expanded) = expand_marked(&cell, &ShortCStr::from(c"a\\,b"), &[]);
    assert_eq!(res.as_bytes().unwrap(), b"a,b");
    assert_eq!(mask, vec![false, true, false]);
    assert!(!expanded);
}

#[test]
fn dollar_var_marks_expansion() {
    let cell = dummy_cell();
    let (_, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"$hello"), &[]);
    assert!(expanded);
}

#[test]
fn bare_dollar_marks_expansion() {
    // The literal `$` the fallback arm pushes is arm output: the span covers
    // it, so the word reaches the splitter exactly as before task #133.
    let cell = dummy_cell();
    let (_, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"$"), &[]);
    assert!(expanded);
}

#[test]
fn dollar_at_without_positionals_records_no_splittable_bytes() {
    // `$@` with an empty deque produces no bytes, so nothing may split; the
    // word-level rule (the empty word yields no word) is pinned by the
    // task #219 integration pins in `tests/wordsplit.rs`.
    let cell = dummy_cell();
    let (res, mask, expanded) = expand_marked(&cell, &ShortCStr::from(c"$@"), &[]);
    assert_eq!(res.as_bytes().unwrap(), b"");
    assert!(mask.is_empty());
    assert!(!expanded);
}

#[test]
fn empty_command_subst_records_no_splittable_bytes() {
    // `$(true)` expands to nothing: no expansion byte exists to delimit.
    // The word-level rule — the empty unquoted word is no word, bash splits
    // its empty result — is pinned by `unquoted_empty_cmd_subst_is_no_words`
    // in `tests/cmd_subst.rs`.
    let cell = dummy_cell();
    let (res, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"$(true)"), &[]);
    assert_eq!(res.as_bytes().unwrap(), b"");
    assert!(!expanded);
}

#[test]
fn bound_percent_fd_marks_expansion() {
    let cell = dummy_cell();
    let (rd, _wr) = sys::pipe::pipe2(0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .set_fd_var(ShortCStr::from(c"?"), fdvar(rd));
    let (_, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"%?"), &[]);
    assert!(expanded);
}

#[test]
fn unbound_percent_fd_marks_expansion() {
    // The literal `%?` output is arm output: the span covers it, so such a
    // word keeps going through the splitter exactly as before task #133.
    let cell = dummy_cell();
    let (_, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"%nosuchfd"), &[]);
    assert!(expanded);
}

#[test]
fn leading_tilde_output_is_never_splittable() {
    // `~` pushed $HOME, and the home bytes are **literal** in the provenance
    // (task #222): bash never splits tilde output, so a literal `IFS` byte in
    // `$HOME` stays content — pinned end to end by
    // `tests/wordsplit.rs::tilde_output_is_never_split_by_ifs` (`~` →
    // `[a,b]`, `~/x` → `[a,b/x]`, and the quoted-tilde contrast `"~"$HOME` →
    // `[~a][b]`, where the `$HOME` bytes *are* splittable).
    let cell = dummy_cell();
    let (res, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"~"), &[]);
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), home.as_bytes());
    assert!(!expanded, "the tilde records no splittable bytes");
}

#[test]
fn tilde_user_marks_no_expansion() {
    // `~user` is literal here (README), so the word performed no expansion
    // and a literal `IFS` byte inside it must not split it. An unset `HOME`
    // records nothing either: the tilde arm never produces a span (§ above).
    let cell = dummy_cell();
    let (res, _, expanded) = expand_marked(&cell, &ShortCStr::from(c"~nobody:x"), &[]);
    assert_eq!(res.as_bytes().unwrap(), b"~nobody:x");
    assert!(!expanded);
}

#[test]
fn dollar_bang_returns_last_bg_pid() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$!");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &s_cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn dollar_bang_in_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"job=$! done");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"job=12345 done");
}

#[test]
fn brace_hash_known_var_returns_length() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#hello}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"5");
}

#[test]
fn brace_hash_empty_var_returns_zero() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#empty}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"0");
}

#[test]
fn brace_hash_unknown_var_is_zero() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"${#nope}");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"0");
}

#[test]
fn brace_hash_in_text() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"len=${#hello} end");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"len=5 end");
}

fn positional_cell() -> ForkCell<ShellState> {
    positional_cell_with_n(3)
}

/// The bash model: `$0` is the program name (`arg0`, a parameter of its own)
/// and `$1..` are the deque (`arg1..arg(n-1)`), so `$#` is `n - 1`.
fn positional_cell_with_n(n: usize) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    let mut positional: alloc::collections::VecDeque<ImportedStr> =
        alloc::collections::VecDeque::new();
    for i in 1..n {
        positional.push_back(ImportedStr::shell(match i {
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
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_program_name(ImportedStr::shell(ShortCStr::from(c"arg0")));
        state.set_positional(positional);
    }
    cell
}

#[test]
fn dollar_hash_returns_positional_count() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$#");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"2");
}

#[test]
fn dollar_hash_empty_positional() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(VecDeque::new());
    let arg = ShortCStr::from(c"$#");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"0");
}

#[test]
fn dollar_at_expands_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$@");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg1 arg2");
}

#[test]
fn dollar_star_expands_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$*");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg1 arg2");
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"only");
}

#[test]
fn dollar_at_empty_positional() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(VecDeque::new());
    let arg = ShortCStr::from(c"$@");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn dollar_zero_is_program_name() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$0");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg0");
}

#[test]
fn dollar_one_is_first_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$1");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg1");
}

#[test]
fn dollar_zero_survives_nounset() {
    // `$0` is always bound (bash: `set -u; echo $0` prints the program name).
    let cell = positional_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"$0");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg0");
}

#[test]
fn nounset_positional_one_past_the_end_names_the_index() {
    // 3 parameters means `$#` = 2: `$3` is the first out-of-range index and
    // bash names it verbatim (`3: unbound variable`).
    let cell = positional_cell_with_n(3);
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let arg = ShortCStr::from(c"$3");
    let mut cache = HashMap::new();
    let e = substitute_arg(&arg, &[], &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var }
            if var.as_bytes().unwrap() == b"3".as_slice()
    ));
}

#[test]
fn dollar_n_is_third_positional() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$2");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg2");
}

#[test]
fn dollar_n_out_of_range_is_empty() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$9");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"");
}

#[test]
fn dollar_positional_in_text() {
    let cell = positional_cell();
    let arg = ShortCStr::from(c"$0-$1-$2");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg0-arg1-arg2");
}

#[test]
fn dollar_multi_digit_index() {
    let cell = positional_cell_with_n(15);
    let arg = ShortCStr::from(c"$10");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg10");
}

#[test]
fn dollar_multi_digit_index_in_text() {
    let cell = positional_cell_with_n(20);
    let arg = ShortCStr::from(c"$1-$10-$19");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"arg1-arg10-arg19");
}

#[test]
fn percent_double_percent_stays_literal() {
    // Task #155: the shell has no `%%` escape — both bytes reach argv so the
    // `printf` builtin can interpret its own `%%` (bash prints `a%b` here).
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a%%b");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a%%b");
}

#[test]
fn percent_double_percent_keeps_the_quote_mask() {
    // The mask invariant of the literal `%`: one byte consumed, one byte
    // written, so `realign` pads 1:1 and the quoted bits stay parallel.
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"a%%b");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (res, mask) = substitute_arg(&arg, &[true, true, true, true], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a%%b");
    assert_eq!(mask, vec![true; 4]);
}

#[test]
fn percent_triple_percent_is_three_lits() {
    let cell = dummy_cell();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let expand = |arg: &'static core::ffi::CStr, cache: &mut HashMap<ShortCStr, ExportedFd>| {
        substitute_arg(&ShortCStr::from(arg), &[], cache, &cell)
            .unwrap()
            .0
            .as_bytes()
            .unwrap()
            .to_vec()
    };
    assert_eq!(expand(c"%%%", &mut cache), b"%%%");
    // The `?` arm keeps its literal fallback when `?` is unbound.
    assert_eq!(expand(c"%%?", &mut cache), b"%%?");
}

#[test]
fn percent_double_percent_before_a_name_is_literal() {
    // Unbound `o`: nothing substitutes, all three bytes are literal.
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"%%o");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"%%o");
}

#[test]
fn percent_double_percent_with_bound_fd_substitutes_the_second_name() {
    // Accepted divergence pinned for task #155: with fd `o` bound the second
    // `%` starts a name lookup, so `%%o` becomes `%<fdnum>` (bash prints
    // `%o`). Task #252 owns the quoting gate that would fix it.
    let cell = ForkCell::new(ShellState::new());
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .fds
        .insert(ShortCStr::from(c"o"), fdvar(dev_null));
    let arg = ShortCStr::from(c"%%o");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    let bytes = res.as_bytes().unwrap().to_vec();
    let fd = cache.get(&ShortCStr::from(c"o")).unwrap().as_raw();
    assert_eq!(bytes, format!("%{fd}").as_bytes());
}

#[test]
fn percent_escaped_percent_with_bound_fd_is_literal() {
    // POSIX #4.1 outside quotes: `\%` protects the `%`, so the arm never runs
    // and the bound fd `s` is not substituted. The escaped byte is masked
    // quoted, the following name bytes keep their own (unquoted) bits.
    let cell = ForkCell::new(ShellState::new());
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .fds
        .insert(ShortCStr::from(c"s"), fdvar(dev_null));
    let arg = ShortCStr::from(c"\\%s");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (res, mask) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"%s");
    assert_eq!(mask, vec![true, false]);
}

#[test]
fn percent_single_percent_unknown_fd_is_literal() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"%unknown_fd");
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res1, _) = substitute_arg(&arg1, &[], &mut cache, &cell).unwrap();

    // Second call — should hit cache and return same value
    let arg2 = ShortCStr::from(c"%testfd");
    let (res2, _) = substitute_arg(&arg2, &[], &mut cache, &cell).unwrap();
    assert_eq!(res1.as_bytes().unwrap(), res2.as_bytes().unwrap());

    // Third call with surrounding text — still hits cache
    let arg3 = ShortCStr::from(c"prefix-%testfd-suffix");
    let (res3, _) = substitute_arg(&arg3, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
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
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"%-testfd");
}

// Mutant-catching tests for substitute.rs (MISSED 25-31)
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
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![false, false]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![false, false]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![false, false]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![false, false]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"ab");
}

#[test]
fn unquoted_var_with_spaces_splits_on_ifs() {
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$multi_word")];
    let args_mask = alloc::vec![alloc::vec![false; 11]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].as_bytes().unwrap(), b"two");
    assert_eq!(result[1].as_bytes().unwrap(), b"words");
}

#[test]
fn quoted_var_with_spaces_does_not_split() {
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$multi_word")];
    let args_mask = alloc::vec![alloc::vec![true; 11]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let args_mask = alloc::vec![alloc::vec![true; 5]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
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
    let result = super::positional::expand_positional_word(false, true, &cell).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].0.as_bytes().unwrap(), b"arg0");
    assert_eq!(result[1].0.as_bytes().unwrap(), b"arg1");
    assert_eq!(result[2].0.as_bytes().unwrap(), b"arg2");
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

fn empty_masked_positional_cell() -> ForkCell<ShellState> {
    // Positionals `a "" b` (bash: `set -- a "" b`).
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"a", c"", c"b"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    cell
}

#[test]
fn empty_quoted_arg_is_one_empty_word() {
    // An empty mask only arises from quoted material: `""` is one empty word.
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"")];
    let args_mask = alloc::vec![alloc::vec![]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].is_empty());
}

// A word that performed no expansion is one field (task #133, POSIX 2.6.5):
// its own bytes are never split on `IFS`.
#[test]
fn literal_word_with_ifs_byte_is_one_word() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().ifs = ShortCStr::from(c",");
    let args = alloc::vec![ShortCStr::from(c"a,b")];
    let args_mask = alloc::vec![alloc::vec![false; 3]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"a,b");
}

#[test]
fn expansion_with_space_still_splits_on_default_ifs() {
    // `x="a b"; set -- $x` — bash: two positionals. Guard: the gate
    // suppresses the splitting of unexpanded words, not the splitting of an
    // expansion's own whitespace. This is also the case that the
    // `out.len() != arg.len()` proxy flips: the expansion has exactly the raw
    // word's length (3 bytes) and must still split.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"x"), is_(c"a b"));
    let args = alloc::vec![ShortCStr::from(c"$x")];
    let args_mask = alloc::vec![alloc::vec![false; 2]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert_eq!(result[1].as_bytes().unwrap(), b"b");
}

// The `expanded` flag must be tracked, not inferred from the text: these two
// words have an expansion of exactly the raw word's length and go both ways.
#[test]
fn equal_length_expansion_of_comma_splits_to_one_empty_word() {
    // `IFS=,; a=,; set -- $a` — bash: one empty positional.
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = ShortCStr::from(c",");
        state.strings.insert(ShortCStr::from(c"a"), is_(c","));
    }
    let args = alloc::vec![ShortCStr::from(c"$a")];
    let args_mask = alloc::vec![alloc::vec![false; 2]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].is_empty());
}

#[test]
fn equal_length_expansion_of_delimited_pair_splits_to_one_word() {
    // `IFS=,; a=b,; set -- $a` — bash: one positional `b` (the delimiter at
    // the end of the expansion produces no trailing empty field). A
    // byte-comparison `expanded` proxy would call this word unexpanded
    // (3 bytes in, 1 byte out of the same length as the raw `b,`) and keep
    // the raw text whole: `b,`.
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.ifs = ShortCStr::from(c",");
        state.strings.insert(ShortCStr::from(c"a"), is_(c"b,"));
    }
    let args = alloc::vec![ShortCStr::from(c"$a")];
    let args_mask = alloc::vec![alloc::vec![false; 2]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"b");
}

#[test]
fn quoted_dollar_at_keeps_empty_positional() {
    // Quoted "$@": one word per positional, empty ones included.
    let cell = empty_masked_positional_cell();
    let result = super::positional::expand_positional_word(false, true, &cell).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].0.as_bytes().unwrap(), b"a");
    assert!(result[1].0.is_empty());
    assert_eq!(result[2].0.as_bytes().unwrap(), b"b");
}

#[test]
fn unquoted_dollar_at_drops_empty_positional() {
    // Unquoted $@: each positional is IFS-split separately; an empty one
    // contributes no words (bash: `set -- a "" b; set -- $@` → `$#` = 2).
    let cell = empty_masked_positional_cell();
    let result = super::positional::expand_positional_word(false, false, &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0.as_bytes().unwrap(), b"a");
    assert_eq!(result[1].0.as_bytes().unwrap(), b"b");
}

#[test]
fn quoted_dollar_star_joins_empty_element_with_two_separators() {
    // Quoted "$*" over `a "" b` joins with the IFS byte between every pair:
    // `a` + ` ` + `` + ` ` + `b` = `a  b` (bash).
    let cell = empty_masked_positional_cell();
    let args = alloc::vec![ShortCStr::from(c"$*")];
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"a  b");
}

// Quoted-empty words with a runtime-empty expansion (task #76): a word whose
// raw span contained quotes but whose IFS split produced no fields is one
// empty word, as in bash.
#[test]
fn quoted_empty_cmd_subst_is_one_empty_word() {
    // `""$(true)`: the quoted part contributes no bytes; the expansion is
    // empty, so the word must survive as one empty word.
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$(true)")];
    let args_mask = alloc::vec![alloc::vec![false; 7]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].is_empty());
}

#[test]
fn unquoted_empty_cmd_subst_is_no_words() {
    // Guard: without quotes in the word, an empty expansion vanishes.
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$(true)")];
    let args_mask = alloc::vec![alloc::vec![false; 7]];
    let result = super::substitute_args(&args, &args_mask, &[], &cell).unwrap();
    assert!(result.is_empty());
}

#[test]
fn mixed_quoting_empty_expansion_keeps_empty_word() {
    // `a ""$(true) b`: the flag vector must map to the right words — only the
    // middle one is quoted, so only it may gain the empty word.
    let cell = dummy_cell();
    let args = alloc::vec![
        ShortCStr::from(c"a"),
        ShortCStr::from(c"$(true)"),
        ShortCStr::from(c"b"),
    ];
    let args_mask = alloc::vec![
        alloc::vec![false],
        alloc::vec![false; 7],
        alloc::vec![false],
    ];
    let args_quoted = alloc::vec![false, true, false];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert!(result[1].is_empty());
    assert_eq!(result[2].as_bytes().unwrap(), b"b");
}

#[test]
fn quoted_dollar_at_no_positionals_is_one_empty_word() {
    // `""$@` with no positionals: bash keeps one empty word.
    let cell = ForkCell::new(ShellState::new());
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_mask = alloc::vec![alloc::vec![false, false]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].is_empty());
}

#[test]
fn quoted_dollar_at_single_empty_positional_is_one_empty_word() {
    // `set -- ""; set -- ""$@`: the empty positional re-splits to nothing,
    // but the quoted word keeps one empty word.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c""]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_mask = alloc::vec![alloc::vec![false, false]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].is_empty());
}

#[test]
fn quoted_dollar_at_nonempty_positionals_not_padded() {
    // Guard: `""$@` over `a b` yields `a b` — no empty word is synthesized
    // when the expansion produced words.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"a", c"b"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_mask = alloc::vec![alloc::vec![false, false]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert_eq!(result[1].as_bytes().unwrap(), b"b");
}

// The fully quoted whole-word `$@` is the exception to the quoted-empty rule
// (task #219): bash's `"$@"` *is* the positional word list, so an empty
// deque is no word at all. The discriminator is the quote mask, never the
// per-word `quoted` flag — the empty-deque triple `"$@"`, `"$*"` and `""$@`
// must all be pinned for the fix to mean anything.
#[test]
fn fully_quoted_dollar_at_with_no_positionals_is_no_words() {
    // `set --; set -- "$@"` — bash: `$#` = 0.
    let cell = ForkCell::new(ShellState::new());
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert!(result.is_empty(), "an empty deque yields no words");
}

#[test]
fn fully_quoted_dollar_star_with_no_positionals_is_one_empty_word() {
    // `set --; set -- "$*"` — bash: `$#` = 1: the join path's single word is
    // empty but present, so the suppression above must not zero it.
    let cell = ForkCell::new(ShellState::new());
    let args = alloc::vec![ShortCStr::from(c"$*")];
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].is_empty());
}

#[test]
fn fully_quoted_dollar_at_keeps_empty_positional_words() {
    // `set -- a "" b; set -- "$@"` — bash: `$#` = 3. Guard: the suppression
    // runs after the push loop, so positional words — empty ones included —
    // are never dropped by it.
    let cell = empty_masked_positional_cell();
    let args = alloc::vec![ShortCStr::from(c"$@")];
    let args_mask = alloc::vec![alloc::vec![true, true]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert!(result[1].is_empty());
    assert_eq!(result[2].as_bytes().unwrap(), b"b");
}

#[test]
fn quoted_whitespace_only_expansion_is_one_empty_word() {
    // `""$(echo "  ")` with the default IFS: two spaces split to zero fields,
    // so the quoted word is one empty word.
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$(echo \"  \")")];
    let args_mask = alloc::vec![alloc::vec![false; 12]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].is_empty());
}

#[test]
fn quoted_whitespace_only_expansion_empty_ifs_keeps_spaces() {
    // With `IFS=`, the same word splits to one field `  ` and stays that.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().ifs = ShortCStr::new();
    let args = alloc::vec![ShortCStr::from(c"$(echo \"  \")")];
    let args_mask = alloc::vec![alloc::vec![false; 12]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].as_bytes().unwrap(), b"  ");
}

#[test]
fn quoted_multiword_expansion_still_splits() {
    // `""$(echo "a b")`: the expansion produces words, so they are kept as-is.
    let cell = dummy_cell();
    let args = alloc::vec![ShortCStr::from(c"$(echo \"a b\")")];
    let args_mask = alloc::vec![alloc::vec![false; 13]];
    let args_quoted = alloc::vec![true];
    let result = super::substitute_args(&args, &args_mask, &args_quoted, &cell).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].as_bytes().unwrap(), b"a");
    assert_eq!(result[1].as_bytes().unwrap(), b"b");
}

// Mutant-catching tests for substitute/paren.rs
#[test]
fn paren_single_level_expr() {
    let mut peek = Counting::new(b"echo hi)".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"echo hi".as_slice());
}

#[test]
fn paren_nested_expr_keeps_all_levels() {
    // depth += 1 mutant (-=, *=) truncates or corrupts nested captures.
    // Input mirrors what arg.rs passes: outer `$(` already consumed, so the
    // byte sequence ends with the `)` closing that outer level.
    let input = b"(a(b(c))d))";
    let mut peek = Counting::new(input.iter().copied());
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"(a(b(c))d)");
}

#[test]
fn paren_empty_inner() {
    let mut peek = Counting::new(b")".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert!(res.is_empty());
}

#[test]
fn paren_unclosed_is_error() {
    let mut peek = Counting::new(b"abc".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek);
    assert!(matches!(
        res.unwrap_err().current_context(),
        crate::error::resolve::ResolveError::UnclosedParen
    ));
}

#[test]
fn paren_paren_in_quotes_is_data() {
    let mut peek = Counting::new(b"echo \"a)b\")".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"echo \"a)b\"".as_slice());
}

#[test]
fn paren_open_paren_in_quotes_is_data() {
    let mut peek = Counting::new(b"a\"(b)c\")".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"a\"(b)c\"".as_slice());
}

#[test]
fn paren_escaped_quote_in_quotes_is_data() {
    let mut peek = Counting::new(b"echo \"a\\\"b)\")".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek).unwrap();
    assert_eq!(res, b"echo \"a\\\"b)\"".as_slice());
}

#[test]
fn paren_unclosed_quote_is_error() {
    let mut peek = Counting::new(b"echo \"a".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek);
    assert!(matches!(
        res.unwrap_err().current_context(),
        crate::error::resolve::ResolveError::UnclosedParen
    ));
}

#[test]
fn paren_trailing_escape_in_quotes_is_error() {
    let mut peek = Counting::new(b"echo \"a\\".iter().copied());
    let res = super::paren::read_paren_expr(&mut peek);
    assert!(matches!(
        res.unwrap_err().current_context(),
        crate::error::resolve::ResolveError::UnclosedParen
    ));
}

#[test]
fn dollar_paren_arith_unclosed_is_error() {
    // `$((` routes to the arithmetic body scan (depth 2), which needs both
    // closing parens; a missing one is `UnclosedParen`, not a command
    // substitution.
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"$((1+1");
    let mut cache = HashMap::new();
    let res = substitute_arg(&arg, &[], &mut cache, &cell);
    assert!(matches!(
        res.unwrap_err().current_context(),
        crate::error::resolve::ResolveError::UnclosedParen
    ));
}

#[test]
fn dollar_underscore_expands_to_last_arg_var() {
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"_"), is_(c"lastword"));
    let arg = ShortCStr::from(c"a $_ b");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"a lastword b");
}

#[test]
fn dollar_underscore_unset_is_empty() {
    // An unset `_` expands to empty, like every other parameter — it needs a
    // pristine environ without an inherited `_`, so reuse env_cell's
    // cleared-environ setup.
    let cell = env_cell();
    let arg = ShortCStr::from(c"[$_]");
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"[]");
}

// Quote-mask propagation: copied bytes keep their bit, expansion inherits
// the trigger byte's bit.
#[test]
fn copied_bytes_keep_unquoted_mask() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"hello");
    let mut cache = HashMap::new();
    let (res, mask) = substitute_arg(&arg, &[false; 5], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"hello");
    assert_eq!(mask, vec![false; 5]);
}

#[test]
fn expansion_inherits_trigger_mask_bit() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"x$multi_word");
    let mut cache = HashMap::new();
    let mask = vec![
        false, true, true, true, true, true, true, true, true, true, true, true,
    ];
    let (res, mask_out) = substitute_arg(&arg, &mask, &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"xtwo words");
    assert_eq!(
        mask_out,
        vec![false, true, true, true, true, true, true, true, true, true]
    );
}

#[test]
fn unquoted_expansion_yields_unprotected_mask() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"x$multi_word");
    let mut cache = HashMap::new();
    let (res, mask_out) = substitute_arg(&arg, &[], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"xtwo words");
    assert!(mask_out.iter().all(|&q| !q));
}

// Mask-index alignment: substitution helpers consume several input bytes
// (the variable name, the paren body); later original bytes must keep their
// own mask bit, not the bit of the byte the helper stopped at.
#[test]
fn backslash_escape_keeps_mask_alignment() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"\\\\ab");
    let mut cache = HashMap::new();
    let (res, mask_out) =
        substitute_arg(&arg, &[true, true, false, false], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"\\ab");
    assert_eq!(mask_out, vec![true, false, false]);
}

#[test]
fn dollar_escape_keeps_mask_alignment() {
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"\\$ab");
    let mut cache = HashMap::new();
    let (res, mask_out) =
        substitute_arg(&arg, &[true, true, false, false], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"$ab");
    assert_eq!(mask_out, vec![true, false, false]);
}

#[test]
fn backslash_plain_char_keeps_mask_alignment() {
    // A backslash before a non-special char is kept and consumes no extra
    // byte; later bytes keep their own mask bits, not a shifted index.
    let cell = dummy_cell();
    let arg = ShortCStr::from(c"\\ab");
    let mut cache = HashMap::new();
    let (res, mask_out) = substitute_arg(&arg, &[true, false, false], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"\\ab");
    assert_eq!(mask_out, vec![true, false, false]);
}

#[test]
fn var_name_consumption_keeps_mask_alignment() {
    // The name `v` is consumed by the substitution; the trailing `:` must
    // keep its own (unquoted) bit, not the name's quoted bit. `v` is bound so
    // the expansion has bytes to align: an unset name expands to nothing and
    // asserts nothing about the trailing byte's bit.
    let cell = dummy_cell();
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(c"v"), is_(c"W"));
    let arg = ShortCStr::from(c"x$v:");
    let mut cache = HashMap::new();
    let (res, mask_out) =
        substitute_arg(&arg, &[false, true, true, false], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"xW:");
    assert_eq!(mask_out, vec![false, true, false]);
}

#[test]
fn percent_expansion_inherits_trigger_mask_bit() {
    let cell = ForkCell::new(ShellState::new());
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    cell.borrow_mut()
        .unwrap()
        .fds
        .insert(ShortCStr::from(c"_fd"), fdvar(dev_null));
    let arg = ShortCStr::from(c"a%_fd");
    let mut cache = HashMap::new();
    let (res, mask_out) =
        substitute_arg(&arg, &[false, true, true, true, true], &mut cache, &cell).unwrap();
    assert!(
        res.as_bytes()
            .unwrap()
            .iter()
            .all(|b| b.is_ascii_digit() || *b == b'a')
    );
    assert!(mask_out.iter().enumerate().all(|(i, &q)| i == 0 || q));
}

// --- the `#`/`##`/`%`/`%%` pattern operators (task #106) ---

use super::param_op::{ParamOp, split_operator};

/// `split_operator` on a braced body, as bytes (the scan is positional).
fn scan(body: &'static core::ffi::CStr) -> Option<(Vec<u8>, ParamOp, Vec<u8>)> {
    let (name, op, word) = split_operator(&ShortCStr::from(body))?;
    Some((
        name.as_bytes().unwrap().to_vec(),
        op,
        word.as_bytes().unwrap().to_vec(),
    ))
}

/// Expand `arg` with the `mask` (parallel to the word text) against `cell`.
fn expand(cell: &ForkCell<ShellState>, arg: &'static core::ffi::CStr, mask: &[bool]) -> Vec<u8> {
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(&ShortCStr::from(arg), mask, &mut cache, cell).unwrap();
    res.as_bytes().unwrap().to_vec()
}

fn bind(
    cell: &ForkCell<ShellState>,
    name: &'static core::ffi::CStr,
    value: &'static core::ffi::CStr,
) {
    cell.borrow_mut()
        .unwrap()
        .strings
        .insert(ShortCStr::from(name), is_(value));
}

#[test]
fn pattern_operator_scan_is_positional() {
    use ParamOp::{Assign, Default, LongestPrefix, LongestSuffix, ShortestPrefix, ShortestSuffix};
    let b = |s: &'static core::ffi::CStr| ShortCStr::from(s).as_bytes().unwrap().to_vec();
    assert_eq!(scan(c"v#a"), Some((b(c"v"), ShortestPrefix, b(c"a"))));
    assert_eq!(scan(c"v##a"), Some((b(c"v"), LongestPrefix, b(c"a"))));
    assert_eq!(scan(c"v%a"), Some((b(c"v"), ShortestSuffix, b(c"a"))));
    assert_eq!(scan(c"v%%a"), Some((b(c"v"), LongestSuffix, b(c"a"))));
    // The operator byte at the body end is an operator with the empty pattern.
    assert_eq!(scan(c"v#"), Some((b(c"v"), ShortestPrefix, b(c""))));
    // The first operator byte wins, so the colon after `#` is pattern content.
    assert_eq!(scan(c"v#a:"), Some((b(c"v"), ShortestPrefix, b(c"a:"))));
    assert_eq!(scan(c"v:-x#y"), Some((b(c"v"), Default, b(c"x#y"))));
    // A colon that is not an operator byte does not stop the scan.
    assert_eq!(scan(c"v:a#b"), Some((b(c"v:a"), ShortestPrefix, b(c"b"))));
    assert_eq!(scan(c"v:=a"), Some((b(c"v"), Assign, b(c"a"))));
    // A single-char operator is not an operator (fdshell implements the colon
    // forms only), so `+` stays inside the name.
    assert_eq!(scan(c"v+a#y"), Some((b(c"v+a"), ShortestPrefix, b(c"y"))));
    // A `#`/`%` at index 0 gives the empty name, and a lone `:` is no operator.
    // The empty scan name never reaches a dispatch: brace.rs resolves the whole
    // content as the parameter name (pinned by scan_empty_name_resolves_the_content).
    assert_eq!(scan(c"#x"), Some((b(c""), ShortestPrefix, b(c"x"))));
    assert_eq!(scan(c"%x"), Some((b(c""), ShortestSuffix, b(c"x"))));
    assert_eq!(scan(c"v:a"), None);
    assert_eq!(scan(c"v:a:-w"), Some((b(c"v:a"), Default, b(c"w"))));
}

#[test]
fn pattern_operators_strip_the_anchored_match() {
    let cell = dummy_cell();
    bind(&cell, c"v", c"abcabc");
    let m = &[false; 8];
    assert_eq!(expand(&cell, c"${v#a}", m), b"bcabc");
    assert_eq!(expand(&cell, c"${v##a}", m), b"bcabc");
    // Shortest `a*c` = `abc`, longest = the whole value.
    assert_eq!(expand(&cell, c"${v#a*c}", m), b"abc");
    assert_eq!(expand(&cell, c"${v##a*c}", m), b"");
    assert_eq!(expand(&cell, c"${v%?}", m), b"abcab");
    assert_eq!(expand(&cell, c"${v%%abc}", m), b"abc");
}

#[test]
fn pattern_direction_and_bounds() {
    // `#`/`%` direction on a value that starts and ends with `a`, and the
    // longest-star bounds that take the whole value (`n - k` suffix slice).
    let cell = dummy_cell();
    bind(&cell, c"v", c"abca");
    let m = &[false; 9];
    assert_eq!(expand(&cell, c"${v#a}", m), b"bca");
    assert_eq!(expand(&cell, c"${v%a}", m), b"abc");
    assert_eq!(expand(&cell, c"${v##a*}", m), b"");
    assert_eq!(expand(&cell, c"${v%%*a}", m), b"");
    bind(&cell, c"s", c"aXbXc");
    assert_eq!(expand(&cell, c"${s#*X}", m), b"bXc");
    assert_eq!(expand(&cell, c"${s##*X}", m), b"c");
    assert_eq!(expand(&cell, c"${s%X*}", m), b"aXb");
    assert_eq!(expand(&cell, c"${s%%X*}", m), b"a");
}

#[test]
fn no_match_and_empty_pattern_leave_the_value_whole() {
    let cell = dummy_cell();
    bind(&cell, c"v", c"abcabc");
    let m = &[false; 7];
    assert_eq!(expand(&cell, c"${v#z}", m), b"abcabc");
    assert_eq!(expand(&cell, c"${v#}", m), b"abcabc");
    assert_eq!(expand(&cell, c"${v#[]}", m), b"abcabc");
    // An unclosed `[` is a literal byte, so nothing matches.
    assert_eq!(expand(&cell, c"${v#[a}", m), b"abcabc");
    assert_eq!(expand(&cell, c"${v#*x}", m), b"abcabc");
}

#[test]
fn word_pattern_has_no_dot_rule() {
    // A leading `*` may consume a leading `.` (no FNM_PERIOD in a word pattern).
    let cell = dummy_cell();
    bind(&cell, c"d", c".abc");
    assert_eq!(expand(&cell, c"${d#*c}", &[false; 7]), b"");
    assert_eq!(expand(&cell, c"${d#c*}", &[false; 7]), b".abc");
    assert_eq!(expand(&cell, c"${d#.*}", &[false; 7]), b"abc");
}

#[test]
fn bracket_ranges_and_negation_in_a_pattern() {
    let cell = dummy_cell();
    bind(&cell, c"v", c"abcabc");
    let m = &[false; 10];
    assert_eq!(expand(&cell, c"${v#[ab]*}", m), b"bcabc");
    assert_eq!(expand(&cell, c"${v#[a-b]*}", m), b"bcabc");
    assert_eq!(expand(&cell, c"${v#[!b]*}", m), b"bcabc");
    assert_eq!(expand(&cell, c"${v#[a-c]bc}", m), b"abc");
}

#[test]
fn quoted_pattern_bytes_are_literal_by_the_mask() {
    // A masked pattern byte is literal, so a quoted `*` matches nothing.
    let cell = dummy_cell();
    bind(&cell, c"v", c"abcabc");
    assert_eq!(expand(&cell, c"${v#*c}", &[false; 7]), b"abc");
    assert_eq!(expand(&cell, c"${v#*c}", &[true; 7]), b"abcabc");
    // The mask slice is the pattern's own bytes: the first pattern byte quoted
    // and the second live (`${v#"a"b}` strips `ab`). The tokenizer strips the
    // `"` bytes, so the word text is `${v#ab}` with the `a` bit set.
    assert_eq!(
        expand(
            &cell,
            c"${v#ab}",
            &[false, false, false, false, true, false, false]
        ),
        b"cabc"
    );
    // A quoted `[` is a literal byte, not a bracket opening (`${v#"["a]}`).
    assert_eq!(
        expand(
            &cell,
            c"${v#[a]}",
            &[false, false, false, false, true, false, false]
        ),
        b"abcabc"
    );
}

#[test]
fn mask_shorter_than_the_pattern_pads_unquoted() {
    // The mask is parallel to the word text; a mask that stops before the
    // pattern pads the pattern bytes as unquoted, so the star stays live.
    let cell = dummy_cell();
    bind(&cell, c"v", c"abcx");
    assert_eq!(expand(&cell, c"${v#*x}", &[false, false, false]), b"");
    assert_eq!(expand(&cell, c"${v#*x}", &[true, true, true]), b"");
}

#[test]
fn word_offset_positions_the_pattern_mask() {
    // `${v##*x}` has two operator bytes, so its pattern mask starts one byte
    // later than `${v#*x}`: the same mask bit is the `*` of the short operator
    // and the second `#` of the long one.
    let cell = dummy_cell();
    bind(&cell, c"v", c"abcx");
    // `$` `{` `v` `#` `#` `*` `x` `}` — the bit set at index 4.
    let quoted_at_4 = [false, false, false, false, true, false, false, false];
    // The short operator reads mask[4] = its `*`: quoted, so the pattern is
    // literal bytes and nothing matches.
    assert_eq!(expand(&cell, c"${v#*x}", &quoted_at_4[0..7]), b"abcx");
    // The long operator reads mask[5] = its `*`: unquoted, so `*x` strips the
    // whole value.
    assert_eq!(expand(&cell, c"${v##*x}", &quoted_at_4), b"");
}

#[test]
fn indirect_name_with_a_pattern() {
    let cell = dummy_cell();
    bind(&cell, c"ind", c"v");
    bind(&cell, c"v", c"abcabc");
    let m = &[false; 10];
    assert_eq!(expand(&cell, c"${!ind#a}", m), b"bcabc");
    assert_eq!(expand(&cell, c"${!ind%bc}", m), b"abca");
    assert_eq!(expand(&cell, c"${!ind}", m), b"abcabc");
}

#[test]
fn pattern_arm_expands_an_unset_parameter_to_empty() {
    let cell = env_cell();
    assert_eq!(expand(&cell, c"${undef#q}", &[false; 8]), b"");
    assert_eq!(expand(&cell, c"${undef%%*}", &[false; 9]), b"");
    // The colon family keeps its nounset exemption: its word supplies the value.
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let mut cache = HashMap::new();
    let (res, _) = substitute_arg(
        &ShortCStr::from(c"${undef:-x}"),
        &[false; 10],
        &mut cache,
        &cell,
    )
    .unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"x");
}

#[test]
fn nounset_bails_a_pattern_parameter() {
    let cell = env_cell();
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let e = substitute_arg(
        &ShortCStr::from(c"${undef#q}"),
        &[false; 8],
        &mut HashMap::new(),
        &cell,
    )
    .unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"undef"
    ));
    // An unbound indirect name keeps bash's `nope: invalid indirect expansion`,
    // and a bound indirect name whose target is unbound reports the `!` body.
    let e = substitute_arg(
        &ShortCStr::from(c"${!nope#a}"),
        &[false; 9],
        &mut HashMap::new(),
        &cell,
    )
    .unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::InvalidIndirect { var } if var.as_bytes().unwrap() == b"nope"
    ));
    bind(&cell, c"p", c"n");
    let e = substitute_arg(
        &ShortCStr::from(c"${!p#a}"),
        &[false; 7],
        &mut HashMap::new(),
        &cell,
    )
    .unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"!p"
    ));
}

#[test]
fn indirect_empty_target_with_a_pattern_errors() {
    // `p` is bound to the empty string, so it names no target.
    let cell = dummy_cell();
    let e = substitute_arg(
        &ShortCStr::from(c"${!empty#a}"),
        &[false; 9],
        &mut HashMap::new(),
        &cell,
    )
    .unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::InvalidIndirect { var } if var.as_bytes().unwrap() == b""
    ));
}

#[test]
fn scan_empty_name_resolves_the_content() {
    // A pattern operator at index 0 (`${%x}`, `${%x#y}`) leaves the scan name
    // empty, so the braced content is read as the parameter name: an unset
    // parameter expands to empty at rc 0, and under `set -u` the message names
    // the content (`%x: unbound variable`), never an empty name (STYLE §4.7).
    let cell = env_cell();
    assert_eq!(expand(&cell, c"${%x}", &[false; 5]), b"");
    assert_eq!(expand(&cell, c"${%x#y}", &[false; 7]), b"");
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let cases: [(&'static core::ffi::CStr, &[u8]); 2] = [(c"${%x}", b"%x"), (c"${%x#y}", b"%x#y")];
    for (arg, want) in cases {
        let e = substitute_arg(
            &ShortCStr::from(arg),
            &[false; 8],
            &mut HashMap::new(),
            &cell,
        )
        .unwrap_err();
        assert!(matches!(
            e.current_context(),
            ResolveError::UnboundVariable { var }
                if var.as_bytes().unwrap() == want
        ));
    }
    // `set -u` keeps the degenerate literals and the length arm's rc-0 `0`.
    assert_eq!(expand(&cell, c"${}", &[false; 3]), b"${}");
    assert_eq!(expand(&cell, c"${!}", &[false; 4]), b"${!}");
    assert_eq!(expand(&cell, c"${#}", &[false; 3]), b"0");
}

// --- positional and special parameters inside `${…}` (task #177) ---

#[test]
fn braced_digit_name_is_the_positional() {
    // A digit run in braces is the positional, not a variable named `1`.
    let cell = positional_cell_with_n(4);
    let m = &[false; 20];
    assert_eq!(expand(&cell, c"${1}", m), b"arg1");
    assert_eq!(expand(&cell, c"${2}", m), b"arg2");
    assert_eq!(expand(&cell, c"${3}", m), b"arg3");
    // Out of range is unset (POSIX 2.6.2), as bash measures `${4}` and `${10}`.
    assert_eq!(expand(&cell, c"${4}", m), b"");
    assert_eq!(expand(&cell, c"${10}", m), b"");
    // Leading zeros parse as the same index; `0` is the program name.
    assert_eq!(expand(&cell, c"${01}", m), b"arg1");
    assert_eq!(expand(&cell, c"${001}", m), b"arg1");
    assert_eq!(expand(&cell, c"${0}", m), b"arg0");
    assert_eq!(expand(&cell, c"${00}", m), b"arg0");
    // The bare path shares `positional_value`, so the two forms cannot drift.
    assert_eq!(
        expand(&cell, c"${1}${2}$1$2${3}$3", m),
        b"arg1arg2arg1arg2arg3arg3"
    );
}

#[test]
fn braced_special_parameters_render() {
    let cell = positional_cell_with_n(4);
    cell.borrow_mut().unwrap().last_status = sys::siginfo::WaitStatus::Exited(7);
    let pid = alloc::format!("{}", cell.borrow().unwrap().shell_pid);
    let m = &[false; 8];
    assert_eq!(expand(&cell, c"${?}", m), b"7");
    assert_eq!(expand(&cell, c"${$}", m), pid.as_bytes());
    // `${@}` and `${*}` join the positionals with the first IFS byte (D1).
    assert_eq!(expand(&cell, c"${@}", m), b"arg1 arg2 arg3");
    assert_eq!(expand(&cell, c"${*}", m), b"arg1 arg2 arg3");
    cell.borrow_mut().unwrap().ifs = ShortCStr::from(c",");
    assert_eq!(expand(&cell, c"${*}", m), b"arg1,arg2,arg3");
    // `${-}` renders fdshell's own option flags (D2): `DEFAULTS` carries no
    // short flag, `-u` is `u`.
    assert_eq!(expand(&cell, c"${-}", m), b"");
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    assert_eq!(expand(&cell, c"${-}", m), b"u");
    // `${#}` in a word is the length arm (`brace.rs`), so the special reaches
    // the shared lookup directly: it is the positional count.
    let count = cell
        .borrow()
        .unwrap()
        .param_value(&ShortCStr::from(c"#"))
        .unwrap();
    assert_eq!(count.unwrap().as_bytes().unwrap(), b"3");
}

#[test]
fn braced_positional_nounset_bails_and_specials_never_do() {
    let cell = positional_cell_with_n(3);
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let e = substitute_arg(&ShortCStr::from(c"${5}"), &[], &mut HashMap::new(), &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"5"
    ));
    // The pattern family is nounset-checked too, and names the index (bash).
    let e = substitute_arg(
        &ShortCStr::from(c"${5#x}"),
        &[false; 6],
        &mut HashMap::new(),
        &cell,
    )
    .unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"5"
    ));
    // `$0` and every special are always bound: nounset never fires on them.
    for arg in [
        c"${0}", c"${#}", c"${?}", c"${$}", c"${@}", c"${*}", c"${-}",
    ] {
        assert!(
            substitute_arg(&ShortCStr::from(arg), &[], &mut HashMap::new(), &cell).is_ok(),
            "{arg:?} is always bound under `set -u`"
        );
    }
    // The colon family stays exempt: its word supplies the value.
    let m = &[false; 8];
    assert_eq!(expand(&cell, c"${5:-z}", m), b"z");
    assert_eq!(expand(&cell, c"${5:+p}", m), b"");
}

#[test]
fn param_name_predicate() {
    let cell = positional_cell();
    for name in [c"1", c"0", c"01", c"12", c"#", c"?", c"$", c"@", c"*", c"-"] {
        assert!(
            cell.borrow().unwrap().is_param_name(&ShortCStr::from(name)),
            "{name:?} is a parameter name"
        );
    }
    for name in [c"", c"1x", c"-1", c"a1", c"1 ", c"#a", c"_", c"u"] {
        assert!(
            !cell.borrow().unwrap().is_param_name(&ShortCStr::from(name)),
            "{name:?} is a variable name"
        );
    }
}

#[test]
fn colon_assign_refuses_positional_and_special_targets() {
    let cell = positional_cell_with_n(3);
    let m = &[false; 9];
    // An out-of-range positional has no slot to write: bash's message, rc 1,
    // and no variable named `5` is created.
    let e =
        substitute_arg(&ShortCStr::from(c"${5:=q}"), m, &mut HashMap::new(), &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::CannotAssign { var } if var.as_bytes().unwrap() == b"5"
    ));
    assert!(
        cell.borrow()
            .unwrap()
            .strings
            .get(&ShortCStr::from(c"5"))
            .is_none(),
        "the bail must leave no write behind"
    );
    // A parameter that holds a non-empty value keeps it: `:=` prints the value
    // and writes nothing (bash `${1:=q}` prints `$1`, `${@:=q}` prints `$@`).
    assert_eq!(expand(&cell, c"${1:=q}", m), b"arg1");
    assert_eq!(expand(&cell, c"${0:=q}", m), b"arg0");
    assert_eq!(expand(&cell, c"${@:=q}", m), b"arg1 arg2");
    // With no positional the value is empty, so the bail fires on `@` too.
    cell.borrow_mut()
        .unwrap()
        .set_positional(alloc::collections::VecDeque::new());
    let e =
        substitute_arg(&ShortCStr::from(c"${@:=q}"), m, &mut HashMap::new(), &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::CannotAssign { var } if var.as_bytes().unwrap() == b"@"
    ));
    // Non-parameter targets keep writing: task #258's indirect target `a b`.
    bind(&cell, c"p", c"a b");
    assert_eq!(expand(&cell, c"${!p:=q}", m), b"q");
    let written = cell
        .borrow()
        .unwrap()
        .strings
        .get(&ShortCStr::from(c"a b"))
        .unwrap()
        .value
        .clone();
    assert_eq!(written.as_bytes().unwrap(), b"q");
}

#[test]
fn pattern_arms_strip_a_positional_value() {
    let cell = positional_cell_with_n(4);
    let m = &[false; 7];
    assert_eq!(expand(&cell, c"${1#a}", m), b"rg1");
    assert_eq!(expand(&cell, c"${1#arg}", m), b"1");
    assert_eq!(expand(&cell, c"${3%3}", m), b"arg");
    assert_eq!(expand(&cell, c"${1##*}", m), b"");
    // Mask alignment with a bound positional: the pattern mask slice of
    // `${1#a*}` starts at word index 4 (the `{` index plus the name and the
    // operator byte), so bits 4..5 are the pattern bytes. Quoted, the pattern
    // is literal and nothing strips; unquoted, the shortest prefix `a` strips.
    let quoted_pat = [false, false, false, false, true, true, false];
    assert_eq!(expand(&cell, c"${1#a*}", &quoted_pat), b"arg1");
    assert_eq!(expand(&cell, c"${1#a*}", m), b"rg1");
    assert_eq!(expand(&cell, c"${1##a*}", &[false; 7]), b"");
}

#[test]
fn braced_digit_run_too_large_keeps_the_too_large_bail() {
    // The bare `$99999…` path is pinned by `tests/herestring.rs`; the braced
    // path shares `positional_value`, so it keeps the same bail (divergence
    // D5: bash expands the braced form to the empty string at rc 0).
    let cell = positional_cell();
    let e = substitute_arg(
        &ShortCStr::from(c"${99999999999999999999}"),
        &[],
        &mut HashMap::new(),
        &cell,
    )
    .unwrap_err();
    assert!(matches!(e.current_context(), ResolveError::TooLarge));
}

#[test]
fn braced_digit_run_never_reads_a_variable_of_the_same_name() {
    // A digit run is answered by the positional and nothing else. bash
    // `env 4=x bash -c 'set -- a b c; echo "[$4][${4}]"'` prints `[][]`: an
    // inherited variable literally named `4` is not positional 4, so the bare
    // and the braced form agree, and `:=` never writes a name `${4}` reads.
    let cell = positional_cell_with_n(4);
    bind(&cell, c"4", c"shell4");
    cell.borrow_mut()
        .unwrap()
        .environ
        .push((ShortCStr::from(c"4"), ShortCStr::from(c"env4")));
    let m = &[false; 20];
    // Out of range is unset (POSIX 2.6.2): empty, the entry shadowed away.
    assert_eq!(expand(&cell, c"${4}", m), b"");
    assert_eq!(expand(&cell, c"$4", m), b"");
    assert_eq!(expand(&cell, c"${4:+p}", m), b"");
    assert_eq!(expand(&cell, c"${4:-z}", m), b"z");
    assert_eq!(expand(&cell, c"${4#4}", m), b"");
    // `:=` refuses the target, so the variable named `4` keeps its value.
    let e =
        substitute_arg(&ShortCStr::from(c"${4:=q}"), m, &mut HashMap::new(), &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::CannotAssign { var } if var.as_bytes().unwrap() == b"4"
    ));
    let kept = cell
        .borrow()
        .unwrap()
        .strings
        .get(&ShortCStr::from(c"4"))
        .unwrap()
        .value
        .clone();
    assert_eq!(
        kept.as_bytes().unwrap(),
        b"shell4",
        "the bail leaves no write"
    );
    // `:?` bails on the unset positional (bash `4: m`, rc 1).
    let e =
        substitute_arg(&ShortCStr::from(c"${4:?m}"), m, &mut HashMap::new(), &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::ParamNullOrNotSet { var, word }
            if var.as_bytes().unwrap() == b"4" && word.as_bytes().unwrap() == b"m"
    ));
    // `set -u` reports the out-of-range index in every checked family, never
    // the inherited variable's name.
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    for arg in [c"${4}", c"$4", c"${4#x}"] {
        let e = substitute_arg(&ShortCStr::from(arg), m, &mut HashMap::new(), &cell).unwrap_err();
        assert!(
            matches!(
                e.current_context(),
                ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"4"
            ),
            "{arg:?} is unbound even with the inherited `4` present"
        );
    }
}

// ── task #223: a quoted expansion followed by literal text ────────────────
//
// Two bookkeeping facts compose the byte loss of `"$v"b,c`:
// * a byte a helper only peeks at must not count as consumed, or every
//   `realign` leaves `idx` one byte ahead of the mask (mask.rs's `Counting`);
// * a bare name is confined to the quote region of its trigger byte, exactly
//   as bash ends `"$v"bc` at the closing quote (`mask::region_len`).
// Every test below binds the name it probes: an unbound name expands to the
// empty string and the byte loss is invisible (LESSONS).
#[test]
fn region_len_measures_the_trigger_byte_region() {
    use super::mask::region_len;
    assert_eq!(region_len(&[true, true, false, false], 0), 2);
    assert_eq!(region_len(&[false, true, true], 1), 2);
    assert_eq!(region_len(&[true], 0), 1);
    // No bit at `from` is no information: the scan stays unbounded, which is
    // what the mask-independent callers (heredoc bodies, `case_exec`) need.
    assert_eq!(region_len(&[], 3), usize::MAX);
    // A run of equal bits is one region, so an all-quoted mask is unbounded.
    assert_eq!(region_len(&[true, true, true], 0), 3);
}

#[test]
fn counting_ignores_peeked_bytes() {
    // `consumed` is the index of the next *unread* byte: `realign` assigns it
    // to `idx`, so a peeked byte counted at peek time desynchronizes the mask.
    let mut peek = Counting::new(b"abc".iter().copied());
    assert_eq!(peek.peek().copied(), Some(b'a'));
    assert_eq!(peek.peek().copied(), Some(b'a'));
    assert_eq!(peek.consumed(), 0);
    assert_eq!(peek.next(), Some(b'a'));
    assert_eq!(peek.consumed(), 1);
    assert_eq!(peek.next(), Some(b'b'));
    assert_eq!(peek.consumed(), 2);
    assert_eq!(peek.peek().copied(), Some(b'c'));
    assert_eq!(peek.consumed(), 2);
    assert_eq!(peek.next(), Some(b'c'));
    assert_eq!(peek.consumed(), 3);
    assert_eq!(peek.peek(), None);
    assert_eq!(peek.next(), None);
    assert_eq!(peek.consumed(), 3);
}

#[test]
fn quoted_name_stops_at_the_closing_quote() {
    // The word of `"$vb",c` is `$vb,c` with the first two bytes quoted: the
    // parameter is `v`, and the literal `b` keeps its byte and its own bit.
    // bash `IFS=,; v=q; set -- "$v"b,c` → one field `qb,c` (no byte loss).
    let cell = dummy_cell();
    bind(&cell, c"v", c"W");
    bind(&cell, c"vb", c"VB");
    let (res, mask, expanded) = expand_marked(
        &cell,
        &ShortCStr::from(c"$vb,c"),
        &[true, true, false, false, false],
    );
    assert_eq!(res.as_bytes().unwrap(), b"Wb,c");
    assert_eq!(mask, vec![true, false, false, false]);
    assert!(expanded);
}

#[test]
fn name_is_greedy_inside_its_quote_region() {
    // `"x$vbc"`: every name byte is inside the `$`'s region, so the greedy
    // scan keeps the whole name (bash `$vbc` is the parameter `vbc`).
    let cell = dummy_cell();
    bind(&cell, c"v", c"V");
    bind(&cell, c"vbc", c"VBC");
    assert_eq!(
        expand(&cell, c"x$vbc", &[true, true, true, true, true]),
        b"xVBC"
    );
    // Crossing out of the region is what the cap forbids: `"x$v"bc` is `xVbc`.
    bind(&cell, c"vb", c"VB");
    assert_eq!(
        expand(&cell, c"x$vbc", &[true, true, true, false, false, false]),
        b"xVbc"
    );
}

#[test]
fn positional_digit_run_stops_at_the_closing_quote() {
    // `"$1"2` is `$1` plus the literal `2` (bash `[A2]`), not positional 12.
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"A"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    assert_eq!(expand(&cell, c"$12", &[true, true, false]), b"A2");
    // A word with no mask at all stays mask-independent: greedy digits.
    assert_eq!(expand(&positional_cell_with_n(15), c"$12", &[]), b"arg12");
}

#[test]
fn percent_name_stops_at_the_closing_quote() {
    // `"%v"b`: the fd name is capped at the region, the literal `b` follows.
    // A seeded cache entry fixes the fd number, so the bytes are exact.
    let cell = dummy_cell();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    cache.insert(ShortCStr::from(c"v"), exported_fd(1000));
    let (res, mask, expanded) = expand_marked_cached(
        &cell,
        &mut cache,
        &ShortCStr::from(c"%vb"),
        &[true, true, false],
    );
    assert_eq!(res.as_bytes().unwrap(), b"1000b");
    assert_eq!(mask, vec![true, true, true, true, false]);
    assert!(expanded);
}

#[test]
fn percent_at_region_end_is_literal_and_consumes_nothing() {
    // `"%v"` region ends at the `%`: the name is empty, so the arm is the
    // literal `%` escape and the `v` is handed to the loop as a literal byte.
    let cell = dummy_cell();
    let (res, mask, expanded) = expand_marked(&cell, &ShortCStr::from(c"%v"), &[true, false]);
    assert_eq!(res.as_bytes().unwrap(), b"%v");
    assert_eq!(mask, vec![true, false]);
    assert!(
        expanded,
        "the `%` arm ran, so its byte reaches the splitter"
    );
}

#[test]
fn dollar_at_region_end_is_literal() {
    // `"$"v`: the byte after `$` starts a new region, so `$` is literal and
    // nothing is consumed — the same rule `$'…'` and `$"…"` already follow
    // (bash `v`, README accepted divergence). The literal `$` is arm output,
    // so the word reaches the splitter (`bare_dollar_marks_expansion` pins
    // the same reading; the literal-delimiter join is task #222).
    let cell = dummy_cell();
    bind(&cell, c"v", c"V");
    let (res, mask, expanded) = expand_marked(&cell, &ShortCStr::from(c"$v"), &[true, false]);
    assert_eq!(res.as_bytes().unwrap(), b"$v");
    assert_eq!(mask, vec![true, false]);
    assert!(expanded);
}

/// The arm as the pipeline runs it, handing back the full provenance.
fn expand_origin(
    cell: &ForkCell<ShellState>,
    word: &ShortCStr,
    mask: &[bool],
) -> (ShortCStr, Vec<bool>, Origin) {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    substitute_arg_marked(word, mask, &mut cache, cell).unwrap()
}

fn splittable_all(origin: &Origin, len: usize) -> Vec<bool> {
    (0..len).map(|i| origin.splittable(i)).collect()
}

// Per-arm provenance (task #222): only the arm's own output bytes are
// splittable; the tilde arm records none, and the gaps around every span
// are literal.
#[test]
fn tilde_arm_records_no_splittable_bytes() {
    let cell = dummy_cell();
    let (res, _mask, origin) = expand_origin(&cell, &ShortCStr::from(c"~"), &[]);
    let home = std::env::var("HOME").unwrap();
    assert_eq!(res.as_bytes().unwrap(), home.as_bytes());
    assert!(splittable_all(&origin, res.len()).iter().all(|&s| !s));
}

#[test]
fn expansion_arms_record_only_their_output() {
    let cell = dummy_cell();
    bind(&cell, c"w", c"B C");
    let cases: &[(&core::ffi::CStr, &[bool])] = &[
        // `$name`: the literal `x` and `,y` are gaps, the value is the span.
        (c"x$w,y", &[false, true, true, true, false, false]),
        // `${name}` produces the same span shape.
        (c"x${w},y", &[false, true, true, true, false, false]),
        // `$(( … ))` output splits like any expansion.
        (c"x$((1+1)),y", &[false, true, false, false]),
        // `$( … )` output is splittable; the literals around it are not.
        (
            c"x$(printf \"B C\"),y",
            &[false, true, true, true, false, false],
        ),
        // A fully quoted expansion keeps every byte protected by the mask;
        // the provenance still marks the span (the mask is what blocks). The
        // harness text keeps the literal quote bytes (gaps here, unmarked).
        (
            c"x\"$w\",y",
            &[false, false, true, true, true, false, false, false],
        ),
    ];
    for (word, want) in cases {
        let (res, mask, origin) = expand_origin(&cell, &ShortCStr::from(*word), &[]);
        let bytes = res.as_bytes().unwrap();
        assert_eq!(bytes.len(), mask.len(), "{word:?}: mask parallel to text");
        assert!(!origin.splittable(bytes.len()), "{word:?}: origin closed");
        assert_eq!(&splittable_all(&origin, bytes.len()), want, "{word:?}");
    }
}

#[test]
fn percent_arm_records_its_fd_number() {
    let cell = dummy_cell();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    cache.insert(ShortCStr::from(c"v"), exported_fd(1000));
    let (res, _mask, origin) =
        substitute_arg_marked(&ShortCStr::from(c"x%v,y"), &[false; 5], &mut cache, &cell).unwrap();
    assert_eq!(res.as_bytes().unwrap(), b"x1000,y");
    assert_eq!(
        splittable_all(&origin, res.len()),
        vec![false, true, true, true, true, false, false]
    );
}

#[test]
fn dollar_at_arm_unquoted_joins_with_the_ifs_byte() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"a b", c"c"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    // `x$@y` unquoted: bash joins the positionals with the first IFS byte
    // and the splitter cuts that byte (`IFS=:; set -- a b; printf %s x$@`
    // → `[xa][b]`), so the arm records one span and no boundaries.
    let (res, mask, origin) = expand_origin(&cell, &ShortCStr::from(c"x$@y"), &[]);
    assert_eq!(res.as_bytes().unwrap(), b"xa b cy");
    assert_eq!(mask.len(), res.len());
    assert_eq!(
        splittable_all(&origin, res.len()),
        vec![false, true, true, true, true, true, false]
    );
    assert!(
        origin.boundaries_at(4).is_empty(),
        "the separator is the boundary"
    );
    assert!(!origin.ends_quoted(res.len()));
}

#[test]
fn dollar_at_arm_quoted_records_a_span_and_a_boundary_per_positional() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"a b", c"c"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    // `x"$@"y`: no separator (bash emits none in a quote region), spans on
    // both positionals, a quoted boundary at the second's start. The harness
    // keeps the quote bytes in the expanded text; the mask marks the quoted
    // region, so the trigger byte's bit selects the quoted arm.
    let (res, mask, origin) = expand_origin(
        &cell,
        &ShortCStr::from(c"x\"$@\"y"),
        &[false, true, true, true, true, false],
    );
    assert_eq!(res.as_bytes().unwrap(), b"x\"a bc\"y");
    assert_eq!(mask.len(), res.len());
    assert_eq!(
        splittable_all(&origin, res.len()),
        vec![false, false, true, true, true, true, false, false]
    );
    assert!(
        origin.boundaries_at(1).is_empty(),
        "the first positional has no boundary"
    );
    assert_eq!(
        origin.boundaries_at(5),
        [(5, true)],
        "boundary at the second's start"
    );
    assert!(!origin.ends_quoted(res.len()));
}

#[test]
fn dollar_star_arm_records_one_span_and_no_boundary() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_positional(
        [c"a", c"b"]
            .into_iter()
            .map(ShortCStr::from)
            .map(ImportedStr::shell)
            .collect(),
    );
    let (res, _mask, origin) = expand_origin(&cell, &ShortCStr::from(c"x$*y"), &[]);
    assert_eq!(res.as_bytes().unwrap(), b"xa by");
    assert_eq!(
        splittable_all(&origin, res.len()),
        vec![false, true, true, true, false]
    );
    assert!(
        origin.boundaries_at(2).is_empty(),
        "$* is one joined string"
    );
}

/// An `ExportedFd` with a fixed raw number, so a `%name` pin asserts exact
/// bytes. `export_to` is `dup2`, whose kernel contract is to return `raw`; the
/// source's `LocalFd` closing afterwards leaves the duplicate open.
fn exported_fd(raw: i32) -> ExportedFd {
    let (src, _dst) = sys::pipe::pipe2(0).unwrap();
    src.export_to(raw).unwrap()
}
