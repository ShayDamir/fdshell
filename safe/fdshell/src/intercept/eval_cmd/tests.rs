use super::join_space;

#[test]
fn join_space_separates_args_with_single_spaces() {
    let args = [c"a".into(), c"b".into(), c"c".into()];
    assert!(join_space(&args).eq_bytes(b"a b c"));
}

#[test]
fn join_space_single_arg_has_no_separator() {
    let args = [c"solo".into()];
    assert!(join_space(&args).eq_bytes(b"solo"));
}
