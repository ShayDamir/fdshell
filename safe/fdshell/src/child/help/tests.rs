#![allow(clippy::unwrap_used)]

use alloc::string::ToString;
use alloc::vec::Vec;

use super::commands::{BUILTINS, SHELL_CMDS};
use crate::child::dispatch::DISPATCH;
use crate::intercept::commands::INTERCEPTED_COMMANDS;

/// Commands the parser handles directly instead of via a dispatch table.
const PARSE_LEVEL: &[&[u8]] = &[b"umask", b"unset"];

/// Fd-passing builtins dispatched by `child::fdpass` rather than `DISPATCH`.
const FDPASS: &[&[u8]] = &[b"import_fd", b"export_fd"];

fn label(name: &[u8]) -> alloc::string::String {
    core::str::from_utf8(name).unwrap().to_string()
}

#[test]
fn every_dispatched_builtin_is_listed() {
    for (name, _) in DISPATCH {
        assert!(
            BUILTINS.iter().any(|(listed, _)| *listed == *name),
            "help Builtins is missing `{}`",
            label(name)
        );
    }
}

#[test]
fn every_intercept_command_is_listed() {
    for name in INTERCEPTED_COMMANDS {
        assert!(
            SHELL_CMDS.iter().any(|(listed, _)| *listed == *name),
            "help Shell commands is missing `{}`",
            label(name)
        );
    }
}

#[test]
fn fdpass_builtins_are_listed() {
    for (name, in_shell) in [(b"import_fd" as &[u8], false), (b"export_fd", true)] {
        let listed = if in_shell {
            SHELL_CMDS.iter().any(|(n, _)| *n == name)
        } else {
            BUILTINS.iter().any(|(n, _)| *n == name)
        };
        assert!(listed, "help is missing `{}`", label(name));
    }
}

#[test]
fn help_lists_no_phantom_commands() {
    let real: Vec<&[u8]> = DISPATCH
        .iter()
        .map(|(name, _)| *name)
        .chain(INTERCEPTED_COMMANDS.iter().copied())
        .chain(FDPASS.iter().copied())
        .chain(PARSE_LEVEL.iter().copied())
        .collect();
    for (name, _) in SHELL_CMDS.iter().chain(BUILTINS) {
        assert!(
            real.contains(name),
            "help lists `{}` but no such command exists",
            label(name)
        );
    }
}

#[test]
fn help_entries_are_well_formed() {
    let mut seen: Vec<&[u8]> = Vec::new();
    for (name, desc) in SHELL_CMDS.iter().chain(BUILTINS) {
        assert!(
            !seen.contains(name),
            "duplicate help entry `{}`",
            label(name)
        );
        seen.push(name);
        assert!(!desc.is_empty(), "`{}` has no description", label(name));
        assert_ne!(
            *desc,
            *name,
            "`{}` reuses its name as description",
            label(name)
        );
    }
}
