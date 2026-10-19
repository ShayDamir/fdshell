//! The `RedirectDef` constructors, one per redirect operator. They live in
//! this module to keep `def.rs` (the struct and its rules) within the §2.2
//! line budget; inherent `impl` blocks may live in any module of the crate.

use alloc::vec::Vec;
use sys::ShortCStr;

use super::RedirectDef;
use crate::redirect::{RedirectDirection, RedirectSource};

impl RedirectDef {
    pub fn var(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Write,
            source: RedirectSource::var(name),
        }
    }

    pub fn read_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::path(name, Vec::new()),
        }
    }

    pub fn write_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Write,
            source: RedirectSource::path(name, Vec::new()),
        }
    }

    pub fn append_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Append,
            source: RedirectSource::path(name, Vec::new()),
        }
    }

    /// Here-string: the expanded `word` (with its own quote mask) becomes the
    /// `export_to` descriptor of the command — `export_to` is 0 for `<<<word`
    /// (stdin) and the io-number of the `N<<<word` form.
    pub fn here_string(
        export_to: i32,
        word: impl Into<ShortCStr>,
        mask: Vec<bool>,
        spans: Vec<usize>,
    ) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::here_string(word, mask, spans),
        }
    }

    /// Here-doc: the body (expanded when `expand`) becomes the
    /// `export_to` descriptor of the command — `export_to` is 0 for `<<T`
    /// (stdin) and the io-number of the `N<<T` form.
    pub fn here_doc(export_to: i32, body: impl Into<ShortCStr>, expand: bool) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::here_doc(body, expand),
        }
    }

    /// Dup redirect: copy the already-open fd `from` to `export_to` (`2>&1`).
    pub fn dup(export_to: i32, from: i32) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::dup(from),
        }
    }

    /// Dup-from-variable redirect: copy the fd the fd variable `name` refers
    /// to into `export_to` (`2>&%f`). Resolves like `>%name`.
    pub fn dup_var(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::var(name),
        }
    }

    /// Close redirect: drop fd `export_to` (`2>&-`).
    pub fn close(export_to: i32) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction: RedirectDirection::Read,
            source: RedirectSource::close(),
        }
    }
}
