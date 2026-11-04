#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use alloc::vec;
use alloc::vec::Vec;

use hashbrown::HashMap;

use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::mask::MaskBit::{Quoted, Unquoted};
use crate::state::ShellState;
use crate::substitute::origin::Origin;

use super::braced_arm::adjacent;
use super::{Shape, fields, join, len, shape, whole_word};

/// A word with one mask bit per byte, the shape the fast path claims.
fn word(text: &[u8], bit: MaskBit) -> (ShortCStr, Vec<MaskBit>) {
    (bytes(text), vec![bit; text.len()])
}

fn bytes(text: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(text.to_vec()).unwrap()
}

fn shell(text: &[u8]) -> ImportedStr {
    ImportedStr::shell(bytes(text))
}

/// A state whose `a` holds `elements` (bash: `a=(…)`).
fn cell_with(name: &[u8], elements: &[&[u8]]) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_str_array(
        &bytes(name),
        elements.iter().map(|e| shell(e)).collect::<Vec<_>>(),
    );
    cell
}

fn bound_scalar(text: &[u8]) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_var(bytes(b"a"), shell(text));
    cell
}

/// The field texts one word expands to — the shape the caller globs.
fn fields_of(
    word: &ShortCStr,
    mask: &[MaskBit],
    quoted: bool,
    cell: &ForkCell<ShellState>,
) -> Vec<ShortCStr> {
    let mut cache = HashMap::new();
    crate::substitute::word_fields(word, mask, &[], quoted, &mut cache, cell)
        .unwrap()
        .into_iter()
        .map(|(field, _)| field)
        .collect()
}

#[test]
fn the_whole_word_shape_claims_only_the_list_reads() {
    let (w, mask) = word(b"${a[@]}", Unquoted);
    assert_eq!(whole_word(&w, &mask), Some((bytes(b"a"), false)));
    let (w, mask) = word(b"${a[*]}", Quoted);
    assert_eq!(whole_word(&w, &mask), Some((bytes(b"a"), true)));
    // A single element and a plain name are not the fast path's shape.
    let (w, mask) = word(b"${a[1]}", Unquoted);
    assert_eq!(whole_word(&w, &mask), None);
    let (w, mask) = word(b"${a}", Unquoted);
    assert_eq!(whole_word(&w, &mask), None);
    // The region guard (the `$@` guard's analogue): a word whose last byte sits
    // outside the trigger byte's quote region is the general path's.
    let (w, mut mask) = word(b"${a[@]}x", Quoted);
    mask[7] = Unquoted;
    assert_eq!(whole_word(&w, &mask), None);
    // One substitution only: the first `}` closes the brace, and a `#`/`!`
    // content is the length or the indirect arm.
    assert_eq!(whole_word(&bytes(b"${a[@]}-${b[@]}"), &[]), None);
    assert_eq!(whole_word(&bytes(b"${#a[@]}"), &[]), None);
    assert_eq!(whole_word(&bytes(b"${!a[@]}"), &[]), None);
    assert_eq!(
        whole_word(&bytes(b"${a[*]}"), &[]),
        Some((bytes(b"a"), true))
    );
    assert_eq!(
        whole_word(&bytes(b"${a[@]}"), &[]),
        Some((bytes(b"a"), false))
    );
}

#[test]
fn a_quoted_whole_word_at_is_one_field_per_element() {
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    let (w, mask) = word(b"${a[@]}", Quoted);
    assert_eq!(
        fields_of(&w, &mask, true, &cell),
        vec![bytes(b"b c"), bytes(b"d")]
    );
}

#[test]
fn a_quoted_whole_word_star_is_one_joined_field() {
    // `[*]` joins with the first `IFS` byte, `[@]` keeps the elements apart —
    // the positional pair's rule, and the two spellings agree.
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    let (w, mask) = word(b"${a[*]}", Quoted);
    assert_eq!(fields_of(&w, &mask, true, &cell), vec![bytes(b"b c d")]);
}

#[test]
fn an_ifs_of_nothing_concatenates_the_join() {
    // bash: `IFS=; a=("b c" d); printf "[%s]" "${a[*]}"` → `[b cd]`.
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    cell.borrow_mut().unwrap().ifs = ShortCStr::new();
    let (w, mask) = word(b"${a[*]}", Quoted);
    assert_eq!(fields_of(&w, &mask, true, &cell), vec![bytes(b"b cd")]);
}

#[test]
fn an_unquoted_whole_word_splits_each_element_separately() {
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    let (w, mask) = word(b"${a[@]}", Unquoted);
    assert_eq!(
        fields_of(&w, &mask, false, &cell),
        vec![bytes(b"b"), bytes(b"c"), bytes(b"d")]
    );
}

#[test]
fn the_empty_array_keeps_the_positional_contract() {
    // A fully quoted `[@]` *is* the element list, so an empty array is no field
    // at all (task #219); a fully quoted `[*]` is its single joined word, empty.
    let cell = cell_with(b"a", &[]);
    let (w, mask) = word(b"${a[@]}", Quoted);
    assert!(fields_of(&w, &mask, true, &cell).is_empty());
    let (w, mask) = word(b"${a[*]}", Quoted);
    assert_eq!(fields_of(&w, &mask, true, &cell), vec![ShortCStr::new()]);
    // Unquoted, an empty array is no field either, and a partly quoted word that
    // carried quotes keeps the one empty field (the `push_quoted_empty` rule).
    let (w, mask) = word(b"${a[@]}", Unquoted);
    assert!(fields_of(&w, &mask, false, &cell).is_empty());
    assert!(
        fields(&bytes(b"a"), false, false, false, &cell)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fields(&bytes(b"a"), false, false, true, &cell)
            .unwrap()
            .len(),
        1
    );
}

/// The embedded arm and the whole-word path, pinned side by side (LESSONS
/// "`$@` has two join rules"): the boundary model for a quoted `[@]`, the IFS
/// join for `[*]`.
#[test]
fn the_embedded_arm_splits_at_the_element_boundaries() {
    // bash: `printf "[%s]" "x ${a[@]} y"` → `[x b c][d y]`.
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    let (w, mask) = word(b"x ${a[@]} y", Quoted);
    assert_eq!(
        fields_of(&w, &mask, true, &cell),
        vec![bytes(b"x b c"), bytes(b"d y")]
    );
}

#[test]
fn the_embedded_star_join_stays_one_field() {
    // bash: `printf "[%s]" "x ${a[*]} y"` → `[x b c d y]`.
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    let (w, mask) = word(b"x ${a[*]} y", Quoted);
    assert_eq!(fields_of(&w, &mask, true, &cell), vec![bytes(b"x b c d y")]);
}

#[test]
fn the_embedded_element_read_walks_the_one_namespace_rule() {
    // The store's element, and the scalar answering index 0.
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    let (w, mask) = word(b"${a[1]}", Unquoted);
    assert_eq!(fields_of(&w, &mask, false, &cell), vec![bytes(b"d")]);
    let cell = bound_scalar(b"hello");
    let (w, mask) = word(b"${a[0]}|${a[1]}", Unquoted);
    assert_eq!(fields_of(&w, &mask, false, &cell), vec![bytes(b"hello|")]);
}

#[test]
fn the_length_arm_answers_count_and_element_length() {
    // bash: `a=("b c" d); echo "${#a[0]} ${#a[1]} ${#a[@]}"` → `3 1 2`.
    let cell = cell_with(b"a", &[b"b c", b"d"]);
    assert_eq!(len(&bytes(b"a[0]"), &cell).unwrap(), Some(3));
    assert_eq!(len(&bytes(b"a[1]"), &cell).unwrap(), Some(1));
    assert_eq!(len(&bytes(b"a[@]"), &cell).unwrap(), Some(2));
    assert_eq!(len(&bytes(b"a[*]"), &cell).unwrap(), Some(2));
    // The scalar bridge (bash: `a=hello; echo "${#a[@]} ${#a[0]}"` → `1 5`).
    let cell = bound_scalar(b"hello");
    assert_eq!(len(&bytes(b"a[@]"), &cell).unwrap(), Some(1));
    assert_eq!(len(&bytes(b"a[0]"), &cell).unwrap(), Some(5));
    // An unresolved element is the caller's reading, not this arm's; and so is
    // an ordinary name.
    assert_eq!(len(&bytes(b"a[1]"), &cell).unwrap(), None);
    assert_eq!(len(&bytes(b"a"), &cell).unwrap(), None);
}

#[test]
fn the_shape_reads_every_array_spelling() {
    assert!(matches!(
        shape(&bytes(b"a[@]")),
        Some(Shape::All { name, star: false }) if name == bytes(b"a")
    ));
    assert!(matches!(
        shape(&bytes(b"a[*]")),
        Some(Shape::All { star: true, .. })
    ));
    assert!(matches!(
        shape(&bytes(b"a[12]")),
        Some(Shape::Element { index: 12, .. })
    ));
    // `array_ref::split_index_ref` owns the `[digits]` read; a non-numeric tail
    // and an empty name are no array shape.
    assert_eq!(shape(&bytes(b"a[x]")), None);
    assert_eq!(shape(&bytes(b"[@]")), None);
}

#[test]
fn the_join_and_adjacent_helpers_agree_with_their_positional_twins() {
    // `[*]`: joined by the first `IFS` byte, concatenated when there is none;
    // a quoted `[@]`: adjacent, with a boundary between (the boundaries are what
    // the field tests above cut on).
    let parts = vec![bytes(b"x"), bytes(b"y")];
    assert_eq!(join(&parts, Some(b',')).unwrap(), bytes(b"x,y"));
    assert_eq!(join(&parts, None).unwrap(), bytes(b"xy"));
    let mut out = ShortCStr::new();
    adjacent(&parts, &mut out, &mut Origin::new());
    assert_eq!(out, bytes(b"xy"));
}

#[test]
fn nounset_bails_on_an_unresolved_element_only() {
    // bash: `set -u; a=(1); echo "${a[3]}"` → `a[3]: unbound variable`, while an
    // unset name's `[@]` expands empty at rc 0.
    let cell = cell_with(b"a", &[b"1"]);
    cell.borrow_mut().unwrap().options |= crate::options::NOUNSET;
    let (w, mask) = word(b"${a[3]}", Unquoted);
    let mut cache = HashMap::new();
    let e = crate::substitute::word_fields(&w, &mask, &[], false, &mut cache, &cell).unwrap_err();
    assert!(matches!(
        e.current_context(),
        ResolveError::UnboundVariable { var } if var.as_bytes().unwrap() == b"a[3]"
    ));
    let (w, mask) = word(b"${q[@]}", Unquoted);
    assert!(
        crate::substitute::word_fields(&w, &mask, &[], false, &mut cache, &cell)
            .unwrap()
            .is_empty()
    );
}
