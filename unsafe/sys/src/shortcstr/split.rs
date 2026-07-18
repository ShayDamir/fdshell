use crate::shortcstr::ShortCStr;

pub struct Split {
    remaining: ShortCStr,
    sep: u8,
}

impl Split {
    pub(crate) fn new(remaining: &ShortCStr, sep: u8) -> Self {
        Self {
            remaining: remaining.clone(),
            sep,
        }
    }
}

impl Iterator for Split {
    type Item = ShortCStr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        match self.remaining.split_once_byte(self.sep) {
            Some((left, right)) => {
                self.remaining = right;
                Some(left)
            }
            None => Some(core::mem::take(&mut self.remaining)),
        }
    }
}
