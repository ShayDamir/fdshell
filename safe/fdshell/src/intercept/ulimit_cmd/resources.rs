use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use sys::rlimit;

/// A display unit: its name and the user→kernel scale factor.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct Unit {
    pub(super) name: &'static str,
    pub(super) scale: u64,
}

const fn unit(name: &'static str, scale: u64) -> Unit {
    Unit { name, scale }
}

const BLOCKS: Unit = unit("blocks", 1024);
const KBYTES: Unit = unit("kbytes", 1024);
const SECONDS: Unit = unit("seconds", 1);

/// One resource: its flag letter, kernel resource, name, and display unit.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct Resource {
    pub(super) flag: u8,
    pub(super) id: u32,
    pub(super) name: &'static str,
    pub(super) unit: Option<Unit>,
}

const fn res(flag: u8, id: u32, name: &'static str, unit: Option<Unit>) -> Resource {
    Resource {
        flag,
        id,
        name,
        unit,
    }
}

const CORE: Resource = res(b'c', rlimit::CORE, "core file size", Some(BLOCKS));
const DATA: Resource = res(b'd', rlimit::DATA, "data seg size", Some(KBYTES));
const FSIZE: Resource = res(b'f', rlimit::FSIZE, "file size", Some(BLOCKS));
const MEMLOCK: Resource = res(b'l', rlimit::MEMLOCK, "max locked memory", Some(KBYTES));
const RSS: Resource = res(b'm', rlimit::RSS, "max memory size", Some(KBYTES));
const NOFILE: Resource = res(b'n', rlimit::NOFILE, "open files", None);
const STACK: Resource = res(b's', rlimit::STACK, "stack size", Some(KBYTES));
const CPU: Resource = res(b't', rlimit::CPU, "cpu time", Some(SECONDS));
const NPROC: Resource = res(b'u', rlimit::NPROC, "max user processes", None);
const AS: Resource = res(b'v', rlimit::AS, "virtual memory", Some(KBYTES));

/// All resources in bash's `ulimit -a` order.
pub(super) const RESOURCES: [Resource; 10] = [
    CORE, DATA, FSIZE, MEMLOCK, RSS, NOFILE, STACK, CPU, NPROC, AS,
];

/// bash's default resource: file size (`-f`).
pub(super) const DEFAULT: Resource = FSIZE;

impl Resource {
    /// The user→kernel scale factor (1 for unitless resources).
    pub(super) const fn scale(self) -> u64 {
        match self.unit {
            Some(unit) => unit.scale,
            None => 1,
        }
    }
}

/// The resource for a flag letter, if any.
pub(super) fn by_flag(flag: u8) -> Option<Resource> {
    RESOURCES.iter().find(|r| r.flag == flag).copied()
}

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
