use alloc::vec::Vec;
use sys::ShortCStr;

use super::{RedirectDirection, RedirectSource};

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct RedirectDef {
    pub export_to: i32,
    pub direction: RedirectDirection,
    pub source: RedirectSource,
}

impl RedirectDef {
    /// The highest fd this command redirects to. Redirect sources must be
    /// cloned/duplicated above it, otherwise a source landing on a target fd
    /// turns the `dup2` into a no-op whose source drop then closes the target.
    pub fn max_target(redirects: &[RedirectDef]) -> i32 {
        redirects.iter().map(|r| r.export_to).max().unwrap_or(0)
    }

    /// `noclobber` applies to a plain write (`>`); `>|` is the bypass (POSIX
    /// #2.2), so only the `Write` direction is gated (redirect/open.rs).
    pub fn write_flag(&self) -> bool {
        matches!(self.direction, RedirectDirection::Write)
    }

    pub fn var(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Write,
            source: RedirectSource::var(name),
        }
    }

    pub fn read_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Read,
            source: RedirectSource::path(name, Vec::new()),
        }
    }

    pub fn write_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Write,
            source: RedirectSource::path(name, Vec::new()),
        }
    }

    pub fn append_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
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
            direction: RedirectDirection::Read,
            source: RedirectSource::here_doc(body, expand),
        }
    }

    /// Dup redirect: copy the already-open fd `from` to `export_to` (`2>&1`).
    pub fn dup(export_to: i32, from: i32) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Read,
            source: RedirectSource::dup(from),
        }
    }

    /// Dup-from-variable redirect: copy the fd the fd variable `name` refers
    /// to into `export_to` (`2>&%f`). Resolves like `>%name`.
    pub fn dup_var(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Read,
            source: RedirectSource::var(name),
        }
    }

    /// Close redirect: drop fd `export_to` (`2>&-`).
    pub fn close(export_to: i32) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Read,
            source: RedirectSource::close(),
        }
    }
}
