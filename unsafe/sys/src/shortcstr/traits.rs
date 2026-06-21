use alloc::sync::Arc;
use core::ffi::CStr;
use core::fmt;
use core::hash::{Hash, Hasher};

use crate::shortcstr::ShortCStr;

impl Clone for ShortCStr {
    fn clone(&self) -> Self {
        match self {
            ShortCStr::Inline { len, buf } => ShortCStr::Inline {
                len: *len,
                buf: *buf,
            },
            ShortCStr::Static(s, offset, length) => ShortCStr::Static(s, *offset, *length),
            ShortCStr::Arc {
                arc,
                offset,
                length,
            } => ShortCStr::Arc {
                arc: Arc::clone(arc),
                offset: *offset,
                length: *length,
            },
        }
    }
}

impl PartialEq for ShortCStr {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_bytes(), other.as_bytes()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ShortCStr {}

impl Hash for ShortCStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Ok(b) = self.as_bytes() {
            b.hash(state);
        }
    }
}

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

impl From<&'static CStr> for ShortCStr {
    fn from(s: &'static CStr) -> Self {
        ShortCStr::Static(s, 0, s.count_bytes())
    }
}
