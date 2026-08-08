use crate::envfilter::EnvFilter;
use crate::intercept::envfilter_display::{help_text, rules_text};
use alloc::vec::Vec;
use sys::ShortCStr;

fn make_filter(allow: &[&str], deny: &[&str]) -> EnvFilter {
    let allow_vec: Vec<ShortCStr> = allow
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    let deny_vec: Vec<ShortCStr> = deny
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    EnvFilter {
        allow: allow_vec,
        deny: deny_vec,
    }
}

#[test]
fn help_text_is_nonempty() {
    let text = help_text();
    assert!(!text.is_empty());
    assert!(text.starts_with(b"Usage: envfilter"));
}

#[test]
fn rules_text_empty_filter() {
    let filter = make_filter(&[], &[]);
    let text = rules_text(&filter).unwrap();
    assert_eq!(text.as_bytes().unwrap(), b"");
}

#[test]
fn rules_text_single_allow() {
    let filter = make_filter(&["PATH"], &[]);
    let text = rules_text(&filter).unwrap();
    assert_eq!(text.as_bytes().unwrap(), b"allow: PATH\n");
}

#[test]
fn rules_text_single_deny() {
    let filter = make_filter(&[], &["*_KEY"]);
    let text = rules_text(&filter).unwrap();
    assert_eq!(text.as_bytes().unwrap(), b"deny: *_KEY\n");
}

#[test]
fn rules_text_multiple_allow() {
    let filter = make_filter(&["PATH", "HOME"], &[]);
    let text = rules_text(&filter).unwrap();
    assert_eq!(text.as_bytes().unwrap(), b"allow: PATH HOME\n");
}

#[test]
fn rules_text_multiple_deny() {
    let filter = make_filter(&[], &["*_KEY", "*_TOKEN"]);
    let text = rules_text(&filter).unwrap();
    assert_eq!(text.as_bytes().unwrap(), b"deny: *_KEY *_TOKEN\n");
}

#[test]
fn rules_text_allow_and_deny() {
    let filter = make_filter(&["PATH"], &["*_KEY"]);
    let text = rules_text(&filter).unwrap();
    assert_eq!(text.as_bytes().unwrap(), b"allow: PATH\ndeny: *_KEY\n");
}

#[test]
fn rules_text_multiple_mixed() {
    let filter = make_filter(&["PATH", "HOME", "USER"], &["*_KEY", "*_TOKEN"]);
    let text = rules_text(&filter).unwrap();
    assert_eq!(
        text.as_bytes().unwrap(),
        b"allow: PATH HOME USER\ndeny: *_KEY *_TOKEN\n"
    );
}
