use crate::importedstr::{Origin, Position};
use crate::shortcstr::{ShortCStr, ShortCStrError};

/// Advance `start` (the position of byte 0) by `off` bytes of `bytes`.
pub(crate) fn position_at(bytes: &[u8], start: Position, off: usize) -> Position {
    let mut pos = start;
    for &b in bytes.get(..off.min(bytes.len())).unwrap_or(&[]) {
        if b == b'\n' {
            pos.line = pos.line.saturating_add(1);
            pos.column = 1;
        } else {
            pos.column = pos.column.saturating_add(1);
        }
    }
    pos
}

/// A chunk of script text together with its provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptText {
    /// The text itself.
    pub data: ShortCStr,
    /// Position of the first byte of `data`.
    pub start: Position,
    /// The source boundary this text originated from.
    pub origin: Origin,
}

impl ScriptText {
    pub fn new(data: ShortCStr, start: Position, origin: Origin) -> Self {
        Self {
            data,
            start,
            origin,
        }
    }

    /// A subview of this text starting at byte `off` with length `len`.
    ///
    /// `start` is advanced past `off` bytes; `origin` is inherited.
    pub fn subslice(&self, off: usize, len: usize) -> Option<Self> {
        let bytes = self.data.as_bytes().ok()?;
        let end = off.checked_add(len)?;
        if end > bytes.len() {
            return None;
        }
        let data = self.data.get(off..end)?;
        Some(Self {
            data,
            start: position_at(bytes, self.start, off),
            origin: self.origin.clone(),
        })
    }

    pub fn as_bytes(&self) -> Result<&[u8], ShortCStrError> {
        self.data.as_bytes()
    }
}

#[cfg(test)]
mod tests;
