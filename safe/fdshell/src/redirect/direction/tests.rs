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

// POSIX #2.2 `>|` opens exactly like `>`: the noclobber bypass is the open.rs
// gate, which keys on `Write` alone, so the flags carry no bypass. The test is
// what makes the shared `Write | Clobber` arm observable — a `Write`→`Clobber`
// mutant is only distinguishable with `noclobber` set (the shopt integration
// tests kill it).
#[test]
fn clobber_uses_the_same_flags_as_write() {
    assert_eq!(
        RedirectDirection::Clobber.open_flags(),
        O_WRONLY + O_CREAT + O_TRUNC
    );
}
