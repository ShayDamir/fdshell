//! ADDRESS resolution and value helpers for the `bind`/`listen` parsers.

use core::ffi::CStr;
use error_stack::{Report, ResultExt, ensure};
use sys::ShortCStr;

use builtins::error::{BuiltinError, Suggestion};

use super::{Address, SUN_PATH_MAX};

/// Exactly one ADDRESS form: the positional (UDS) or `--bind`/`--port`
/// (AF_INET v4). Mixing a positional address with `--bind`/`--port` is a
/// usage error.
pub(super) fn make_address(
    addr: Option<&CStr>,
    bind_addr: Option<&CStr>,
    port: Option<&CStr>,
) -> Result<Address, Report<BuiltinError>> {
    if addr.is_some() && (bind_addr.is_some() || port.is_some()) {
        return Err(
            Report::new(BuiltinError::InvalidArgument("address")).attach_opaque(Suggestion(
                "Use the positional ADDRESS or --bind/--port, not both",
            )),
        );
    }
    match (addr, bind_addr, port) {
        (Some(a), None, None) => uds_address(a),
        (None, Some(a), Some(p)) => Ok(Address::Inet {
            addr: to_short(a.to_bytes())?,
            port: parse_port(p)?,
        }),
        (None, Some(_), None) => Err(Report::new(BuiltinError::InvalidArgument("port"))
            .attach_opaque(Suggestion("Pass the port with --port N"))),
        (None, None, Some(_)) => Err(Report::new(BuiltinError::InvalidArgument("bind"))
            .attach_opaque(Suggestion("Pass the address with --bind ADDR"))),
        (None, None, None) => Err(Report::new(BuiltinError::MissingArgument("address"))
            .attach_opaque(Suggestion("Pass an @name, a path, or --bind ADDR --port N"))),
        // The `(Some(_), Some(_), _)` mix is rejected by the early return.
        _ => Err(Report::new(BuiltinError::Never)),
    }
}

/// `@name` is an abstract-namespace name; anything else is a filesystem
/// path (the kernel resolves it against the CWD). Both are capped at
/// `SUN_PATH_MAX` bytes.
pub(super) fn uds_address(a: &CStr) -> Result<Address, Report<BuiltinError>> {
    let abstract_ = a.to_bytes().starts_with(b"@");
    let name = a.to_bytes().strip_prefix(b"@").unwrap_or(a.to_bytes());
    ensure!(!name.is_empty(), BuiltinError::InvalidArgument("address"));
    ensure!(
        name.len() <= SUN_PATH_MAX,
        Report::new(BuiltinError::InvalidArgument("address"))
            .attach_opaque(Suggestion("Address must be at most 107 bytes"))
    );
    let name = to_short(name)?;
    if abstract_ {
        Ok(Address::UdsAbstract(name))
    } else {
        Ok(Address::UdsPath(name))
    }
}

/// The input comes from a `CStr`, which carries no inner NULs, so
/// `from_vec` cannot fail.
fn to_short<P: AsRef<[u8]>>(s: P) -> Result<ShortCStr, Report<BuiltinError>> {
    ShortCStr::from_vec(s.as_ref().to_vec()).change_context(BuiltinError::Never)
}

/// Port number, `0..=65535` (`0` lets the kernel pick one).
pub(super) fn parse_port(v: &CStr) -> Result<u16, Report<BuiltinError>> {
    let text =
        core::str::from_utf8(v.to_bytes()).change_context(BuiltinError::InvalidArgument("port"))?;
    text.parse::<u16>()
        .change_context(BuiltinError::InvalidArgument("port"))
        .attach_opaque(Suggestion("Port must be a number in 0..=65535"))
}

/// Backlog length, `0..=i32::MAX` (the kernel's `listen` cap).
pub(super) fn parse_backlog(v: &CStr) -> Result<i32, Report<BuiltinError>> {
    let n = crate::child::fdops::args::number(v, "backlog")?;
    ensure!(
        (0..=i32::MAX as i64).contains(&n),
        Report::new(BuiltinError::InvalidArgument("backlog"))
            .attach_opaque(Suggestion("Backlog must be in 0..=2147483647"))
    );
    Ok(n as i32)
}
