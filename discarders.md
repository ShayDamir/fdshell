# Discarders — Functions That Discard Underlying Errors

Each item should be wrapped in `Report<...>` so the error chain is preserved.

## safe/fdshell/ — `Option` that silently drops parse/runtime errors

- [x] `parse_capture` (`parse/capture.rs:4`) — changed to `Result<Option<Capture>, Report<ParseError>>`; split into `CaptureMissingPercent` (e.g. `%>var` — missing `%` before var name) and `CaptureEmptyVar` (e.g. `%>%` — no var name after `%`)
- [ ] `detect_nested` (`init.rs:11`) — returns `Option<ImportedFd>`, discards `FDSHELL_CAPTURE` env var parse errors via `.ok()()`

## safe/fdshell/ — returns raw `ShortCStrError` instead of `Report<ExportError>`

- [ ] `set_export` (`exports.rs:39`) — returns `Result<(), ShortCStrError>`, should return `Result<(), Report<ExportError>>`
- [ ] `mark_exported` (`exports.rs:61`) — returns `Result<(), ShortCStrError>`, should return `Result<(), Report<ExportError>>`

## safe/fdshell/ — `unwrap_or` silencing errors

- [ ] `list_exports` (`exports.rs:31`) — uses `.unwrap_or()` to silence NUL byte / UTF-8 errors
- [ ] `format_line_and_caret` (`debug.rs:46`) — uses `.unwrap_or()` to silence slice / UTF-8 errors

## safe/fdshell/ — `process::exit()` discards error chain

- [ ] `run_exit` (`intercept/exit.rs:30`) — calls `std::process::exit()`, discards error chain
- [ ] `run_replace` (`intercept/become_cmd.rs:28`) — calls `std::process::exit()`, discards error chain
- [ ] `launch_pipeline` child path (`pipeline/mod.rs:49`) — calls `std::process::exit()`, discards error chain

## unsafe/sys/ — `Option` that silently drops syscall errors

- [ ] `read_proc_umask` (`umask.rs:29`) — returns `Option<u32>`, discards open/read/parse errors via `.ok()()` chain
- [ ] `ShortCStr::get` (`shortcstr/get.rs:7`) — returns `Option<Self>`, discards bounds / NUL byte
- [ ] `ShortCStr::split_once_byte` (`shortcstr/get.rs:33`) — returns `Option<(Self, Self)>`, discards not found
- [ ] `ShortCStr::strip_prefix` (`shortcstr/get.rs:38`) — returns `Option<Self>`, discards mismatch

## unsafe/sys/ — `try_into_local` returned `Result<_, SyscallError>` instead of `Report<ImportedFdError>`

- [x] `ImportedFd::try_into_local` (`importedfd.rs:43`) — fixed: now returns `Report<ImportedFdError>` with `SetFlags` variant
