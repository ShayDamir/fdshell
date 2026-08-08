use crate::envfilter::EnvFilter;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn help_text() -> &'static [u8] {
    b"Usage: envfilter [OPTIONS]\n\
                  \nOptions:\n  \
                  --allow <pattern>...   Add allowlist glob patterns\n  \
                  --deny <pattern>...    Add denylist glob patterns\n  \
                  --list                 Show current rules\n  \
                  --clear                Clear all rules\n  \
                  --help, -h             Show this help\n\
                  \nPatterns support * wildcard only.\n\
                  Allowlist is applied first, then denylist removes from it."
}

pub(crate) fn rules_text(
    filter: &EnvFilter,
) -> Result<ShortCStr, Report<crate::error::cmd::CmdError>> {
    let mut result = ShortCStr::new();
    if !filter.allow.is_empty() {
        result
            .push_slice(b"allow: ")
            .change_context(crate::error::cmd::CmdError::Never)?;
        for (i, pattern) in filter.allow.iter().enumerate() {
            if i > 0 {
                result
                    .push(b' ')
                    .change_context(crate::error::cmd::CmdError::Never)?;
            }
            result
                .push_str(pattern)
                .change_context(crate::error::cmd::CmdError::Never)?;
        }
        result
            .push(b'\n')
            .change_context(crate::error::cmd::CmdError::Never)?;
    }
    if !filter.deny.is_empty() {
        result
            .push_slice(b"deny: ")
            .change_context(crate::error::cmd::CmdError::Never)?;
        for (i, pattern) in filter.deny.iter().enumerate() {
            if i > 0 {
                result
                    .push(b' ')
                    .change_context(crate::error::cmd::CmdError::Never)?;
            }
            result
                .push_str(pattern)
                .change_context(crate::error::cmd::CmdError::Never)?;
        }
        result
            .push(b'\n')
            .change_context(crate::error::cmd::CmdError::Never)?;
    }
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
