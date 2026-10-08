use alloc::vec::Vec;

/// How the line ended.
#[cfg_attr(test, derive(Debug, Clone, Copy, PartialEq, Eq))]
pub(crate) enum LineEnd {
    /// The delimiter or the `-n` cap was reached.
    Delim,
    /// End of input before the delimiter.
    Eof,
    /// The `-t` timeout expired with no input (produced by the caller).
    Timeout,
}

/// Shared line accumulator driving both read paths (stdin and fd).
///
/// Default backslash mode: `\` + newline is a continuation (both dropped),
/// `\` + any other byte drops the backslash, a trailing `\` at EOF is
/// dropped. With `raw` (`-r`) every backslash is literal.
pub(crate) struct Line<'a> {
    pub buf: Vec<u8>,
    done: bool,
    delim: &'a [u8],
    raw: bool,
    max: Option<usize>,
    esc: bool,
}

impl<'a> Line<'a> {
    /// New accumulator; an empty `delim` set means "read until EOF", and
    /// `max == Some(0)` finishes the line before any byte is read.
    pub fn new(delim: Option<&'a [u8]>, raw: bool, max: Option<usize>) -> Self {
        Self {
            buf: Vec::new(),
            done: max == Some(0),
            delim: delim.unwrap_or(b"\n"),
            raw,
            max,
            esc: false,
        }
    }

    /// Feed one input byte; a no-op once the line is finished.
    pub fn feed(&mut self, b: u8) {
        if self.done {
            return;
        }
        if self.esc {
            self.esc = false;
            if b != b'\n' {
                self.store(b);
            }
            return;
        }
        if !self.raw && b == b'\\' {
            self.esc = true;
            return;
        }
        if self.delim.contains(&b) {
            self.done = true;
            return;
        }
        self.store(b);
    }

    /// True once the delimiter, the `-n` cap, or `-n 0` finished the line.
    pub fn finished(&self) -> bool {
        self.done
    }

    /// The end reason once finished: `Delim` on a finished line, else `Eof`.
    pub fn end(&self) -> LineEnd {
        if self.done {
            LineEnd::Delim
        } else {
            LineEnd::Eof
        }
    }

    /// Store a byte, finishing the line when the `-n` cap is reached.
    fn store(&mut self, b: u8) {
        self.buf.push(b);
        if let Some(max) = self.max
            && self.buf.len() >= max
        {
            self.done = true;
        }
    }
}
