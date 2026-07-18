use alloc::sync::Arc;
use core::ffi::CStr;
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

impl From<&'static CStr> for ShortCStr {
    fn from(s: &'static CStr) -> Self {
        ShortCStr::Static(s, 0, s.count_bytes())
    }
}
