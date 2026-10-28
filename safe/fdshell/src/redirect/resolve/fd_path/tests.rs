use super::{PREFIXES, magic};
use core::ffi::CStr;
use sys::ShortCStr;

/// The fd number a magic-path word names, for the tables below.
fn number(word: &'static CStr) -> ShortCStr {
    ShortCStr::from(word)
}

/// The recognized form is exact, and the two prefixes are the two Linux
/// installs. The rejected rows are plain paths to the kernel, and bash 5.3.9
/// fails each of them on the open (rc 1, `No such file or directory`):
/// `/dev/fd/01` is not fd 1 (the entry's name is `1`), `/dev/fd/1x` and
/// `/dev/fd/+1` carry junk, `/dev/fd/` is the directory, and a value outside the
/// `i32` fd-number range is no fd at all. Integration twin:
/// `tests/redirect.rs::dev_fd_target_is_canonical_digits_only`.
#[test]
fn dev_fd_word_is_recognized_only_in_the_exact_form() {
    let accepted: &[(&CStr, &str, i32)] = &[
        (c"/dev/fd/0", "/dev/fd/", 0),
        (c"/dev/fd/1", "/dev/fd/", 1),
        (c"/dev/fd/12", "/dev/fd/", 12),
        (c"/proc/self/fd/0", "/proc/self/fd/", 0),
        (c"/proc/self/fd/7", "/proc/self/fd/", 7),
    ];
    for (text, prefix, n) in accepted {
        assert_eq!(
            magic(&number(text)),
            Some((*prefix, *n)),
            "{:?} is a magic path",
            text.to_str()
        );
    }
    let rejected: &[&CStr] = &[
        c"",
        c"dev/fd/1",
        c"/dev/fd",
        c"/dev/fd/",
        c"/dev/fd/01",
        c"/dev/fd/00",
        c"/dev/fd/1x",
        c"/dev/fd/x1",
        c"/dev/fd/+1",
        c"/dev/fd/-1",
        c"/dev/fd/ 1",
        c"/dev/fd/1 ",
        c"/dev/fd/99999999999",
        c"/tmp/dev/fd/1",
        c"/proc/self/fd",
        c"/proc/self/fd/",
        c"/proc/self/fd/01",
        c"/proc/self/fd/1x",
        c"/proc/thread-self/fd/1",
        c"/dev/fd/1/2",
    ];
    for text in rejected {
        assert_eq!(
            magic(&number(text)),
            None,
            "{:?} is an ordinary path",
            text.to_str()
        );
    }
    // The accepted rows cover every prefix in the table, so dropping a prefix
    // from the list is a test failure and not a silent narrowing.
    for prefix in PREFIXES {
        assert!(
            accepted.iter().any(|(_, p, _)| p == prefix),
            "the accepted rows must cover the {prefix} prefix"
        );
    }
}
