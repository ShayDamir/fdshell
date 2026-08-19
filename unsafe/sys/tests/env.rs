#![allow(clippy::unwrap_used)]

use sys::env::getenv;

#[test]
fn getenv_returns_set_value() {
    // SAFETY: each nextest test runs in its own process, so mutating the
    // process environment here cannot race with other tests.
    unsafe { std::env::set_var("FDSHELL_TEST_VAR", "hello_env") };
    // A stub returning None (or an empty Some) would fail this exact-value check.
    let v = getenv(c"FDSHELL_TEST_VAR");
    assert_eq!(v.unwrap().as_bytes().unwrap(), b"hello_env");
    // SAFETY: same process-isolation reasoning as above.
    unsafe { std::env::remove_var("FDSHELL_TEST_VAR") };
}

#[test]
fn getenv_unset_returns_none() {
    // SAFETY: same process-isolation reasoning as above.
    unsafe { std::env::remove_var("FDSHELL_DEFINITELY_UNSET_VAR") };
    let v = getenv(c"FDSHELL_DEFINITELY_UNSET_VAR");
    assert!(v.is_none());
}
