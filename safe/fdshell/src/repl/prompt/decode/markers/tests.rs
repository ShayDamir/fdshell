use crate::repl::prompt::decode::Action;
use crate::repl::prompt::decode::markers::Markers;

#[test]
fn the_prompt_drawer_keeps_the_marker_byte() {
    assert!(matches!(Markers::Bytes.action(0x01), Action::Byte(0x01)));
    assert!(matches!(Markers::Bytes.action(0x02), Action::Byte(0x02)));
}

#[test]
fn the_raw_drawer_contributes_nothing_for_a_marker_escape() {
    assert!(matches!(Markers::Dropped.action(0x01), Action::Skipped));
    assert!(matches!(Markers::Dropped.action(0x02), Action::Skipped));
}
