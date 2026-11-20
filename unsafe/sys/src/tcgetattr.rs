// mutants: -C --lib -C --test -C tcgetattr -- -E 'binary(tcgetattr)'

//! `tcgetattr(3)` / `tcsetattr(3)`: read a terminal's mode and write one back.
//!
//! [`Termios`] mirrors `libc::termios` field for field with plain field types,
//! so the safe crates can read and set the flags without naming libc
//! (STYLE §5.1, §7.1). Both wrappers take a raw `i32` fd — the established
//! shape for sys wrappers here (`dup::close(fd: i32)`) — and report through
//! [`crate::cvt`] / [`SyscallError`].
//!
//! [`Termios::enter_raw`] applies the minimal editor set measured on a real
//! pty: clear `ICANON | ECHO | ISIG | IXON`, set `VMIN = 1`, `VTIME = 0`, touch
//! nothing else. Save/restore is the caller's job, and the saved value is the
//! baseline: bash's readline restores the mode it saved before every command,
//! so its raw mode is invisible to the commands it runs — a shell that wants
//! raw keys must keep its own baseline
//! (LESSONS: "A pty's termios baseline is host state, so pins are deltas against a snapshot").

use crate::SyscallError;

/// Control-character slots in a termios on x86_64 Linux (`libc::NCCS`).
pub const NCCS: usize = libc::NCCS;

/// `tcsetattr` action: apply the mode immediately (the exit path).
pub const TCSANOW: i32 = libc::TCSANOW;

/// `tcsetattr` action: wait for the output queue to drain, then apply.
pub const TCSADRAIN: i32 = libc::TCSADRAIN;

/// `tcsetattr` action: throw away queued input, then apply.
pub const TCSAFLUSH: i32 = libc::TCSAFLUSH;

/// A terminal mode — a plain mirror of `libc::termios`, one field per libc
/// field, so no libc type reaches the safe crates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Termios {
    /// Input flags (`c_iflag`).
    pub iflag: u32,
    /// Output flags (`c_oflag`).
    pub oflag: u32,
    /// Control flags (`c_cflag`).
    pub cflag: u32,
    /// Local flags (`c_lflag`).
    pub lflag: u32,
    /// Line discipline (`c_line`).
    pub line: u8,
    /// Special control characters (`c_cc`), indexed by `libc::VMIN`,
    /// `libc::VTIME`, …
    pub cc: [u8; NCCS],
    /// Input speed (`c_ispeed`).
    pub ispeed: u32,
    /// Output speed (`c_ospeed`).
    pub ospeed: u32,
}

/// Pin the mirror to the libc layout, so a libc/uapi drift fails at compile
/// time rather than silently shifting a field.
const _: () = assert!(core::mem::size_of::<Termios>() == core::mem::size_of::<libc::termios>());

/// Pin the control-character indexes `enter_raw` writes to, so its
/// bounds-checked helper provably never skips one.
const _: () = assert!(libc::VMIN < NCCS && libc::VTIME < NCCS);

impl Termios {
    /// `tcgetattr(3)`: read the mode of the terminal behind `fd`.
    pub fn save(fd: i32) -> Result<Self, SyscallError> {
        // SAFETY: `libc::termios` holds only integers and a byte array, so
        // zeroed bytes are a valid value to hand to the kernel.
        let mut raw: libc::termios = unsafe { core::mem::zeroed() };
        // SAFETY: `&mut raw` is an aligned pointer to a valid `libc::termios`
        // the kernel writes; any `fd` is a legal argument and a bad one answers
        // `EBADF`, which `cvt` reports.
        crate::cvt(unsafe { libc::tcgetattr(fd, &mut raw) as isize })?;
        Ok(Self {
            iflag: raw.c_iflag,
            oflag: raw.c_oflag,
            cflag: raw.c_cflag,
            lflag: raw.c_lflag,
            line: raw.c_line,
            cc: raw.c_cc,
            ispeed: raw.c_ispeed,
            ospeed: raw.c_ospeed,
        })
    }

    /// `tcsetattr(3)`: write this mode to the terminal behind `fd`. `action` is
    /// [`TCSANOW`], [`TCSADRAIN`] (output may still be queued) or
    /// [`TCSAFLUSH`].
    pub fn apply(&self, fd: i32, action: i32) -> Result<(), SyscallError> {
        let raw = libc::termios {
            c_iflag: self.iflag,
            c_oflag: self.oflag,
            c_cflag: self.cflag,
            c_lflag: self.lflag,
            c_line: self.line,
            c_cc: self.cc,
            c_ispeed: self.ispeed,
            c_ospeed: self.ospeed,
        };
        // SAFETY: `raw` is a fully initialized `libc::termios` the kernel only
        // reads; an unknown `action` answers `EINVAL` and a bad `fd` `EBADF`,
        // both reported by `cvt`.
        crate::cvt(unsafe { libc::tcsetattr(fd, action, &raw) as isize })?;
        Ok(())
    }

    /// Enter the minimal editor mode measured on a real pty: clear
    /// `ICANON | ECHO | ISIG | IXON`, set `VMIN = 1` and `VTIME = 0`, and touch
    /// nothing else — every other bit keeps the value the caller saved. Each
    /// flag is cleared on its own line: an `|`-mask of these single-bit flags
    /// is a `^` away from itself (LESSONS: "`|` on disjoint bit flags produces equivalent mutants").
    pub fn enter_raw(&mut self) {
        self.iflag &= !libc::IXON;
        self.lflag &= !libc::ICANON;
        self.lflag &= !libc::ECHO;
        self.lflag &= !libc::ISIG;
        self.set_cc(libc::VMIN, 1);
        self.set_cc(libc::VTIME, 0);
    }

    /// Write one control character without indexing (STYLE §4.11); the pinned
    /// indexes above are always in range.
    fn set_cc(&mut self, index: usize, value: u8) {
        if let Some(slot) = self.cc.get_mut(index) {
            *slot = value;
        }
    }
}
