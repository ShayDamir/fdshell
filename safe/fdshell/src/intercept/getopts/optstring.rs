// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! The optstring parser and its alphabet rule (task #590, decision pinned in the
//! plan §4.1/§4.2): a **leading** `:` selects silent mode and is not an option
//! character; a `:` directly after an option character is that character's
//! takes-an-argument marker; the option-character alphabet is `[A-Za-z0-9]` —
//! POSIX allows only letters and says nothing for the rest, and bash accepts
//! digits (measured), so stray bytes (a `#`, a `?`, a non-ASCII byte, a `:` that
//! is no one's marker) are ignored inside the optstring, and an option word that
//! reaches one takes the presentation-error path.

use alloc::vec::Vec;

use sys::ShortCStr;

use super::byte_at;

/// A parsed optstring: the silent flag and the option-character table. The flag
/// has no consumer before task #591 (naming and diagnostics land there); it is
/// pinned at the unit level in this task.
#[derive(Debug, PartialEq)]
pub(crate) struct Optstring {
    silent: bool,
    options: Vec<(u8, bool)>,
}

impl Optstring {
    pub(crate) fn parse(optstring: &ShortCStr) -> Self {
        let (rest, silent) = match optstring.strip_prefix(b":") {
            Some(rest) => (rest, true),
            None => (optstring.clone(), false),
        };
        let mut options = Vec::new();
        let mut i = 0;
        while let Some(ch) = byte_at(&rest, i) {
            if is_option_char(ch) {
                options.push((ch, byte_at(&rest, i + 1) == Some(b':')));
            }
            i += 1;
        }
        Self { silent, options }
    }

    /// The alphabet rule (§4.1) applied: `None` for every byte that is not an
    /// option character, `Some(takes an argument)` for every one that is.
    pub(crate) fn lookup(&self, ch: u8) -> Option<bool> {
        self.options
            .iter()
            .find(|(c, _)| *c == ch)
            .map(|(_, takes)| *takes)
    }

    pub(crate) fn is_silent(&self) -> bool {
        self.silent
    }
}

/// Option characters are letters and digits, and nothing else (task #590 §4.1);
/// every other byte is ignored inside the optstring.
fn is_option_char(ch: u8) -> bool {
    matches!(ch, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
}

#[cfg(test)]
mod tests;
