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
            source: RedirectSource::path(name),
        }
    }

    pub fn write_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Write,
            source: RedirectSource::path(name),
        }
    }

    pub fn append_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Append,
            source: RedirectSource::path(name),
        }
    }

    /// Here-string: the expanded `word` becomes the stdin of the command.
    pub fn here_string(word: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to: 0,
            direction: RedirectDirection::Read,
            source: RedirectSource::here_string(word),
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

    /// Close redirect: drop fd `export_to` (`2>&-`).
    pub fn close(export_to: i32) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Read,
            source: RedirectSource::close(),
        }
    }
}
