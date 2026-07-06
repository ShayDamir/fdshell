use crate::envfilter::EnvFilter;

pub(crate) fn print_help() {
    println!("Usage: envfilter [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --allow <pattern>...   Add allowlist glob patterns");
    println!("  --deny <pattern>...    Add denylist glob patterns");
    println!("  --list                 Show current rules");
    println!("  --clear                Clear all rules");
    println!("  --help, -h             Show this help");
    println!();
    println!("Patterns support * wildcard only.");
    println!("Allowlist is applied first, then denylist removes from it.");
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
        println!("allow: {}", allow_strs.join(" "));
    }
    if !deny_strs.is_empty() {
        println!("deny: {}", deny_strs.join(" "));
    }
}
