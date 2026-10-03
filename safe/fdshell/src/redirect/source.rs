use alloc::vec::Vec;
use sys::ShortCStr;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RedirectSource {
    Var(ShortCStr),
    /// A filesystem path word. `mask` is the per-byte quote mask: which
    /// pattern bytes are quoted (unquoted `*`/`?`/`[...]` glob the target).
    Path {
        path: ShortCStr,
        mask: Vec<bool>,
    },
    HereString(ShortCStr),
    /// Here-doc (`<<EOF`): the body bytes become the stdin of the command.
    /// `expand` runs `$` / backtick expansion in the body (unquoted delimiter).
    HereDoc {
        body: ShortCStr,
        expand: bool,
    },
    /// Dup from an already-open fd number (`2>&1`).
    Dup(i32),
    /// Close the target fd (`2>&-`).
    Close,
}

impl RedirectSource {
    pub fn var(name: impl Into<ShortCStr>) -> Self {
        Self::Var(name.into())
    }
    pub fn path(name: impl Into<ShortCStr>, mask: Vec<bool>) -> Self {
        Self::Path {
            path: name.into(),
            mask,
        }
    }
    pub fn here_string(word: impl Into<ShortCStr>) -> Self {
        Self::HereString(word.into())
    }
    pub fn here_doc(body: impl Into<ShortCStr>, expand: bool) -> Self {
        Self::HereDoc {
            body: body.into(),
            expand,
        }
    }
    pub fn dup(from: i32) -> Self {
        Self::Dup(from)
    }
    pub fn close() -> Self {
        Self::Close
    }
}
