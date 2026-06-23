use crate::shortcstr::ShortCStr;

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
}
