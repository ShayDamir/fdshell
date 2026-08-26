use sys::ShortCStr;

#[test]
fn parse_args_single_untagged_uses_own_name() {
    assert!(super::parse_args(&[ShortCStr::from(c"%rd")]).is_ok());
}

#[test]
fn parse_args_tagged_uses_explicit_tag() {
    assert!(super::parse_args(&[ShortCStr::from(c"out"), ShortCStr::from(c"%rd")]).is_ok());
}

#[test]
fn parse_args_requires_percent() {
    assert!(super::parse_args(&[ShortCStr::from(c"rd")]).is_err());
    assert!(super::parse_args(&[]).is_err());
}
