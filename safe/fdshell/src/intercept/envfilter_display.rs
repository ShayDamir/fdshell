use crate::envfilter::EnvFilter;
use alloc::vec::Vec;

pub(crate) fn print_help() {
    let _ = sys::OUT.write_all(b"Usage: envfilter [OPTIONS]\n");
    let _ = sys::OUT.write_all(b"\n");
    let _ = sys::OUT.write_all(b"Options:\n");
    let _ = sys::OUT.write_all(b"  --allow <pattern>...   Add allowlist glob patterns\n");
    let _ = sys::OUT.write_all(b"  --deny <pattern>...    Add denylist glob patterns\n");
    let _ = sys::OUT.write_all(b"  --list                 Show current rules\n");
    let _ = sys::OUT.write_all(b"  --clear                Clear all rules\n");
    let _ = sys::OUT.write_all(b"  --help, -h             Show this help\n");
    let _ = sys::OUT.write_all(b"\n");
    let _ = sys::OUT.write_all(b"Patterns support * wildcard only.\n");
    let _ = sys::OUT.write_all(b"Allowlist is applied first, then denylist removes from it.");
}

pub(crate) fn print_rules(filter: &EnvFilter) {
    let allow_strs: Vec<&str> = filter
        .allow
        .iter()
        .filter_map(|s| core::str::from_utf8(s.as_bytes().unwrap_or(&[])).ok())
        .collect();
    let deny_strs: Vec<&str> = filter
        .deny
        .iter()
        .filter_map(|s| core::str::from_utf8(s.as_bytes().unwrap_or(&[])).ok())
        .collect();

    if !allow_strs.is_empty() {
        let mut line: alloc::string::String = "allow: ".into();
        line.push_str(&allow_strs.join(" "));
        let _ = sys::OUT.write_all(line.as_bytes());
    }
    if !deny_strs.is_empty() {
        let mut line: alloc::string::String = "deny: ".into();
        line.push_str(&deny_strs.join(" "));
        let _ = sys::OUT.write_all(line.as_bytes());
    }
}
