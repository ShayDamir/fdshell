//! Environment variable filtering.
//!
//! Allows/ denies variables by glob pattern (pure `*` wildcard) before
//! they are inherited by child processes.

mod glob;

use alloc::vec::Vec;
pub(crate) use glob::glob_match;
use sys::ShortCStr;

/// Filter state: allowlist patterns, denylist patterns.
#[derive(Clone, Default)]
pub(crate) struct EnvFilter {
    pub allow: Vec<ShortCStr>,
    pub deny: Vec<ShortCStr>,
}

impl EnvFilter {
    pub fn new() -> Self {
        Self {
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }

    pub fn is_allowed(&self, name: &ShortCStr) -> bool {
        let name_bytes = match name.as_bytes() {
            Ok(b) => b,
            Err(_) => return false,
        };
        if !self.allow.is_empty() {
            let allowed = self
                .allow
                .iter()
                .any(|p| glob_match(p.as_bytes().unwrap_or(&[]), name_bytes));
            if !allowed {
                return false;
            }
        }
        !self
            .deny
            .iter()
            .any(|p| glob_match(p.as_bytes().unwrap_or(&[]), name_bytes))
    }

    pub fn clear(&mut self) {
        self.allow.clear();
        self.deny.clear();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
