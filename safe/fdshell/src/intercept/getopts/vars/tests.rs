#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::ShellState;
use alloc::vec::Vec;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

/// Read a variable back the way the caller reads it: `None` is **unbound** (bash's
/// `${OPTARG+set}` answers empty for a name it never wrote, plan §2).
fn read(state: &ShellState, name: &[u8]) -> Option<Vec<u8>> {
    state
        .strings
        .get(&s(name))
        .map(|value| value.value.as_bytes().unwrap().to_vec())
}

#[test]
fn the_identifier_rule_accepts_exactly_bashs_set() {
    // Measured on bash 5.3.9, one call each (`set -- -a; getopts "ab:c" <name>`).
    for name in [b"v".as_slice(), b"_v", b"v1", b"a_1", b"z9", b"Z_"] {
        assert!(is_valid_name(&s(name)), "{name:?} is an identifier");
    }
}

#[test]
fn the_identifier_rule_rejects_exactly_bashs_set() {
    // …and the rejects, each answering bash's status 1 with its own message.
    for name in [
        b"1abc".as_slice(),
        b"v.x",
        b"a-b",
        b"%x",
        b"-x",
        b"n=1",
        b"",
    ] {
        assert!(!is_valid_name(&s(name)), "{name:?} is not an identifier");
    }
}

#[test]
fn a_rejected_name_answers_status_one_and_writes_nothing() {
    // Measured `set -- -bval; getopts "ab:c" 1abc`: the scan's commits — the counter
    // and `OPTARG` — **stand**, only the name write fails (plan §2/§3.1). The printed
    // line is pinned in `diagnostic/tests.rs`; here the state effects.
    let mut state = ShellState::new();
    state.set_last_exit(0);
    write_counter(&mut state, 2).unwrap();
    write_optarg(&mut state, Some(s(b"val")));
    report_name(&mut state, &s(b"1abc"), b'a', REPORT_STATUS).unwrap();
    assert_eq!(read(&state, b"1abc"), None, "nothing named");
    assert_eq!(state.last_status.exit_code(), FAILURE_STATUS);
    assert_eq!(read(&state, b"OPTIND"), Some(b"2".to_vec()));
    assert_eq!(read(&state, b"OPTARG"), Some(b"val".to_vec()));
}

#[test]
fn an_accepted_name_gets_the_rows_character_and_its_status() {
    let mut state = ShellState::new();
    report_name(&mut state, &s(b"v"), b'a', REPORT_STATUS).unwrap();
    assert_eq!(read(&state, b"v"), Some(b"a".to_vec()));
    assert_eq!(state.last_status.exit_code(), REPORT_STATUS);
}

#[test]
fn optarg_takes_the_argument_or_nothing_at_all() {
    let mut state = ShellState::new();
    write_optarg(&mut state, Some(s(b"val")));
    assert_eq!(read(&state, b"OPTARG"), Some(b"val".to_vec()));
    write_optarg(&mut state, None);
    assert!(read(&state, b"OPTARG").is_none(), "unbound, not empty");
}

#[test]
fn the_counter_writer_moves_the_field_and_the_visible_row_together() {
    // The row bash's `$OPTIND` reads is the store row, so `set` lists it.
    let mut state = ShellState::new();
    write_counter(&mut state, 2).unwrap();
    assert_eq!(state.optind, 2);
    assert_eq!(read(&state, b"OPTIND"), Some(b"2".to_vec()));
}

#[test]
fn the_counter_writer_leaves_the_cursor_where_the_row_left_it() {
    // Plan §3.4: `set_var(c"OPTIND", …)` is the name-**sync**, and an assignment
    // resets the cluster cursor to 0 (task #590's rule). Writing the field and the
    // row directly keeps a cluster mid-walk in place — measured
    // `set -- -abc val`: `a` reports, the counter stays 1, the cursor moves to 2.
    let mut state = ShellState::new();
    state.optchar = 2;
    write_counter(&mut state, 1).unwrap();
    assert_eq!(state.optchar, 2, "the cursor survived the counter write");
    assert_eq!(state.optind, 1);
    assert_eq!(read(&state, b"OPTIND"), Some(b"1".to_vec()));
}

#[test]
fn one_byte_refuses_a_nul_and_keeps_everything_else() {
    assert_eq!(one_byte(b'a').unwrap(), s(b"a"));
    assert!(one_byte(0).is_none(), "a NUL is not a value");
}
