use core::fmt;

use crate::shortcstr::ShortCStr;

impl fmt::Display for ShortCStr {
    /// Formats for user-facing output. Uses lossy UTF-8 conversion for
    /// non-UTF-8 bytes — this is the standard Unix convention for error
    /// messages (bash, ls, etc. all show `?` for invalid bytes).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_bytes() {
            Ok(bytes) => {
                if let Ok(s) = core::str::from_utf8(bytes) {
                    write!(f, "{s}")
                } else {
                    let s = alloc::string::String::from_utf8_lossy(bytes);
                    write!(f, "{s}")
                }
            }
            Err(_) => write!(f, "<BadState>"),
        }
    }
}

impl fmt::Debug for ShortCStr {
    /// Formats as a quoted string when the bytes are valid UTF-8, falling back
    /// to a faithful byte-array representation for non-UTF-8 content. Never
    /// silently discards or replaces bytes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_bytes() {
            Ok(bytes) => {
                if let Ok(s) = core::str::from_utf8(bytes) {
                    write!(f, "{s:?}")
                } else {
                    write!(f, "{:?}", bytes)
                }
            }
            Err(_) => f.write_str("\"<BadState>\""),
        }
    }
}

impl fmt::Write for ShortCStr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.extend_from_slice(s.as_bytes()).map_err(|_| fmt::Error)
    }
}
