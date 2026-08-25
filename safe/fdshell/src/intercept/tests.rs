use super::*;
use crate::capture::Capture;
use crate::parse::CommandLine;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

fn make_cmdline(command: &[u8], args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(command.to_vec()).unwrap(),
        args: args_vec,
        args_fq: vec![false; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn make_line(command: &str, args: &[&str]) -> Vec<u8> {
    if args.is_empty() {
        command.into()
    } else {
        format!("{} {}", command, args.join(" ")).into_bytes()
    }
}

fn text(bytes: &[u8]) -> sys::ScriptText {
    sys::ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        sys::Position::new(1, 1),
        sys::Origin::Shell,
    )
}

#[test]
fn try_intercept_cd_returns_true() {
    let line = make_line("cd", &["/tmp"]);
    let cmdline = make_cmdline(b"cd", &["/tmp"]);
    let cell = make_cell();
    assert!(
        try_intercept(&text(&line), &cmdline, &cell)
            .unwrap()
            .is_some()
    );
}

#[test]
fn try_intercept_envfilter_returns_true() {
    let line = make_line("envfilter", &["--list"]);
    let cmdline = make_cmdline(b"envfilter", &["--list"]);
    let cell = make_cell();
    assert!(
        try_intercept(&text(&line), &cmdline, &cell)
            .unwrap()
            .is_some()
    );
}

#[test]
fn try_intercept_shift_returns_true() {
    let line = make_line("shift", &[]);
    let cmdline = make_cmdline(b"shift", &[]);
    let cell = make_cell();
    assert!(
        try_intercept(&text(&line), &cmdline, &cell)
            .unwrap()
            .is_some()
    );
}

#[test]
fn run_shift_removes_positional_args() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .positional
            .push_back(sys::ImportedStr::shell(c"zero".into()));
        state
            .positional
            .push_back(sys::ImportedStr::shell(c"one".into()));
    }
    let cmdline = make_cmdline(b"shift", &["1"]);
    assert!(shift::run_shift(b"shift", &cmdline, &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(state.positional.len(), 1);
    assert_eq!(
        state.positional.front().unwrap().as_bytes().unwrap(),
        b"one"
    );
}

#[test]
fn run_set_replaces_positional_args() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .positional
            .push_back(sys::ImportedStr::shell(c"old".into()));
    }
    let cmdline = make_cmdline(b"set", &["--", "a", "b"]);
    let line = make_line("set", &["--", "a", "b"]);
    assert!(set_cmd::run_set(&line, &cmdline, &text(&line), &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(state.positional.len(), 2);
    assert_eq!(state.positional.front().unwrap().as_bytes().unwrap(), b"a");
    assert_eq!(state.positional.back().unwrap().as_bytes().unwrap(), b"b");
}

#[test]
fn run_set_dashdash_only_clears_positional() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .positional
            .push_back(sys::ImportedStr::shell(c"old".into()));
    }
    let cmdline = make_cmdline(b"set", &["--"]);
    assert!(set_cmd::run_set(b"set --", &cmdline, &text(b"set --"), &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert!(state.positional.is_empty());
}

#[test]
fn run_set_substitutes_variable_args() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .strings
            .insert(c"X".into(), sys::ImportedStr::shell(c"val".into()));
    }
    let cmdline = make_cmdline(b"set", &["--", "$X"]);
    assert!(set_cmd::run_set(b"set -- $X", &cmdline, &text(b"set -- $X"), &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(state.positional.len(), 1);
    assert_eq!(
        state.positional.front().unwrap().as_bytes().unwrap(),
        b"val"
    );
}

#[test]
fn run_eval_executes_assignment_in_current_state() {
    let cell = make_cell();
    let cmdline = make_cmdline(b"eval", &[r"A=1"]);
    let line = make_line("eval", &[r"A=1"]);
    assert!(
        eval_cmd::run_eval(&line, &cmdline, &text(&line), &cell)
            .unwrap()
            .is_none()
    );
    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get(&sys::ShortCStr::from(c"A"))
            .unwrap()
            .value
            .as_bytes()
            .unwrap(),
        b"1"
    );
}

#[test]
fn run_eval_break_propagates_loop_control() {
    let cell = make_cell();
    let cmdline = make_cmdline(b"eval", &[r"break"]);
    let line = make_line("eval", &[r"break"]);
    let control = eval_cmd::run_eval(&line, &cmdline, &text(&line), &cell).unwrap();
    assert!(matches!(
        control,
        Some(crate::loop_control::LoopControl::Break)
    ));
}

#[test]
fn run_eval_parse_error_propagates() {
    let cell = make_cell();
    let cmdline = make_cmdline(b"eval", &[r"if"]);
    let line = make_line("eval", &[r"if"]);
    assert!(eval_cmd::run_eval(&line, &cmdline, &text(&line), &cell).is_err());
}

#[test]
fn try_intercept_eval_with_captures_returns_error() {
    let line = make_line("eval", &["%tag", "%var"]);
    let mut cmdline = make_cmdline(b"eval", &["%tag", "%var"]);
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
        cap: None,
        set_at: sys::Position::new(1, 1),
    }];
    let cell = make_cell();
    assert!(try_intercept(&text(&line), &cmdline, &cell).is_err());
}

#[test]
fn run_eval_empty_script_sets_zero_status() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_last_exit(7);
    }
    let cmdline = make_cmdline(b"eval", &[]);
    assert!(
        eval_cmd::run_eval(b"eval", &cmdline, &text(b"eval"), &cell)
            .unwrap()
            .is_none()
    );
    assert_eq!(cell.borrow().unwrap().last_status.exit_code(), 0);
}

#[test]
fn try_intercept_set_unhandled_forms_return_false() {
    for args in [&["-e"] as &[&str], &["a", "b"] as &[&str]] {
        let cmdline = make_cmdline(b"set", args);
        let line = make_line("set", args);
        let cell = make_cell();
        assert!(
            try_intercept(&text(&line), &cmdline, &cell)
                .unwrap()
                .is_none(),
            "set {args:?} should fall through to external lookup"
        );
    }
}

#[test]
fn try_intercept_bare_set_lists_variables() {
    let cmdline = make_cmdline(b"set", &[]);
    let line = make_line("set", &[]);
    let cell = make_cell();
    assert!(
        try_intercept(&text(&line), &cmdline, &cell)
            .unwrap()
            .is_some(),
        "bare set should list variables"
    );
}

#[test]
fn try_intercept_read_returns_true() {
    let line = make_line("read", &["var1"]);
    let cmdline = make_cmdline(b"read", &["-u", "0", "var1"]);
    let cell = make_cell();
    assert!(
        try_intercept(&text(&line), &cmdline, &cell)
            .unwrap()
            .is_some()
    );
}

#[test]
fn try_intercept_unknown_returns_false() {
    let line = make_line("unknown_xyzzy", &[]);
    let cmdline = make_cmdline(b"unknown_xyzzy", &[]);
    let cell = make_cell();
    assert!(
        try_intercept(&text(&line), &cmdline, &cell)
            .unwrap()
            .is_none()
    );
}

#[test]
fn try_intercept_export_fd_with_captures_returns_error() {
    let line = make_line("export_fd", &["%tag", "%var"]);
    let mut cmdline = make_cmdline(b"export_fd", &["%tag", "%var"]);
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
        cap: None,
        set_at: sys::Position::new(1, 1),
    }];
    let cell = make_cell();
    let result = try_intercept(&text(&line), &cmdline, &cell);
    assert!(
        result.is_err(),
        "export_fd with captures should return an error"
    );
}

#[test]
fn try_intercept_export_fd_with_redirects_returns_error() {
    let line = make_line("export_fd", &["%tag", "%var"]);
    let mut cmdline = make_cmdline(b"export_fd", &["%tag", "%var"]);
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(c"test".into()),
    }];
    let cell = make_cell();
    let result = try_intercept(&text(&line), &cmdline, &cell);
    assert!(
        result.is_err(),
        "export_fd with redirects should return an error"
    );
}

#[test]
fn builtin_pos_found_at_correct_position() {
    let line = make_line("builtin envfilter", &["--allow", "PATH"]);
    let mut cmdline = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline.builtin = true;
    let cell = make_cell();
    let result = try_intercept(&text(&line), &cmdline, &cell);
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(
        pos, 0,
        "builtin should be detected at position 0 (start of line)"
    );

    // Test with builtin in the middle — position should be 4
    let line2 = make_line("cmd builtin envfilter", &["--allow", "PATH"]);
    let mut cmdline2 = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline2.builtin = true;
    let result2 = try_intercept(&text(&line2), &cmdline2, &cell);
    assert!(result2.is_err());
    let e2 = result2.unwrap_err();
    let pos2: usize = e2
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(
        pos2, 4,
        "builtin should be detected at position 4 (after 'cmd ')"
    );
}

#[test]
fn capture_pos_found_at_correct_position() {
    let mut cmdline = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
        cap: None,
        set_at: sys::Position::new(1, 1),
    }];
    let cell = make_cell();
    let result = try_intercept(&text(b"envfilter --allow %>fd PATH"), &cmdline, &cell);
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos, 18, "%> should be detected at position 18");
}

#[test]
fn redirect_pos_found_at_correct_position() {
    let mut cmdline = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(c"test".into()),
    }];
    let cell = make_cell();

    let result = try_intercept(&text(b"envfilter --allow PATH < input"), &cmdline, &cell);
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos, 23, "< should be detected at position 23");

    let result2 = try_intercept(&text(b"cmd > output"), &cmdline, &cell);
    assert!(result2.is_err());
    let e2 = result2.unwrap_err();
    let pos2: usize = e2
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos2, 4, "> should be detected at position 4");
}
