# Discarders — Functions That Discard Underlying Errors

Each item should be wrapped in `Report<...>` so the error chain is preserved.

## safe/fdshell/ — `Option` that silently drops parse/runtime errors

- [x] `parse_capture` (`parse/capture.rs:4`) — changed to `Result<Option<Capture>, Report<ParseError>>`; split into `CaptureMissingPercent` (e.g. `%>var` — missing `%` before var name) and `CaptureEmptyVar` (e.g. `%>%` — no var name after `%`)
- [x] `detect_nested` (`init.rs:11`) — changed to log `InvalidUtf8` via `.inspect_err()`, `NotPresent` stays silent

## safe/fdshell/ — returns raw `ShortCStrError` instead of `Report<ExportError>`

- [ ] `set_export` (`exports.rs:39`) — returns `Result<(), ShortCStrError>`, should return `Result<(), Report<ExportError>>`
- [ ] `mark_exported` (`exports.rs:61`) — returns `Result<(), ShortCStrError>`, should return `Result<(), Report<ExportError>>`

## safe/fdshell/ — `unwrap_or` silencing errors

- [x] `list_exports` (`exports.rs:31`) — changed to `Result<(), Report<ExportError>>`; writes raw bytes to stdout instead of `from_utf8`; `as_bytes()` failure uses `.change_context(ExportError::Never)`
- [ ] `format_line_and_caret` (`debug.rs:46`) — uses `.unwrap_or()` to silence slice / UTF-8 errors

## safe/fdshell/ — `process::exit()` discards error chain

- [ ] `run_exit` (`intercept/exit.rs:30`) — calls `std::process::exit()`, discards error chain
- [ ] `run_replace` (`intercept/become_cmd.rs:28`) — calls `std::process::exit()`, discards error chain
- [ ] `launch_pipeline` child path (`pipeline/mod.rs:49`) — calls `std::process::exit()`, discards error chain

## unsafe/sys/ — `Option` that silently drops syscall errors

- [x] `read_proc_umask` (`umask.rs:29`) — changed to `Result<u32, UmaskError>` with variants: `ProcOpen`, `ProcRead`, `UmaskNotFound`, `InvalidUmask`
- [x] `ShortCStr::get` (`shortcstr/get.rs:7`) — `Option` is correct; callers treat `None` as "not found", not an error. Added `split_once(&[u8])` using `windows()`.
- [x] `ShortCStr::split_once_byte` (`shortcstr/get.rs:33`) — delegates to `split_once`; same reasoning.
- [x] `ShortCStr::strip_prefix` (`shortcstr/get.rs:38`) — same reasoning.

## unsafe/sys/ — `try_into_local` returned `Result<_, SyscallError>` instead of `Report<ImportedFdError>`

- [x] `ImportedFd::try_into_local` (`importedfd.rs:43`) — fixed: now returns `Report<ImportedFdError>` with `SetFlags` variant
