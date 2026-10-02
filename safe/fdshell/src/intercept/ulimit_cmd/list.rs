//! `ulimit -a`: one line per resource, in bash's order.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use sys::rlimit;

use super::resources::{RESOURCES, Resource};

/// `ulimit -a`: one line per resource, in bash's order.
pub(super) fn list(hard: bool) -> Result<Vec<u8>, Report<CmdError>> {
    let mut out = Vec::new();
    for res in &RESOURCES {
        let lim = rlimit::get(res.id).change_context(CmdError::UlimitGet)?;
        let raw = if hard { lim.hard } else { lim.soft };
        let flag = res.flag as char;
        let v = value_bytes(raw, *res);
        let line = match res.unit {
            Some(unit) => format!("{:<26} ({}, -{}) {}", res.name, unit.name, flag, v),
            None => format!("{:<26}        (-{}) {}", res.name, flag, v),
        };
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}

/// A limit as bash prints it: `unlimited`, or the count in the resource's unit.
pub(super) fn value_bytes(value: u64, res: Resource) -> String {
    if value == rlimit::UNLIMITED {
        return "unlimited".to_string();
    }
    match res.unit {
        Some(unit) => (value / unit.scale).to_string(),
        None => value.to_string(),
    }
}
