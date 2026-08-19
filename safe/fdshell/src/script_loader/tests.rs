#![allow(clippy::unwrap_used)]
use super::*;
use alloc::{format, vec};

#[test]
fn load_script_source_reads_file_and_sets_positional() {
    let path = std::env::temp_dir().join(format!("fdshell_script_test_{}.sh", std::process::id()));
    std::fs::write(&path, b"builtin echo hello\n").unwrap();
    let path_str = sys::ShortCStr::from_vec(path.to_str().unwrap().as_bytes().to_vec()).unwrap();
    let parsed = CliArgs {
        dirfd: None,
        script_fd: None,
        script_origin: None,
        positional: vec![sys::ImportedStr::new(
            path_str.clone(),
            sys::Trace::boundary(sys::Origin::CliArgument(1)),
        )],
    };
    let res = load_script_source(&parsed).unwrap();
    let (content, pos, origin) = res.unwrap();
    assert_eq!(content, b"builtin echo hello\n");
    assert_eq!(pos.len(), 1);
    assert_eq!(origin, sys::Origin::File(path_str));
    std::fs::remove_file(&path).unwrap();
}
