//! `verity` argument parsing. The first token is a `%var` fd-variable (fd form
//! only, like `statx`); the flags follow. `--algo`/`--digest` values are plain
//! words read from `args` (originals), so no substitution is involved.

use core::ffi::CStr;
use error_stack::{Report, bail, ensure};
use sys::ShortCStr;

use builtins::error::BuiltinError;

use crate::child::fdops::args::var_arg;
use crate::child::flags::{flag_val, split_eq};

pub(super) use super::config::VerityConfig;

pub(super) fn verity_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<VerityConfig, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let var = var_arg(args)?;
    let mut cfg = VerityConfig {
        var,
        enable: false,
        algo: sys::fsverity::FS_VERITY_HASH_ALG_SHA256,
        expected: None,
        dump: None,
    };
    cfg.parse_flags(args)?;
    ensure!(
        !(cfg.enable && cfg.expected.is_some()),
        BuiltinError::InvalidArgument("--digest")
    );
    ensure!(
        cfg.dump.is_none() || !cfg.enable,
        BuiltinError::InvalidArgument("--dump")
    );
    ensure!(
        cfg.dump.is_none() || cfg.expected.is_none(),
        BuiltinError::InvalidArgument("--dump")
    );
    Ok(cfg)
}

impl VerityConfig {
    fn parse_flags(&mut self, args: &[ShortCStr]) -> Result<(), Report<BuiltinError>> {
        let mut i = 1;
        while i < args.len() {
            let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
            let (key, val) = split_eq(arg)?;
            i += 1;
            match key {
                b"--enable" => {
                    ensure!(val.is_none(), BuiltinError::InvalidArgument("--enable"));
                    ensure!(!self.enable, BuiltinError::InvalidArgument("--enable"));
                    self.enable = true;
                }
                b"--algo" => self.algo = parse_algo(flag_val(val, args, &mut i, "--algo")?)?,
                b"--digest" => {
                    self.expected = Some(super::hex::parse_hex(flag_val(
                        val, args, &mut i, "--digest",
                    )?)?)
                }
                b"--dump" | b"--offset" | b"--length" => {
                    super::dump::apply(key, val, args, &mut i, &mut self.dump)?
                }
                _ => bail!(BuiltinError::InvalidArgument("flag")),
            }
        }
        Ok(())
    }
}

fn parse_algo(v: &[u8]) -> Result<u32, Report<BuiltinError>> {
    if v == b"sha256" {
        Ok(sys::fsverity::FS_VERITY_HASH_ALG_SHA256)
    } else if v == b"sha512" {
        Ok(sys::fsverity::FS_VERITY_HASH_ALG_SHA512)
    } else {
        bail!(BuiltinError::InvalidArgument("algo"))
    }
}
