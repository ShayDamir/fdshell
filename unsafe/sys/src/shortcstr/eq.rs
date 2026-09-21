use core::hash::{Hash, Hasher};

use crate::shortcstr::ShortCStr;

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

impl ShortCStr {
    pub fn eq_bytes(&self, other: &[u8]) -> bool {
        self.as_bytes().is_ok_and(|b| b == other)
    }

    pub fn starts_with(&self, prefix: &[u8]) -> bool {
        self.as_bytes().is_ok_and(|b| b.starts_with(prefix))
    }

    pub fn ends_with(&self, suffix: &[u8]) -> bool {
        self.as_bytes().is_ok_and(|b| b.ends_with(suffix))
    }

    pub fn contains(&self, byte: u8) -> bool {
        self.as_bytes().is_ok_and(|b| b.contains(&byte))
    }

    /// Index of the first occurrence of `byte`, or `None` if absent.
    pub fn find_byte(&self, byte: u8) -> Option<usize> {
        self.as_bytes().ok()?.iter().position(|&b| b == byte)
    }

    /// Index of the last occurrence of `byte`, or `None` if absent.
    pub fn rfind_byte(&self, byte: u8) -> Option<usize> {
        self.as_bytes().ok()?.iter().rposition(|&b| b == byte)
    }
}
