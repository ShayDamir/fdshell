// mutants: -C --lib -C --test -C ulimit -- -E 'test(/^intercept::ulimit_cmd::/) | binary(ulimit)'

use error_stack::{Report, ResultExt, bail};

use crate::error::cmd::CmdError;
use crate::intercept::ulimit_cmd::resources;
use sys::ShortCStr;

/// A limit value word: the parsed amount and the original text (for errors).
/// A keyword word carries no number: `set.rs` resolves it against the current
/// limits at apply time.
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct Value {
    pub(super) amount: u64,
    pub(super) keyword: Option<Keyword>,
    pub(super) text: ShortCStr,
}

/// A value word naming the current limit it resolves to (bash's `soft` and
/// `hard` spellings); `unlimited` is an amount, not a keyword.
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub(super) enum Keyword {
    Soft,
    Hard,
}

/// Parsed `ulimit` arguments: scope flags, at most one resource, the value.
#[cfg_attr(test, derive(Debug))]
pub(super) struct Parsed {
    pub(super) list: bool,
    pub(super) hard: bool,
    pub(super) soft: bool,
    pub(super) resource: Option<resources::Resource>,
    pub(super) value: Option<Value>,
}

/// Flags in any order, combinable in one word; `--` is skipped; the first
/// non-flag word is the value, later non-flag words are ignored (bash).
pub(super) fn parse(args: &[ShortCStr]) -> Result<Parsed, Report<CmdError>> {
    let mut list = false;
    let mut hard = false;
    let mut soft = false;
    let mut resource: Option<resources::Resource> = None;
    let mut value: Option<Value> = None;
    for arg in args {
        if arg.eq_bytes(b"--") {
            continue;
        }
        let bytes = arg.as_bytes().change_context(CmdError::Never)?;
        if bytes.first() == Some(&b'-') && bytes.len() > 1 {
            for &ch in bytes.get(1..).unwrap_or(&[]) {
                match ch {
                    b'H' => hard = true,
                    b'S' => soft = true,
                    b'a' => list = true,
                    other => match resources::by_flag(other) {
                        Some(res) => set_resource(&mut resource, res)?,
                        None => bail!(CmdError::UlimitInvalidOption {
                            flag: other as char
                        }),
                    },
                }
            }
        } else if value.is_none() {
            value = Some(value::parse_value(arg)?);
        }
    }
    Ok(Parsed {
        list,
        hard,
        soft,
        resource,
        value,
    })
}

/// A second resource flag is a usage error (bash's odd `-cn` behavior is not replicated).
fn set_resource(
    resource: &mut Option<resources::Resource>,
    next: resources::Resource,
) -> Result<(), Report<CmdError>> {
    if resource.is_some() {
        bail!(CmdError::UlimitUsage);
    }
    *resource = Some(next);
    Ok(())
}

mod value;
