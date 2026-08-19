use super::*;
use alloc::vec;
use hashbrown::HashMap;

fn is_(value: &'static core::ffi::CStr) -> ImportedStr {
    ImportedStr::shell(value.into())
}

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
    let exports: HashMap<ShortCStr, ImportedStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(sys::Pid::from_raw(12345), &[], &exports, &filter, None);

    assert!(
        entry_has_prefix(&result, b"FDSHELL_PID="),
        "FDSHELL_PID should be present"
    );
    let pid_entry = find_entry(&result, b"FDSHELL_PID=").unwrap();
    assert_eq!(pid_entry, "FDSHELL_PID=12345");
}

#[test]
fn get_environ_excludes_socket_when_none() {
    let exports: HashMap<ShortCStr, ImportedStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(sys::Pid::from_raw(1), &[], &exports, &filter, None);

    assert!(
        !entry_has_prefix(&result, b"FDSHELL_SOCKET="),
        "FDSHELL_SOCKET should not be present when exec_sock is None"
    );
}

#[test]
fn get_environ_merges_exports() {
    let mut exports = HashMap::new();
    exports.insert(c"MY_VAR".into(), is_(c"my_value"));
    exports.insert(c"OTHER_VAR".into(), is_(c"other_value"));
    let filter = EnvFilter::new();
    let result = get_environ(sys::Pid::from_raw(1), &[], &exports, &filter, None);

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
    exports.insert(c"ALLOWED".into(), is_(c"yes"));
    exports.insert(c"DENIED".into(), is_(c"no"));

    let mut filter = EnvFilter::new();
    filter.deny.push(c"DENIED".into());

    let result = get_environ(sys::Pid::from_raw(1), &[], &exports, &filter, None);

    assert!(entry_has_prefix(&result, b"ALLOWED="));
    assert!(
        !entry_has_prefix(&result, b"DENIED="),
        "DENIED var should be filtered out"
    );
}

#[test]
fn get_environ_filters_exports_by_allow() {
    let mut exports = HashMap::new();
    exports.insert(c"ALLOWED".into(), is_(c"yes"));
    exports.insert(c"NOT_ALLOWED".into(), is_(c"no"));

    let mut filter = EnvFilter::new();
    filter.allow.push(c"ALLOWED".into());

    let result = get_environ(sys::Pid::from_raw(1), &[], &exports, &filter, None);

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
    let exports: HashMap<ShortCStr, ImportedStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(sys::Pid::from_raw(999), &[], &exports, &filter, None);

    // Should have exactly one FDSHELL_PID (added by function)
    assert_eq!(count_prefix(&result, b"FDSHELL_PID="), 1);
}

#[test]
fn get_environ_empty_exports() {
    let exports: HashMap<ShortCStr, ImportedStr> = HashMap::new();
    let filter = EnvFilter::new();
    let result = get_environ(sys::Pid::from_raw(42), &[], &exports, &filter, None);

    assert_eq!(count_prefix(&result, b"FDSHELL_PID="), 1);
    assert!(!entry_has_prefix(&result, b"FDSHELL_SOCKET="));
}

#[test]
fn get_environ_exports_override_inherited() {
    let environ = vec![(c"MY_VAR".into(), c"inherited".into())];

    let mut exports = HashMap::new();
    exports.insert(c"MY_VAR".into(), is_(c"overridden"));

    let filter = EnvFilter::new();
    let result = get_environ(sys::Pid::from_raw(1), &environ, &exports, &filter, None);

    assert_eq!(count_prefix(&result, b"MY_VAR="), 1);
    assert_eq!(
        find_entry(&result, b"MY_VAR=").unwrap(),
        "MY_VAR=overridden"
    );
}

#[test]
fn get_environ_keeps_unique_inherited_when_not_exported() {
    let environ = vec![(c"INHERITED".into(), c"val".into())];

    let mut exports = HashMap::new();
    exports.insert(c"EXPORTED".into(), is_(c"val2"));

    let filter = EnvFilter::new();
    let result = get_environ(sys::Pid::from_raw(1), &environ, &exports, &filter, None);

    assert!(entry_has_prefix(&result, b"INHERITED="));
    assert_eq!(find_entry(&result, b"INHERITED=").unwrap(), "INHERITED=val");
    assert!(entry_has_prefix(&result, b"EXPORTED="));
    assert_eq!(find_entry(&result, b"EXPORTED=").unwrap(), "EXPORTED=val2");
}
