// mutants: -C --lib -C --test -C ulimit -- -E 'test(/^intercept::ulimit_cmd::/) | binary(ulimit)'

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::intercept::ulimit_cmd::parse::{Keyword, Value};
use crate::numarg::{ULIMIT_VALUE, decimal};
use sys::ShortCStr;

/// A value word: a digit run in bash's strict grammar (`crate::numarg`'s
/// `ULIMIT_VALUE` — no whitespace, no sign), `unlimited`, or a keyword naming
/// the current limit it resolves to at apply time.
pub(super) fn parse_value(text: &ShortCStr) -> Result<Value, Report<CmdError>> {
    let bytes = text.as_bytes().change_context(CmdError::Never)?;
    let keyword = match bytes {
        b"soft" => Some(Keyword::Soft),
        b"hard" => Some(Keyword::Hard),
        _ => None,
    };
    let amount = match keyword {
        // A keyword carries no number; `set.rs` resolves it at apply time.
        Some(_) => 0,
        None if bytes == b"unlimited" => sys::rlimit::UNLIMITED,
        None => decimal(bytes, &ULIMIT_VALUE)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(CmdError::UlimitBadValue {
                value: text.clone(),
            })?,
    };
    Ok(Value {
        amount,
        keyword,
        text: text.clone(),
    })
}
