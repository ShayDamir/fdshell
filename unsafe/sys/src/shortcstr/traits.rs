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

impl fmt::Debug for ShortCStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.as_bytes().unwrap_or(b"<?>");
        match self {
            ShortCStr::Inline { len, .. } => f
                .debug_struct("Inline")
                .field("len", &len.as_u8())
                .field("buf", &bytes)
                .finish(),
            ShortCStr::Static(..) => f.debug_tuple("Static").field(&bytes).finish(),
            ShortCStr::Arc { .. } => f.debug_tuple("Arc").field(&bytes).finish(),
        }
    }
}

impl From<&'static CStr> for ShortCStr {
    fn from(s: &'static CStr) -> Self {
        ShortCStr::Static(s, 0, s.count_bytes())
    }
}
