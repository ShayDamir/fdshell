#![allow(clippy::unwrap_used)]

use sys::cmdline::read_cmdline;

#[test]
fn read_cmdline_matches_args() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let argv = read_cmdline().unwrap();
    // A mutation that splits on the wrong byte (e.g. non-NUL) would shatter the
    // argument count; comparing against the real argv catches it.
    assert_eq!(argv.len(), args.len());
    for (got, want) in argv.iter().zip(&args) {
        assert_eq!(got.as_bytes().unwrap(), want.as_encoded_bytes());
    }
}
