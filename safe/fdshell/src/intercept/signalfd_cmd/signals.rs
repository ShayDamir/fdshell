use error_stack::Report;
use sys::ShortCStr;
use sys::signalfd;

use crate::error::cmd::CmdError;

/// Resolve a signal name (e.g. `INT`, `USR1`) or number to a signal number.
pub(super) fn parse_signal(bytes: &[u8], arg: &ShortCStr) -> Result<i32, Report<CmdError>> {
    if let Ok(n) = core::str::from_utf8(bytes)
        && let Ok(v) = n.parse::<i32>()
    {
        return Ok(v);
    }
    match bytes {
        b"HUP" => Ok(signalfd::SIGHUP),
        b"INT" => Ok(signalfd::SIGINT),
        b"QUIT" => Ok(signalfd::SIGQUIT),
        b"ILL" => Ok(signalfd::SIGILL),
        b"TRAP" => Ok(signalfd::SIGTRAP),
        b"ABRT" => Ok(signalfd::SIGABRT),
        b"BUS" => Ok(signalfd::SIGBUS),
        b"FPE" => Ok(signalfd::SIGFPE),
        b"SEGV" => Ok(signalfd::SIGSEGV),
        b"PIPE" => Ok(signalfd::SIGPIPE),
        b"ALRM" => Ok(signalfd::SIGALRM),
        b"TERM" => Ok(signalfd::SIGTERM),
        b"CHLD" => Ok(signalfd::SIGCHLD),
        b"CONT" => Ok(signalfd::SIGCONT),
        b"STOP" => Ok(signalfd::SIGSTOP),
        b"TSTP" => Ok(signalfd::SIGTSTP),
        b"TTIN" => Ok(signalfd::SIGTTIN),
        b"TTOU" => Ok(signalfd::SIGTTOU),
        b"URG" => Ok(signalfd::SIGURG),
        b"XCPU" => Ok(signalfd::SIGXCPU),
        b"XFSZ" => Ok(signalfd::SIGXFSZ),
        b"VTALRM" => Ok(signalfd::SIGVTALRM),
        b"PROF" => Ok(signalfd::SIGPROF),
        b"WINCH" => Ok(signalfd::SIGWINCH),
        b"IO" => Ok(signalfd::SIGIO),
        b"PWR" => Ok(signalfd::SIGPWR),
        b"SYS" => Ok(signalfd::SIGSYS),
        b"USR1" => Ok(signalfd::SIGUSR1),
        b"USR2" => Ok(signalfd::SIGUSR2),
        _ => Err(Report::new(CmdError::SignalfdBadSignal {
            value: arg.clone(),
        })),
    }
}
