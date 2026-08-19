use sys::Pid;

#[test]
fn pid_display_formats_raw() {
    assert_eq!(Pid::from_raw(1234).to_string(), "1234");
    assert_eq!(Pid::from_raw(0).to_string(), "0");
    assert_eq!(Pid::from_raw(-1).to_string(), "-1");
}
