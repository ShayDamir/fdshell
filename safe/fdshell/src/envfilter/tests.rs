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
    f.allow.push(c"P*".into());
    assert!(f.is_allowed(b"PATH"));
    assert!(f.is_allowed(b"PWD"));
    assert!(!f.is_allowed(b"HOME"));
}

#[test]
fn is_allowed_denylist() {
    let mut f = EnvFilter::new();
    let star_key = c"*_KEY".into();
    f.deny.push(star_key);
    assert!(!f.is_allowed(b"SECRET_KEY"));
    assert!(f.is_allowed(b"PATH"));
}

#[test]
fn is_allowed_deny_wins_over_allow() {
    let mut f = EnvFilter::new();
    f.allow.push(c"PATH".into());
    f.deny.push(c"PATH".into());
    assert!(!f.is_allowed(b"PATH"));
}
