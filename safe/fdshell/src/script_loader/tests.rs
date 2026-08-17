#![allow(clippy::unwrap_used)]
use super::*;
use alloc::{format, vec};

#[test]
fn load_script_source_reads_file_and_sets_positional() {
    let path = std::env::temp_dir().join(format!("fdshell_script_test_{}.sh", std::process::id()));
    std::fs::write(&path, b"builtin echo hello\n").unwrap();
    let parsed = CliArgs {
        dirfd: None,
        script_fd: None,
        positional: vec![ShortCStr::from_vec(path.to_str().unwrap().as_bytes().to_vec()).unwrap()],
    };
    let res = load_script_source(&parsed).unwrap();
    let (content, pos) = res.unwrap();
    assert_eq!(content, b"builtin echo hello\n");
    assert_eq!(pos.len(), 1);
    std::fs::remove_file(&path).unwrap();
}
