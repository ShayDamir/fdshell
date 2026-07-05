//! Environment variable filtering.
//!
//! Allows/ denies variables by glob pattern (pure `*` wildcard) before
//! they are inherited by child processes.

use sys::ShortCStr;

/// Filter state: allowlist patterns, denylist patterns.
#[derive(Clone)]
pub(crate) struct EnvFilter {
    pub allow: Vec<ShortCStr>,
    pub deny: Vec<ShortCStr>,
}

impl EnvFilter {
    pub fn new() -> Self {
        EnvFilter {
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }

    /// Decide whether a variable name should pass through.
    ///
    /// 1. If `allow` is non-empty and name matches no allow pattern → false.
    /// 2. If name matches any deny pattern → false.
    /// 3. Otherwise → true.
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

impl Default for EnvFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple `*` wildcard glob match.
///
/// Supports only `*` (match any sequence). No `?`, `[...]`, or escapes.
pub(crate) fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    match_glob(pattern, 0, name, 0)
}

fn match_glob(pat: &[u8], pi: usize, str: &[u8], si: usize) -> bool {
    let plen = pat.len();
    let slen = str.len();

    // Both consumed → match
    if pi == plen && si == slen {
        return true;
    }

    // Pattern has `*` ahead
    if pat.get(pi) == Some(&b'*') {
        // Skip consecutive stars
        let mut npi = pi + 1;
        while pat.get(npi) == Some(&b'*') {
            npi += 1;
        }
        // If star is last in pattern, it matches everything remaining
        if npi == plen {
            return true;
        }
        // Try matching the rest of the pattern at each position
        for i in si..=slen {
            if match_glob(pat, npi, str, i) {
                return true;
            }
        }
        return false;
    }

    // Literal character match (None == None unreachable — caught by early return above)
    if pat.get(pi) == str.get(si) {
        return match_glob(pat, pi + 1, str, si + 1);
    }

    // Mismatch or one side exhausted
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn glob_exact_match() {
        assert!(glob_match(b"PATH", b"PATH"));
        assert!(!glob_match(b"PATH", b"PATHNAME"));
        assert!(!glob_match(b"PATH", b"APATH"));
    }

    #[test]
    fn glob_star_matches_all() {
        assert!(glob_match(b"*", b""));
        assert!(glob_match(b"*", b"anything"));
    }

    #[test]
    fn glob_suffix_star() {
        assert!(glob_match(b"*_KEY", b"FOO_KEY"));
        assert!(!glob_match(b"*_KEY", b"FOO"));
        // *_KEY does NOT match KEY (star matches empty, _KEY ≠ KEY)
        assert!(!glob_match(b"*_KEY", b"KEY"));
    }

    #[test]
    fn glob_prefix_star() {
        assert!(glob_match(b"PATH*", b"PATH"));
        assert!(glob_match(b"PATH*", b"PATHNAME"));
        assert!(!glob_match(b"PATH*", b"APATH"));
    }

    #[test]
    fn glob_contains() {
        assert!(glob_match(b"*MID*", b"FOOMIDBAR"));
        assert!(glob_match(b"*MID*", b"MID"));
        assert!(!glob_match(b"*MID*", b"FOOBAR"));
    }

    #[test]
    fn glob_multiple_stars() {
        assert!(glob_match(b"a*b*c", b"axbyc"));
        assert!(glob_match(b"a*b*c", b"abc"));
        // a*b*c matches abxc: a, *(empty), b, *(x), c
        assert!(glob_match(b"a*b*c", b"abxc"));
    }

    #[test]
    fn glob_empty_pattern() {
        assert!(glob_match(b"", b""));
        assert!(!glob_match(b"", b"x"));
    }

    #[test]
    fn is_allowed_empty_filter() {
        let f = EnvFilter::new();
        assert!(f.is_allowed(b"PATH"));
        assert!(f.is_allowed(b"SECRET_KEY"));
    }

    #[test]
    fn is_allowed_allowlist() {
        let mut f = EnvFilter::new();
        f.allow.push(ShortCStr::from_vec(b"P*".to_vec()).unwrap());
        assert!(f.is_allowed(b"PATH"));
        assert!(f.is_allowed(b"PWD"));
        assert!(!f.is_allowed(b"HOME"));
    }

    #[test]
    fn is_allowed_denylist() {
        let mut f = EnvFilter::new();
        // Build "*_KEY" as ShortCStr
        let star_key = ShortCStr::from_vec(b"*_KEY".to_vec()).unwrap();
        f.deny.push(star_key);
        assert!(!f.is_allowed(b"SECRET_KEY"));
        assert!(f.is_allowed(b"PATH"));
    }

    #[test]
    fn is_allowed_deny_wins_over_allow() {
        let mut f = EnvFilter::new();
        f.allow.push(ShortCStr::from_vec(b"PATH".to_vec()).unwrap());
        f.deny.push(ShortCStr::from_vec(b"PATH".to_vec()).unwrap());
        assert!(!f.is_allowed(b"PATH"));
    }
}
