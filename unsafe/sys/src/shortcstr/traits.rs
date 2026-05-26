use alloc::rc::Rc;
use core::borrow::Borrow;
use core::fmt;
use core::hash::{Hash, Hasher};

use crate::shortcstr::ShortCStr;

impl Borrow<[u8]> for ShortCStr {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Clone for ShortCStr {
    fn clone(&self) -> Self {
        match self {
            ShortCStr::Inline { len, buf } => ShortCStr::Inline {
                len: *len,
                buf: *buf,
            },
            ShortCStr::Static(s, offset, length) => ShortCStr::Static(s, *offset, *length),
            ShortCStr::Rc { rc, offset, length } => ShortCStr::Rc {
                rc: Rc::clone(rc),
                offset: *offset,
                length: *length,
            },
        }
    }
}

impl PartialEq for ShortCStr {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for ShortCStr {}

impl Hash for ShortCStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

impl fmt::Debug for ShortCStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShortCStr::Inline { len, .. } => f
                .debug_struct("Inline")
                .field("len", &len.as_u8())
                .field("buf", &self.as_bytes())
                .finish(),
            ShortCStr::Static(s, offset, length) => f
                .debug_struct("Static")
                .field("s", s)
                .field("offset", offset)
                .field("length", length)
                .finish(),
            ShortCStr::Rc { rc, offset, length } => f
                .debug_struct("Rc")
                .field("rc", rc)
                .field("offset", offset)
                .field("length", length)
                .finish(),
        }
    }
}
