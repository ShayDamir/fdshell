use super::*;
use hashbrown::HashMap;

fn entry_has_prefix(entries: &[ExportedCStr], prefix: &[u8]) -> bool {
    entries
        .iter()
        .any(|e| e.as_ref().to_bytes().starts_with(prefix))
}

fn count_prefix(entries: &[ExportedCStr], prefix: &[u8]) -> usize {
    entries
        .iter()
        .filter(|e| e.as_ref().to_bytes().starts_with(prefix))
        .count()
}

fn find_entry<'a>(entries: &'a [ExportedCStr], prefix: &[u8]) -> Option<&'a str> {
    entries
        .iter()
        .find(|e| e.as_ref().to_bytes().starts_with(prefix))
        .map(|e| e.to_str().unwrap())
}

#[test]
fn get_environ_includes_pid() {
    let exports: HashMap<ShortCStr, ShortCStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(12345, &[], &exports, &filter, None);

    assert!(
        entry_has_prefix(&result, b"FDSHELL_PID="),
        "FDSHELL_PID should be present"
    );
    let pid_entry = find_entry(&result, b"FDSHELL_PID=").unwrap();
    assert_eq!(pid_entry, "FDSHELL_PID=12345");
}

#[test]
fn get_environ_excludes_socket_when_none() {
    let exports: HashMap<ShortCStr, ShortCStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(1, &[], &exports, &filter, None);

    assert!(
        !entry_has_prefix(&result, b"FDSHELL_SOCKET="),
        "FDSHELL_SOCKET should not be present when exec_sock is None"
    );
}

#[test]
fn get_environ_merges_exports() {
    let mut exports = HashMap::new();
    exports.insert(c"MY_VAR".into(), c"my_value".into());
    exports.insert(c"OTHER_VAR".into(), c"other_value".into());
    let filter = EnvFilter::new();
    let result = get_environ(1, &[], &exports, &filter, None);

    assert!(entry_has_prefix(&result, b"MY_VAR="));
    assert!(entry_has_prefix(&result, b"OTHER_VAR="));
    assert_eq!(find_entry(&result, b"MY_VAR=").unwrap(), "MY_VAR=my_value");
    assert_eq!(
        find_entry(&result, b"OTHER_VAR=").unwrap(),
        "OTHER_VAR=other_value"
    );
}

#[test]
fn get_environ_filters_exports_by_deny() {
    let mut exports = HashMap::new();
    exports.insert(c"ALLOWED".into(), c"yes".into());
    exports.insert(c"DENIED".into(), c"no".into());

    let mut filter = EnvFilter::new();
    filter.deny.push(c"DENIED".into());

    let result = get_environ(1, &[], &exports, &filter, None);

    assert!(entry_has_prefix(&result, b"ALLOWED="));
    assert!(
        !entry_has_prefix(&result, b"DENIED="),
        "DENIED var should be filtered out"
    );
}

#[test]
fn get_environ_filters_exports_by_allow() {
    let mut exports = HashMap::new();
    exports.insert(c"ALLOWED".into(), c"yes".into());
    exports.insert(c"NOT_ALLOWED".into(), c"no".into());

    let mut filter = EnvFilter::new();
    filter.allow.push(c"ALLOWED".into());

    let result = get_environ(1, &[], &exports, &filter, None);

    assert!(entry_has_prefix(&result, b"ALLOWED="));
    assert!(
        !entry_has_prefix(&result, b"NOT_ALLOWED="),
        "NOT_ALLOWED var should be filtered out by allowlist"
    );
}

#[test]
fn get_environ_excludes_fdshell_vars_from_environ() {
    // Ensure FDSHELL_PID and FDSHELL_SOCKET are not in current environ
    // (they shouldn't be in test env, but verify the function handles them)
    let exports: HashMap<ShortCStr, ShortCStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(999, &[], &exports, &filter, None);

    // Should have exactly one FDSHELL_PID (added by function)
    assert_eq!(count_prefix(&result, b"FDSHELL_PID="), 1);
}

#[test]
fn get_environ_empty_exports() {
    let exports: HashMap<ShortCStr, ShortCStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(42, &[], &exports, &filter, None);

    assert_eq!(count_prefix(&result, b"FDSHELL_PID="), 1);
    assert!(!entry_has_prefix(&result, b"FDSHELL_SOCKET="));
}
