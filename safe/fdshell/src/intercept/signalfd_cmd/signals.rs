// mutants: -C --lib -C --test -C signalfd -- -E 'test(/^intercept::signalfd_cmd::/) | binary(signalfd)'

//! The signal-name table, and the two directions that read it: the parser (a
//! name or a decimal number → a signal number) and the spelling the list forms
//! print. One table owns both, because a printer and a parser that disagree about
//! a name break the round trip the list forms promise (STYLE §2.6, one owner).
//! The names are the POSIX XCU 1.6 spellings without the `SIG` prefix; the `trap`
//! layer adds its own rules on top (its own `SIG`-prefixed spelling on input,
//! `EXIT`/`0`, the range, the reserved pair), so `signalfd`'s semantics — this
//! module's first caller — stay exactly as they were.

use error_stack::Report;
use sys::ShortCStr;
use sys::signalfd;

use crate::error::cmd::CmdError;

/// Every name the parser accepts, with its number — `signalfd` cannot watch
/// `SIGKILL`, so no `KILL` row exists here and the `trap` layer names that one
/// itself.
const NAMES: &[(&[u8], i32)] = &[
    (b"HUP", signalfd::SIGHUP),
    (b"INT", signalfd::SIGINT),
    (b"QUIT", signalfd::SIGQUIT),
    (b"ILL", signalfd::SIGILL),
    (b"TRAP", signalfd::SIGTRAP),
    (b"ABRT", signalfd::SIGABRT),
    (b"BUS", signalfd::SIGBUS),
    (b"FPE", signalfd::SIGFPE),
    (b"SEGV", signalfd::SIGSEGV),
    (b"PIPE", signalfd::SIGPIPE),
    (b"ALRM", signalfd::SIGALRM),
    (b"TERM", signalfd::SIGTERM),
    (b"CHLD", signalfd::SIGCHLD),
    (b"CONT", signalfd::SIGCONT),
    (b"STOP", signalfd::SIGSTOP),
    (b"TSTP", signalfd::SIGTSTP),
    (b"TTIN", signalfd::SIGTTIN),
    (b"TTOU", signalfd::SIGTTOU),
    (b"URG", signalfd::SIGURG),
    (b"XCPU", signalfd::SIGXCPU),
    (b"XFSZ", signalfd::SIGXFSZ),
    (b"VTALRM", signalfd::SIGVTALRM),
    (b"PROF", signalfd::SIGPROF),
    (b"WINCH", signalfd::SIGWINCH),
    (b"IO", signalfd::SIGIO),
    (b"PWR", signalfd::SIGPWR),
    (b"SYS", signalfd::SIGSYS),
    (b"USR1", signalfd::SIGUSR1),
    (b"USR2", signalfd::SIGUSR2),
];

/// Resolve a signal name (e.g. `INT`, `USR1`) or a decimal number to a signal
/// number: the number spelling first (it is not a name, and the range is the
/// caller's decision), then the table. `None` when the operand is neither, so
/// each caller reports its own wording.
pub(crate) fn resolve(bytes: &[u8]) -> Option<i32> {
    if let Some(sig) = number(bytes) {
        return Some(sig);
    }
    table(bytes)
}

/// The decimal spelling of a signal, as `signalfd %s 2` and `trap 'x' 5` give it.
fn number(bytes: &[u8]) -> Option<i32> {
    core::str::from_utf8(bytes).ok()?.parse::<i32>().ok()
}

/// The table's answer for a name; `None` when the table has no such name.
fn table(bytes: &[u8]) -> Option<i32> {
    NAMES
        .iter()
        .find(|(name, _)| *name == bytes)
        .map(|(_, sig)| *sig)
}

/// The table's spelling of `sig` — the list form's side of the round trip. `None`
/// when the table has no name for it, and the printer then renders the decimal
/// number, which re-inputs just as well (measured: bash prints `32` for the one
/// signal whose number it cannot name).
pub(crate) fn signal_name(sig: i32) -> Option<&'static [u8]> {
    NAMES.iter().find(|(_, n)| *n == sig).map(|(name, _)| *name)
}

/// `signalfd`'s resolver: the shared table, with `signalfd`'s error wording.
pub(crate) fn parse_signal(bytes: &[u8], arg: &ShortCStr) -> Result<i32, Report<CmdError>> {
    resolve(bytes).ok_or_else(|| Report::new(CmdError::SignalfdBadSignal { value: arg.clone() }))
}
