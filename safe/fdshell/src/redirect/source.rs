use alloc::vec::Vec;
use sys::ShortCStr;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RedirectSource {
    Var(ShortCStr),
    /// A filesystem path word. `mask` is the per-byte quote mask: which
    /// pattern bytes are quoted (unquoted `*`/`?`/`[...]` glob the target).
    /// `spans` is the parallel zero-byte quote-span list (task #321) and
    /// `quoted` says the word's raw span carried quotes — both feed the word
    /// expansion the target goes through (task #278): the tilde rule reads
    /// `spans`, `quoted` keeps one empty word when the expansion is empty.
    Path {
        path: ShortCStr,
        mask: Vec<bool>,
        spans: Vec<usize>,
        quoted: bool,
    },
    /// The here-string word of `<<<`. `mask` is the word's per-byte quote
    /// mask: the word is expanded with its own quoting, so a `\<char>` written
    /// inside double quotes keeps its backslash (task #269). `spans` is the
    /// parallel zero-byte quote-span list (task #321).
    HereString {
        word: ShortCStr,
        mask: Vec<bool>,
        spans: Vec<usize>,
    },
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
            spans: Vec::new(),
            quoted: false,
        }
    }
    /// The target word as the parser derived it (`parse/redirect/path_word.rs`):
    /// the word, its own quote mask, its zero-byte quote spans and its
    /// `quoted` flag — the same quadruple an argument word carries, so the
    /// target expands exactly like an argument (task #278).
    pub fn path_word(
        path: impl Into<ShortCStr>,
        mask: Vec<bool>,
        spans: Vec<usize>,
        quoted: bool,
    ) -> Self {
        Self::Path {
            path: path.into(),
            mask,
            spans,
            quoted,
        }
    }
    pub fn here_string(word: impl Into<ShortCStr>, mask: Vec<bool>, spans: Vec<usize>) -> Self {
        Self::HereString {
            word: word.into(),
            mask,
            spans,
        }
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
