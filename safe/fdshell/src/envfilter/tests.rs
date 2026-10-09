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
    assert!(glob_match(b"a*b*c", b"abxc"));
}

#[test]
fn glob_consecutive_stars() {
    assert!(glob_match(b"**", b""));
    assert!(glob_match(b"**", b"a"));
    assert!(glob_match(b"**", b"ab"));
    assert!(glob_match(b"***", b"anything"));
    assert!(glob_match(b"a**b", b"axxb"));
    assert!(glob_match(b"a**b", b"ab"));
    assert!(glob_match(b"**suffix", b"prefixsuffix"));
    assert!(glob_match(b"prefix**", b"prefixsuffix"));
}

#[test]
fn glob_empty_pattern() {
    assert!(glob_match(b"", b""));
    assert!(!glob_match(b"", b"x"));
}

#[test]
fn is_allowed_empty_filter() {
    let f = EnvFilter::new();
    assert!(f.is_allowed(&c"PATH".into()));
    assert!(f.is_allowed(&c"SECRET_KEY".into()));
}

#[test]
fn is_allowed_allowlist() {
    let mut f = EnvFilter::new();
    f.allow.push(c"P*".into());
    assert!(f.is_allowed(&c"PATH".into()));
    assert!(f.is_allowed(&c"PWD".into()));
    assert!(!f.is_allowed(&c"HOME".into()));
}

#[test]
fn is_allowed_denylist() {
    let mut f = EnvFilter::new();
    let star_key = c"*_KEY".into();
    f.deny.push(star_key);
    assert!(!f.is_allowed(&c"SECRET_KEY".into()));
    assert!(f.is_allowed(&c"PATH".into()));
}

#[test]
fn is_allowed_deny_wins_over_allow() {
    let mut f = EnvFilter::new();
    f.allow.push(c"PATH".into());
    f.deny.push(c"PATH".into());
    assert!(!f.is_allowed(&c"PATH".into()));
}

/// POSIX #4.1: the escape pair `\X` in a pattern is the literal `X` (the
/// backslash is dropped), so `a\*` matches the name `a*` and never wildcards.
#[test]
fn glob_escape_pair_is_the_literal_byte() {
    assert!(glob_match(b"a\\*", b"a*"));
    assert!(!glob_match(b"a\\*", b"ab"));
    assert!(!glob_match(b"a\\*", b"a"));
    assert!(glob_match(b"a\\*b", b"a*b"));
    assert!(!glob_match(b"a\\*b", b"aXXb"));
    // A star before the pair still wildcards.
    assert!(glob_match(b"*a\\*", b"xxa*"));
}

/// A trailing `\` in the pattern matches a literal backslash in the name.
#[test]
fn glob_trailing_backslash_is_literal() {
    assert!(glob_match(b"a\\", b"a\\"));
    assert!(!glob_match(b"a\\", b"a"));
    assert!(glob_match(b"\\\\", b"\\"));
}
