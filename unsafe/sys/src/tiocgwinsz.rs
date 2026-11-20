// mutants: -C --lib -C --test -C tiocgwinsz -- -E 'binary(tiocgwinsz)'

//! `TIOCGWINSZ`: read a terminal's window size.
//!
//! [`Winsize`] is a plain struct mirroring `libc::winsize` (like the crate's
//! `RLimit`), so the safe crates never name libc (STYLE §5.1). The wrapper
//! takes a raw `i32` fd and reports through [`crate::cvt`] / [`SyscallError`].
//!
//! A size of `0 x 0` is a real answer, not an error: a freshly allocated pty
//! reports exactly that until someone sets a size, and rows/columns are only
//! what the kernel was told. A non-terminal fd answers `ENOTTY`
//! (LESSONS: "A pty's termios baseline is host state, so pins are deltas against a snapshot").

use crate::SyscallError;

/// The window size of a terminal — a plain mirror of `libc::winsize`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Winsize {
    /// Rows (`ws_row`).
    pub rows: u16,
    /// Columns (`ws_col`).
    pub cols: u16,
    /// Horizontal size in pixels (`ws_xpixel`); `0` unless the terminal reports
    /// pixels.
    pub xpixel: u16,
    /// Vertical size in pixels (`ws_ypixel`); `0` unless the terminal reports
    /// pixels.
    pub ypixel: u16,
}

/// Pin the mirror to the libc layout, so a libc/uapi drift fails at compile
/// time rather than silently shifting a field.
const _: () = assert!(core::mem::size_of::<Winsize>() == core::mem::size_of::<libc::winsize>());

/// `TIOCGWINSZ`: read the window size of the terminal behind `fd`. A pipe or a
/// regular file answers `ENOTTY`.
pub fn get(fd: i32) -> Result<Winsize, SyscallError> {
    // SAFETY: `libc::winsize` holds only integers, so zeroed bytes are a valid
    // value to hand to the kernel.
    let mut raw: libc::winsize = unsafe { core::mem::zeroed() };
    // SAFETY: `&mut raw` is an aligned pointer to a valid `libc::winsize` the
    // kernel writes; the request is the fixed `TIOCGWINSZ` number, and a bad
    // `fd` or a non-terminal answers with an errno `cvt` reports.
    crate::cvt(unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut raw) as isize })?;
    Ok(Winsize {
        rows: raw.ws_row,
        cols: raw.ws_col,
        xpixel: raw.ws_xpixel,
        ypixel: raw.ws_ypixel,
    })
}
