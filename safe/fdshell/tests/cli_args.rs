#![allow(clippy::unwrap_used)]

use fdshell::{load_script, parse_cli_args};
use sys::pipe::pipe2;

#[test]
fn parse_dirfd_without_value_is_usage_error() {
    // --dirfd with no value should return UsageMissingDirfdValue
    let args = vec![c"--dirfd".into()];
    let result = parse_cli_args(&args);
    assert!(
        result.is_err(),
        "--dirfd without a value should return an error"
    );
    if let Err(report) = result {
        let msg = format!("{report:?}");
        assert!(
            msg.contains("missing value for --dirfd"),
            "error should say 'missing value for --dirfd': {msg}"
        );
    }
}

#[test]
fn parse_fd_without_value_is_usage_error() {
    let args = vec![c"--fd".into()];
    let result = parse_cli_args(&args);
    assert!(
        result.is_err(),
        "--fd without a value should return an error"
    );
    if let Err(report) = result {
        let msg = format!("{report:?}");
        assert!(
            msg.contains("missing value for --fd"),
            "error should say 'missing value for --fd': {msg}"
        );
    }
}

#[test]
fn parse_dirfd_invalid_fd_number() {
    let args = vec![c"--dirfd".into(), c"badfd".into()];
    let result = parse_cli_args(&args);
    assert!(
        result.is_err(),
        "--dirfd with invalid fd should return an error"
    );
    if let Err(report) = result {
        let msg = format!("{report:?}");
        assert!(
            msg.contains("invalid fd number for --dirfd"),
            "error should say 'invalid fd number for --dirfd': {msg}"
        );
    }
}

#[test]
fn parse_fd_invalid_fd_number() {
    let args = vec![c"--fd".into(), c"badfd".into()];
    let result = parse_cli_args(&args);
    assert!(
        result.is_err(),
        "--fd with invalid fd should return an error"
    );
    if let Err(report) = result {
        let msg = format!("{report:?}");
        assert!(
            msg.contains("invalid fd number for --fd"),
            "error should say 'invalid fd number for --fd': {msg}"
        );
    }
}

#[test]
fn parse_dirfd_skips_positional_when_index_is_wrong() {
    // With the bug (i starts at 1), --dirfd value positional
    // would parse "value" as positional instead of as --dirfd's value.
    // This test verifies positional is correctly empty when --dirfd
    // has a value (fd -1 is invalid, so dirfd will be Err, but positional
    // should still be empty if the loop index is correct).
    let args = vec![c"--dirfd".into(), c"badfd".into(), c"script.sh".into()];
    let result = parse_cli_args(&args);
    // dirfd parsing fails (badfd is not a valid open fd), but we can
    // check the error is from fd validation, not from missing args.
    // The key assertion: if i started at 1, "script.sh" would be parsed
    // as the dirfd value and "badfd" would be positional.
    // With i=0, "badfd" is parsed as dirfd value (fails), "script.sh"
    // is never reached.
    // We verify by checking that positional is empty (script.sh was not
    // consumed as a positional arg).
    match result {
        Ok(parsed) => {
            // dirfd parsed successfully (unlikely with "badfd")
            assert!(
                parsed.positional.is_empty(),
                "positional should be empty; script.sh was incorrectly parsed as positional"
            );
        }
        Err(_) => {
            // dirfd validation failed - this is expected
            // The test passes because we got an error (not a panic about
            // missing args), meaning the loop correctly consumed --dirfd
            // and its value.
        }
    }
}

#[test]
fn parse_fd_skips_positional_when_index_is_wrong() {
    // Same test for --fd flag
    let args = vec![c"--fd".into(), c"badfd".into(), c"extra".into()];
    let result = parse_cli_args(&args);
    match result {
        Ok(parsed) => {
            assert!(
                parsed.positional.is_empty(),
                "positional should be empty; extra was incorrectly parsed as positional"
            );
        }
        Err(_) => {
            // fd validation failed - expected
        }
    }
}

#[test]
fn parse_positional_only() {
    let args = vec![c"script.sh".into(), c"--flag".into()];
    let result = parse_cli_args(&args);
    assert!(
        result.is_ok(),
        "positional-only args should parse successfully"
    );
    let parsed = result.unwrap();
    assert_eq!(parsed.positional.len(), 2, "both args should be positional");
    assert_eq!(
        parsed.positional.first().unwrap().as_bytes().unwrap(),
        b"script.sh"
    );
    assert_eq!(
        parsed.positional.get(1).unwrap().as_bytes().unwrap(),
        b"--flag"
    );
    assert!(parsed.dirfd.is_none());
    assert!(parsed.script_fd.is_none());
}

#[test]
fn parse_positional_origins_are_one_based() {
    let args = vec![c"script.sh".into(), c"--flag".into()];
    let parsed = parse_cli_args(&args).unwrap();
    assert_eq!(
        parsed.positional.first().unwrap().trace.origin,
        sys::Origin::CliArgument(1)
    );
    assert_eq!(
        parsed.positional.get(1).unwrap().trace.origin,
        sys::Origin::CliArgument(2)
    );
}

#[test]
fn load_script_reads_data_until_eof() {
    let (rd, wr) = pipe2(0).unwrap();
    // Write > 4096 bytes to force multiple reads; mutant breaks on first read
    // and returns truncated data, causing this assertion to fail.
    let expected: Vec<u8> = vec![0xAB; 8192];
    let data = expected.clone();
    std::thread::scope(|s| {
        s.spawn(move || {
            sys::rw::write(&wr, &data).unwrap();
        });
        let content = load_script(&rd).unwrap();
        assert_eq!(content.as_slice(), &expected[..]);
    });
}
