#![allow(deprecated)]

use core::mem::size_of_val;
use sys::siginfo::SigInfo;

#[test]
fn siginfo_layout_matches_libc() {
    // SAFETY: zero-initialized SigInfo and siginfo_t are valid for
    // layout comparison (all fields are integers).
    let (ours, theirs) = unsafe {
        (
            core::mem::zeroed::<SigInfo>(),
            core::mem::zeroed::<libc::siginfo_t>(),
        )
    };
    let ob = &raw const ours as usize;
    let tb = &raw const theirs as usize;

    assert_eq!(size_of_val(&ours), size_of_val(&theirs));

    macro_rules! check_field {
        ($field:ident) => {
            assert_eq!(size_of_val(&ours.$field), size_of_val(&theirs.$field));
            assert_eq!(
                (&raw const ours.$field as usize) - ob,
                (&raw const theirs.$field as usize) - tb,
            );
        };
    }
    check_field!(si_signo);
    check_field!(si_errno);
    check_field!(si_code);

    // _pad[0] = padding, _pad[1] = si_pid, _pad[2] = si_uid, _pad[3] = si_status
    macro_rules! check_pad {
        ($field:ident, $pad:literal) => {
            assert_eq!(size_of_val(&ours.$field), size_of_val(&theirs._pad[$pad]));
            assert_eq!(
                (&raw const ours.$field as usize) - ob,
                (&raw const theirs._pad[$pad] as usize) - tb,
            );
        };
    }
    check_pad!(si_pid, 1usize);
    check_pad!(si_uid, 2usize);
    check_pad!(si_status, 3usize);
}
