#![allow(clippy::unwrap_used)]

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::glob::expand;
use crate::options::NULLGLOB;
use crate::state::ShellState;

static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// A scratch dir with `a1 a2 b1 x .hidden file sub/{alpha,beta,gamma}` (the
/// single-char `x` exercises `?`); each test gets its own (unit-test cwd is
/// the repo root, so patterns below are absolute).
fn scratch() -> String {
    let c = COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-glob-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    for name in ["a1", "a2", "b1", "x", ".hidden", "file"] {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    for name in ["alpha", "beta", "gamma"] {
        std::fs::write(dir.join("sub").join(name), b"").unwrap();
    }
    String::from(dir.to_str().unwrap())
}

fn word(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn expand_to_bytes(cell: &ForkCell<ShellState>, w: &str) -> Vec<Vec<u8>> {
    expand(&word(w), &[], cell)
        .unwrap()
        .iter()
        .map(|s| s.as_bytes().unwrap().to_vec())
        .collect()
}

#[test]
fn star_matches_sorted_and_excludes_dots() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
    let out = expand_to_bytes(&cell, &format!("{dir}/*"));
    assert_eq!(
        out,
        vec![
            format!("{dir}/a1").into_bytes(),
            format!("{dir}/a2").into_bytes(),
            format!("{dir}/b1").into_bytes(),
            format!("{dir}/file").into_bytes(),
            format!("{dir}/sub").into_bytes(),
            format!("{dir}/x").into_bytes(),
        ]
    );
}

#[test]
fn non_pattern_word_passes_through_fs_free() {
    let cell = ForkCell::new(ShellState::new());
    assert_eq!(expand_to_bytes(&cell, "plain"), vec![b"plain".to_vec()]);
    // The empty word is one empty word, never a pattern.
    assert_eq!(expand_to_bytes(&cell, ""), vec![Vec::<u8>::new()]);
}

#[test]
fn no_match_keeps_word_verbatim_by_default() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/zzz*")),
        vec![format!("{dir}/zzz*").into_bytes()]
    );
}

#[test]
fn no_match_disappears_with_nullglob() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().options |= NULLGLOB;
    let dir = scratch();
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/zzz*")),
        Vec::<Vec<u8>>::new()
    );
    // Matches are unaffected by nullglob.
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/a*")),
        vec![
            format!("{dir}/a1").into_bytes(),
            format!("{dir}/a2").into_bytes()
        ]
    );
}

#[test]
fn question_matches_one_char_but_not_dots() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
    // `?` matches the single-char `x`, never `.`/`..`/`.hidden` (FNM_PERIOD).
    let names = expand_to_bytes(&cell, &format!("{dir}/?"));
    assert_eq!(names, vec![format!("{dir}/x").into_bytes()]);
    assert!(!names.iter().any(|n| n.last() == Some(&b'.')));
}

#[test]
fn dot_leading_patterns_never_match_dot_entries() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
    // getdents also returns `.`/`..`; the walk drops them, so `.*` lists
    // only the real dotfiles (bash: `echo .*` never prints `.`/`..`).
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/.*")),
        vec![format!("{dir}/.hidden").into_bytes()]
    );
    // A dot pattern with no real match stays verbatim: `..` must not count.
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/.?")),
        vec![format!("{dir}/.?").into_bytes()]
    );
}

#[test]
fn trailing_slash_yields_directories_only() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/*/")),
        vec![format!("{dir}/sub/").into_bytes()]
    );
    // `file` is not a directory: no match, verbatim word (no nullglob).
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/file/")),
        vec![format!("{dir}/file/").into_bytes()]
    );
}

#[test]
fn nested_pattern_walks_intermediate_components() {
    let cell = ForkCell::new(ShellState::new());
    let dir = scratch();
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/sub/*")),
        vec![
            format!("{dir}/sub/alpha").into_bytes(),
            format!("{dir}/sub/beta").into_bytes(),
            format!("{dir}/sub/gamma").into_bytes(),
        ]
    );
    assert_eq!(
        expand_to_bytes(&cell, &format!("{dir}/*/gamma")),
        vec![format!("{dir}/sub/gamma").into_bytes()]
    );
}
