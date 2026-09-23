//! Split a pattern word into path components on unquoted `/`.

use alloc::vec::Vec;

use crate::glob::r#match as m;

/// One pattern component: its bytes, the parallel quote mask, and whether it
/// holds any unquoted pattern byte (literal components are opened directly
/// instead of listing the directory).
pub(super) struct Comp {
    pub(super) pat: Vec<u8>,
    pub(super) mask: Vec<bool>,
    pub(super) literal: bool,
}

pub(super) struct Parts {
    pub(super) comps: Vec<Comp>,
    pub(super) absolute: bool,
    pub(super) dir_only: bool,
}

/// Split the word on unquoted `/`. Quoted bytes, unquoted `\X` pairs, and
/// whole valid bracket spans never split. Empty components are dropped
/// (`a//b` ≡ `a/b`); a leading `/` marks the pattern absolute and a trailing
/// unquoted `/` demands directories only.
pub(super) fn split(bytes: &[u8], mask: &[bool]) -> Parts {
    let mut comps = Vec::new();
    let mut absolute = false;
    let mut start = 0usize;
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        if m::bracket::quoted(mask, i) {
            i += 1;
            continue;
        }
        match b {
            b'/' => {
                if i > start {
                    comps.push(component(bytes, mask, start, i));
                }
                if i == 0 {
                    absolute = true;
                }
                start = i + 1;
                i += 1;
            }
            b'\\' => i += 2,
            b'[' => i = m::bracket::span(bytes, mask, i).unwrap_or(i + 1),
            _ => i += 1,
        }
    }
    if start < bytes.len() {
        comps.push(component(bytes, mask, start, bytes.len()));
    }
    Parts {
        comps,
        absolute,
        dir_only: start == bytes.len(),
    }
}

fn component(bytes: &[u8], mask: &[bool], start: usize, end: usize) -> Comp {
    let pat = bytes.get(start..end).unwrap_or(b"").to_vec();
    let mask = mask.get(start..end).unwrap_or(&[]).to_vec();
    let literal = !m::has_unquoted_pattern(&pat, &mask);
    Comp { pat, mask, literal }
}
