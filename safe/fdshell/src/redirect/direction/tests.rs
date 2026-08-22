use super::*;

#[test]
fn read_uses_rdonly() {
    assert_eq!(RedirectDirection::Read.open_flags(), O_RDONLY);
}

#[test]
fn write_uses_wronly_creat_trunc() {
    assert_eq!(
        RedirectDirection::Write.open_flags(),
        O_WRONLY + O_CREAT + O_TRUNC
    );
}

#[test]
fn append_uses_wronly_creat_append() {
    assert_eq!(
        RedirectDirection::Append.open_flags(),
        O_WRONLY + O_CREAT + O_APPEND
    );
}

#[test]
fn rw_uses_rdwr_creat_without_trunc() {
    assert_eq!(RedirectDirection::Rw.open_flags(), O_RDWR + O_CREAT);
}
