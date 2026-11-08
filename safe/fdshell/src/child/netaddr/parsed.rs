//! The flag collector of the shared net-address parse: the one `while`/`match`
//! over the words, so the `--type`/`--bind`/`--port`/`--backlog` repeats and the
//! positional ADDRESS live in one place and `parse_net_args` keeps only its
//! help bail and the assembly (README "Sockets").

// mutants: -C --lib -- -E 'test(/^child::netaddr::/)'

use core::ffi::CStr;
use error_stack::{Report, bail, ensure};

use builtins::error::{BuiltinError, Suggestion};

use super::SocketType;
use super::address::parse_type;

/// What the flag loop collects: each slot holds the word the flag named, so a
/// repeat is the usage error it has always been, and the assembly in
/// `parse_net_args` reads the slots once the loop is done.
#[derive(Default)]
pub(crate) struct Parsed<'a> {
    pub(crate) ty: Option<SocketType>,
    pub(crate) bind_addr: Option<&'a CStr>,
    pub(crate) port: Option<&'a CStr>,
    pub(crate) backlog: Option<&'a CStr>,
    pub(crate) addr: Option<&'a CStr>,
}

impl<'a> Parsed<'a> {
    /// Walk `refs`, filling every slot. `allows_backlog` is the caller's
    /// `listen` flag: `bind` and `connect` never call `listen(2)`, so only
    /// `listen` gives `--backlog` a meaning, and a flag that parses and does
    /// nothing is a usage error (STYLE.md §4.7).
    pub(crate) fn collect(
        &mut self,
        refs: &[&'a CStr],
        allows_backlog: bool,
    ) -> Result<(), Report<BuiltinError>> {
        let mut i = 0;
        while i < refs.len() {
            let arg = refs.get(i).ok_or(BuiltinError::Never)?;
            i += 1;
            let (key, val) = builtins::argparse::split(arg)?;
            match key {
                b"--type" => {
                    ensure!(self.ty.is_none(), BuiltinError::InvalidArgument("type"));
                    let v = builtins::argparse::next_val(refs, &mut i, val)?;
                    self.ty = Some(parse_type(v)?);
                }
                b"--bind" | b"--addr" => {
                    ensure!(
                        self.bind_addr.is_none(),
                        BuiltinError::InvalidArgument("bind")
                    );
                    self.bind_addr = Some(builtins::argparse::next_val(refs, &mut i, val)?);
                }
                b"--port" => {
                    ensure!(self.port.is_none(), BuiltinError::InvalidArgument("port"));
                    self.port = Some(builtins::argparse::next_val(refs, &mut i, val)?);
                }
                b"--backlog" => {
                    ensure!(
                        allows_backlog,
                        Report::new(BuiltinError::InvalidArgument("backlog"))
                            .attach_opaque(Suggestion("--backlog is accepted only by listen"))
                    );
                    ensure!(
                        self.backlog.is_none(),
                        BuiltinError::InvalidArgument("backlog")
                    );
                    self.backlog = Some(builtins::argparse::next_val(refs, &mut i, val)?);
                }
                a if a.starts_with(b"-") => bail!(BuiltinError::InvalidArgument("flag")),
                _ => {
                    ensure!(
                        self.addr.is_none(),
                        BuiltinError::InvalidArgument("address")
                    );
                    self.addr = Some(arg);
                }
            }
        }
        Ok(())
    }
}
