#![allow(clippy::unwrap_used, clippy::expect_used)]

use sys::access::{self, R_OK, W_OK};

/// A readable/writable path returns success from `access`.
#[test]
fn access_ok_on_readable_path() {
    access::access(c"/dev/null", R_OK).expect("R_OK on /dev/null must succeed");
    access::access(c"/dev/null", W_OK).expect("W_OK on /dev/null must succeed");
}

/// A missing path must fail; a blanket `Ok(())` mutation flips this to success.
#[test]
fn access_err_on_missing_path() {
    access::access(c"/nonexistent-fdshell-access-test", R_OK)
        .expect_err("access on a missing path must fail");
}
