use crate::shortcstr::ShortCStr;

pub struct Split {
    remaining: ShortCStr,
    sep: u8,
    pending_trailing_empty: bool,
}

impl Split {
    pub(crate) fn new(remaining: &ShortCStr, sep: u8) -> Self {
        Self {
            remaining: remaining.clone(),
            sep,
            pending_trailing_empty: false,
        }
    }
}

impl Iterator for Split {
    type Item = ShortCStr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pending_trailing_empty {
            self.pending_trailing_empty = false;
            return Some(core::mem::take(&mut self.remaining));
        }

        if self.remaining.is_empty() {
            return None;
        }

        match self.remaining.split_once_byte(self.sep) {
            Some((left, right)) => {
                if right.is_empty() {
                    self.pending_trailing_empty = true;
                }
                self.remaining = right;
                Some(left)
            }
            None => Some(core::mem::take(&mut self.remaining)),
        }
    }
}
