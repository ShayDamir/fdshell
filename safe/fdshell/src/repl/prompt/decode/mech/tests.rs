#![allow(clippy::unwrap_used)]

use crate::repl::prompt::segment::ValueEscape;

#[test]
fn the_mechanical_table_maps_its_five_escapes() {
    assert_eq!(super::mechanical(b'a'), Some(0x07));
    assert_eq!(super::mechanical(b'e'), Some(0x1b));
    assert_eq!(super::mechanical(b'n'), Some(0x0a));
    assert_eq!(super::mechanical(b'r'), Some(0x0d));
    assert_eq!(super::mechanical(b'\\'), Some(0x5c));
    assert_eq!(super::mechanical(b'z'), None);
}

#[test]
fn octal_wants_three_digits_and_truncates_to_eight_bits() {
    assert_eq!(super::octal(br"101", 0), Some(0x41));
    assert_eq!(super::octal(br"777", 0), Some(0xff));
    assert_eq!(super::octal(br"400", 0), Some(0x00));
    assert_eq!(super::octal(br"12", 0), None);
    assert_eq!(super::octal(br"12x", 0), None);
    assert_eq!(super::octal(br"999", 0), None);
}

#[test]
fn hex_takes_one_or_two_digits() {
    assert_eq!(super::hex(br"4", 0), Some((0x04, 1)));
    assert_eq!(super::hex(br"41", 0), Some((0x41, 2)));
    assert_eq!(super::hex(br"39", 0), Some((0x39, 2)));
    assert_eq!(super::hex(br"6f", 0), Some((0x6f, 2)));
    assert_eq!(super::hex(br"66", 0), Some((0x66, 2)));
    assert_eq!(super::hex(br"4F", 0), Some((0x4F, 2)));
    assert_eq!(super::hex(br"4f", 0), Some((0x4f, 2)));
    assert_eq!(super::hex(br"z", 0), None);
}

#[test]
fn the_value_table_maps_the_bash_value_escapes() {
    // Every arm is pinned: the table is the contract the value subtasks
    // (#617–#619) plug into, so no arm may be silently deleted.
    assert_eq!(super::value_escape(b'u'), Some(ValueEscape::Username));
    assert_eq!(super::value_escape(b'h'), Some(ValueEscape::Hostname));
    assert_eq!(super::value_escape(b'H'), Some(ValueEscape::FullHostname));
    assert_eq!(super::value_escape(b'l'), Some(ValueEscape::TtyName));
    assert_eq!(super::value_escape(b'$'), Some(ValueEscape::Dollars));
    assert_eq!(super::value_escape(b's'), Some(ValueEscape::ShellName));
    assert_eq!(super::value_escape(b'v'), Some(ValueEscape::Version));
    assert_eq!(super::value_escape(b'V'), Some(ValueEscape::PlusVersion));
    assert_eq!(super::value_escape(b'j'), Some(ValueEscape::Jobs));
    assert_eq!(super::value_escape(b'!'), Some(ValueEscape::History));
    assert_eq!(super::value_escape(b'#'), Some(ValueEscape::AlignedNumber));
    assert_eq!(super::value_escape(b'd'), Some(ValueEscape::Date));
    assert_eq!(super::value_escape(b't'), Some(ValueEscape::Time));
    assert_eq!(super::value_escape(b'T'), Some(ValueEscape::Time24));
    assert_eq!(super::value_escape(b'@'), Some(ValueEscape::AmPmTime));
    assert_eq!(super::value_escape(b'A'), Some(ValueEscape::MinuteTime));
    assert_eq!(super::value_escape(b'w'), Some(ValueEscape::WorkingDir));
    assert_eq!(
        super::value_escape(b'W'),
        Some(ValueEscape::ShortWorkingDir)
    );
    assert_eq!(super::value_escape(b'D'), None);
}
