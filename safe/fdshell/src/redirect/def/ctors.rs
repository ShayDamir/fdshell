//! The `RedirectDef` constructors, one per redirect operator. They live in
//! this module to keep `def.rs` (the struct and its rules) within the §2.2
//! line budget; inherent `impl` blocks may live in any module of the crate.

use alloc::vec::Vec;
use sys::ShortCStr;

use super::RedirectDef;
use crate::mask::MaskBit;
use crate::redirect::{RedirectDirection, RedirectSource};

impl RedirectDef {
    /// The single place a plain single-descriptor redirect is assembled:
    /// `extra_to` is `None` here and is set only by `RedirectDef::combined`
    /// (`def.rs`).
    fn opened(export_to: i32, direction: RedirectDirection, source: RedirectSource) -> Self {
        RedirectDef {
            export_to,
            extra_to: None,
            direction,
            source,
        }
    }

    /// A path-source redirect: file `name` opened in `direction`, with no
    /// trailing expansion bytes.
    fn path(export_to: i32, direction: RedirectDirection, name: impl Into<ShortCStr>) -> Self {
        Self::opened(export_to, direction, RedirectSource::path(name, Vec::new()))
    }

    pub fn var(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        Self::opened(
            export_to,
            RedirectDirection::Write,
            RedirectSource::var(name),
        )
    }

    pub fn read_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        Self::path(export_to, RedirectDirection::Read, name)
    }

    pub fn write_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        Self::path(export_to, RedirectDirection::Write, name)
    }

    pub fn append_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        Self::path(export_to, RedirectDirection::Append, name)
    }

    /// Here-string: the expanded `word` (with its own quote mask) becomes the
    /// `export_to` descriptor of the command — `export_to` is 0 for `<<<word`
    /// (stdin) and the io-number of the `N<<<word` form.
    pub fn here_string(
        export_to: i32,
        word: impl Into<ShortCStr>,
        mask: Vec<MaskBit>,
        spans: Vec<usize>,
    ) -> Self {
        Self::opened(
            export_to,
            RedirectDirection::Read,
            RedirectSource::here_string(word, mask, spans),
        )
    }

    /// Here-doc: the body (expanded when `expand`) becomes the
    /// `export_to` descriptor of the command — `export_to` is 0 for `<<T`
    /// (stdin) and the io-number of the `N<<T` form.
    pub fn here_doc(export_to: i32, body: impl Into<ShortCStr>, expand: bool) -> Self {
        Self::opened(
            export_to,
            RedirectDirection::Read,
            RedirectSource::here_doc(body, expand),
        )
    }

    /// Dup redirect: copy the already-open fd `from` to `export_to` (`2>&1`).
    pub fn dup(export_to: i32, from: i32) -> Self {
        Self::opened(
            export_to,
            RedirectDirection::Read,
            RedirectSource::dup(from),
        )
    }

    /// Dup-from-variable redirect: copy the fd the fd variable `name` refers
    /// to into `export_to` (`2>&%f`). Resolves like `>%name`.
    pub fn dup_var(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        Self::opened(
            export_to,
            RedirectDirection::Read,
            RedirectSource::var(name),
        )
    }

    /// Close redirect: drop fd `export_to` (`2>&-`).
    pub fn close(export_to: i32) -> Self {
        Self::opened(export_to, RedirectDirection::Read, RedirectSource::close())
    }
}
