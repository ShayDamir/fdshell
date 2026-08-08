use crate::envfilter::EnvFilter;
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

pub(crate) fn rules_text(filter: &EnvFilter) -> ShortCStr {
    let mut result = ShortCStr::new();
    if !filter.allow.is_empty() {
        result.push_cstr(c"allow: ");
        for (i, pattern) in filter.allow.iter().enumerate() {
            if i > 0 {
                result.push_cstr(c" ");
            }
            result.push_str(pattern);
        }
        result.push_cstr(c"\n");
    }
    if !filter.deny.is_empty() {
        result.push_cstr(c"deny: ");
        for (i, pattern) in filter.deny.iter().enumerate() {
            if i > 0 {
                result.push_cstr(c" ");
            }
            result.push_str(pattern);
        }
        result.push_cstr(c"\n");
    }
    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
