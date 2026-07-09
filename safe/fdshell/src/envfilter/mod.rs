//! Environment variable filtering.
//!
//! Allows/ denies variables by glob pattern (pure `*` wildcard) before
//! they are inherited by child processes.

mod glob;

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

    pub fn is_allowed(&self, name: &[u8]) -> bool {
        if !self.allow.is_empty() {
            let allowed = self
                .allow
                .iter()
                .any(|p| p.as_bytes().is_ok_and(|b| glob_match(b, name)));
            if !allowed {
                return false;
            }
        }
        !self
            .deny
            .iter()
            .any(|p| p.as_bytes().is_ok_and(|b| glob_match(b, name)))
    }

    pub fn clear(&mut self) {
        self.allow.clear();
        self.deny.clear();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
