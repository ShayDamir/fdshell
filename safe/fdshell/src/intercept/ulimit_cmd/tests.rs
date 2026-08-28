use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

use super::parse::{Parsed, Value, parse};
use super::set::scale;
use crate::error::cmd::CmdError;
use crate::intercept::ulimit_cmd::resources;

fn run(words: &[&str]) -> Result<Parsed, Report<CmdError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    parse(&args)
}

#[test]
fn no_args_is_empty() {
    let p = run(&[]).unwrap();
    assert!(!p.list);
    assert!(!p.hard);
    assert!(!p.soft);
    assert!(p.resource.is_none());
    assert!(p.value.is_none());
}

#[test]
fn list_flag() {
    let p = run(&["-a"]).unwrap();
    assert!(p.list);
    assert!(!p.hard);
    assert!(p.resource.is_none());
}

#[test]
fn scope_flags() {
    let p = run(&["-Ha"]).unwrap();
    assert!(p.list);
    assert!(p.hard);
    assert!(!p.soft);
}

#[test]
fn combined_scope_and_resource_flags() {
    let p = run(&["-HSn", "100"]).unwrap();
    assert!(p.hard);
    assert!(p.soft);
    assert_eq!(p.resource.unwrap().flag, b'n');
    assert_eq!(p.value.unwrap().amount, 100);
}

#[test]
fn resource_flag_without_value() {
    let p = run(&["-Hn"]).unwrap();
    assert!(p.hard);
    assert!(!p.soft);
    assert_eq!(p.resource.unwrap().flag, b'n');
    assert!(p.value.is_none());
}

#[test]
fn unlimited_value() {
    let p = run(&["-n", "unlimited"]).unwrap();
    assert_eq!(p.resource.unwrap().flag, b'n');
    assert_eq!(p.value.unwrap().amount, sys::rlimit::UNLIMITED);
}

#[test]
fn value_before_flags() {
    let p = run(&["100", "-n"]).unwrap();
    assert_eq!(p.resource.unwrap().flag, b'n');
    assert_eq!(p.value.unwrap().amount, 100);
}

#[test]
fn dashdash_is_skipped() {
    let p = run(&["-n", "--", "5"]).unwrap();
    assert_eq!(p.value.unwrap().amount, 5);
}

#[test]
fn extra_value_words_are_ignored() {
    let p = run(&["-n", "1", "2"]).unwrap();
    assert_eq!(p.value.unwrap().amount, 1);
}

#[test]
fn bad_values_are_rejected() {
    for word in ["abc", "100K", "99999999999999999999"] {
        let r = run(&["-n", word]);
        assert!(
            matches!(
                r.unwrap_err().current_context(),
                CmdError::UlimitBadValue { .. }
            ),
            "{word:?} must be rejected"
        );
    }
}

#[test]
fn lone_dash_is_a_bad_value() {
    let r = run(&["-"]);
    assert!(
        matches!(
            r.unwrap_err().current_context(),
            CmdError::UlimitBadValue { .. }
        ),
        "a lone dash is a value word, not a flag"
    );
}

#[test]
fn invalid_options_are_rejected() {
    for word in ["-z", "-zz", "-N"] {
        let flag = word.chars().nth(1).unwrap();
        let r = run(&[word]);
        assert!(
            matches!(
                r.unwrap_err().current_context(),
                CmdError::UlimitInvalidOption { flag: f } if f == &flag
            ),
            "{word:?} must be rejected"
        );
    }
}

fn val(amount: u64) -> Value {
    Value {
        amount,
        text: ShortCStr::from_vec(b"v".to_vec()).unwrap(),
    }
}

#[test]
fn scale_passes_unlimited_through() {
    let fsize = resources::by_flag(b'f').unwrap();
    assert_eq!(
        scale(fsize, val(sys::rlimit::UNLIMITED)).unwrap(),
        sys::rlimit::UNLIMITED
    );
}

#[test]
fn scale_uses_the_resource_unit() {
    let fsize = resources::by_flag(b'f').unwrap();
    assert_eq!(scale(fsize, val(10)).unwrap(), 10240);
    let cpu = resources::by_flag(b't').unwrap();
    assert_eq!(scale(cpu, val(10)).unwrap(), 10);
    let nofile = resources::by_flag(b'n').unwrap();
    assert_eq!(scale(nofile, val(10)).unwrap(), 10);
}

#[test]
fn scale_overflow_is_a_bad_value() {
    let fsize = resources::by_flag(b'f').unwrap();
    let r = scale(fsize, val(u64::MAX / 2));
    assert!(
        matches!(
            r.unwrap_err().current_context(),
            CmdError::UlimitBadValue { .. }
        ),
        "scaling past u64::MAX must be a bad value"
    );
}

#[test]
fn two_resource_flags_are_a_usage_error() {
    for words in [&["-n", "-c", "5"] as &[&str], &["-nc"] as &[&str]] {
        let r = run(words);
        assert!(
            matches!(r.unwrap_err().current_context(), CmdError::UlimitUsage),
            "{words:?} must be a usage error"
        );
    }
}
