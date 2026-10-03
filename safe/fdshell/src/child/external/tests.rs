#![allow(clippy::unwrap_used)]

use sys::ShortCStr;

#[test]
fn split_command_empty_expansion_falls_back_to_literal() {
    // A nullglob command name expands to nothing: the literal word is the
    // command, with no leading args (README: bash drops the command instead).
    let literal = ShortCStr::from_vec(b"zzz*".to_vec()).unwrap();
    let (name, extra) = super::split_command(&[], &literal);
    assert_eq!(name, literal);
    assert!(extra.is_empty());
}

#[test]
fn split_command_first_field_is_command_rest_are_args() {
    let first = ShortCStr::from_vec(b"./c1.sh".to_vec()).unwrap();
    let second = ShortCStr::from_vec(b"./c2.sh".to_vec()).unwrap();
    let (name, extra) = super::split_command(&[first.clone(), second.clone()], &first);
    assert_eq!(name, first);
    let Some(extra0) = extra.first() else {
        panic!("expected one leading arg");
    };
    assert_eq!(extra0.as_ref().to_bytes(), b"./c2.sh");
}
