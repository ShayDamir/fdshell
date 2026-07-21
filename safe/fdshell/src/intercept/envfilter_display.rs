use crate::envfilter::EnvFilter;

pub(crate) fn print_help() {
    let help = b"Usage: envfilter [OPTIONS]\n\
                 \nOptions:\n  \
                 --allow <pattern>...   Add allowlist glob patterns\n  \
                 --deny <pattern>...    Add denylist glob patterns\n  \
                 --list                 Show current rules\n  \
                 --clear                Clear all rules\n  \
                 --help, -h             Show this help\n\
                 \nPatterns support * wildcard only.\n\
                 Allowlist is applied first, then denylist removes from it.";
    let _ = sys::OUT.write_all(help);
}

pub(crate) fn print_rules(filter: &EnvFilter) {
    if !filter.allow.is_empty() {
        let _ = sys::OUT.write_all(b"allow: ");
        for (i, pattern) in filter.allow.iter().enumerate() {
            if i > 0 {
                let _ = sys::OUT.write_all(b" ");
            }
            let _ = sys::OUT.write_str(pattern);
        }
        let _ = sys::OUT.write_all(b"\n");
    }
    if !filter.deny.is_empty() {
        let _ = sys::OUT.write_all(b"deny: ");
        for (i, pattern) in filter.deny.iter().enumerate() {
            if i > 0 {
                let _ = sys::OUT.write_all(b" ");
            }
            let _ = sys::OUT.write_str(pattern);
        }
        let _ = sys::OUT.write_all(b"\n");
    }
}
