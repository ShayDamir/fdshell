# Lessons Learned

## `input.get(pos)` is ambiguous when `pos` comes from a trait object
When `pos` is returned from a trait method (e.g., `dyn ErrorPosition::source_start()` returning `usize`), `input.get(pos)` is ambiguous — the compiler can't tell if you mean indexing (`&[u8]` → `&u8`) or slicing (`&[u8]` → `Option<&[u8]>`). Fix: extract `pos` into a local `usize` variable first, then use explicit slice syntax like `input.get(pos..)` or `input.get(..pos)`.

## `ParseError::Reason` uses `&'static str`, not `String`
The `Reason` variant stores a `&'static str` for the error message. All call sites must use bare string literals (no `.to_string()`). For dynamic keywords, use a `match` on known values instead of `format!`. The `From<ParseError> for ParseErrorInfo` conversion discards the `reason` field entirely, so heap allocation would be wasted.

## `Iterator::rposition` returns index from start, not from end
`Iterator::rposition` consumes the iterator from the right but returns the index from the **start** of the iterator. This is easy to miscount when calculating byte positions in multi-line strings. Always verify with a small test program before trusting manual calculations.

## `edit` tool can massively bloat files
When using the `edit` tool with `oldString` that matches only the first occurrence, repeated edits append rather than replace. If a file starts growing unexpectedly, check for duplicate content and rewrite the entire file with `write` instead.

## `From<i32>` trait needed alongside `to_parse_err` function
The `?` operator requires `From<E>` trait implementations for automatic error conversion. When using `.map_err(fn)?` pattern, the function alone suffices. But when `?` is used directly on `Result<_, i32>`, both the function AND the `From<i32>` impl are needed.

## Test expectations must match actual byte positions
When writing tests for position-sensitive output (like caret positioning), manually counting byte positions in string literals is error-prone. Use a small debug program to verify byte positions before writing test assertions.

## `cargo fmt` reformatting can change test formatting
Multi-line `assert_eq!` macros may be reformatted to single-line by `cargo fmt`. Run `cargo fmt` before `nix flake check` to avoid CI failures from formatting differences.

## Token position tracking requires consistent indexing
When adding byte position tracking to a tokenizer, `token_start` should represent the position **after** the last delimiter (or 0 for the first token). When pushing a token, use `token_start` as the position, then set `token_start = pos` (current position after delimiter). Delimiter tokens (`,`, `|`, `;`) use `pos - 1` as their position.

## `semi.rs` functions must be updated when token types change
When changing `tokenize()` to return `Vec<(ShortCStr, usize)>`, all consumers of `semi.rs` functions (`find_preceded_by_semi`, `trim_semi`, `try_join`) must be updated to accept the new tuple type. This includes `if_block.rs`, `for_block.rs`, and `while_block.rs`.

## `From<i32>` for `ParseErrorInfo` causes ambiguous type errors
When both `From<i32>` is implemented for `ParseErrorInfo` AND `Into<_>` is used via `.map_err(|e| e.into())`, the compiler cannot infer the target type. Use explicit `ParseErrorInfo::from(e)` instead of `e.into()` to avoid this ambiguity.

## `assert_eq!` with `ShortCStr` compares struct variants, not bytes
`ShortCStr` has both a derive `PartialEq` (in tests) and a custom `PartialEq` impl (in production). When using `assert_eq!` on `ShortCStr` values, the derive may compare struct variants (`Inline` vs `Static`) rather than bytes. Use `token.as_bytes().unwrap()` for byte-level comparison in tests.

## Error position 0 is valid for errors at the start of input
When `if` is at position 0, errors like "missing 'then'" will have `source_start == 0`. Tests should not assert `source_start != 0` for all errors — only errors that occur mid-input will have non-zero positions.

## Always show the caret, even at position 0
The `format_parse_error` function previously skipped the caret when `source_start == 0` on the first line, reasoning it was "obvious". Fish shell shows the caret at position 0 (`^^`), and users benefit from the visual marker too. Remove the `!(is_first_line && caret_col == 0)` guard — always show the line and caret.

## Show keyword-length carets to match fish shell
Fish shows carets covering the full keyword with a tilde fill pattern: `^^` for 2-char keywords (`if`, `fi`), `^~~~^` for 5-char keywords (`while`, `until`), `^~~^` for 4-char keywords (`then`, `else`, `elif`, `done`). The pattern is: `^` + `~` repeated (len-2) times + `^`. For length 2, it's just `^^`. For non-keyword errors, show a single `^`. Check word boundaries by verifying the character after the keyword is whitespace or end-of-line. Keywords to detect: `if`, `fi`, `then`, `else`, `elif`, `for`, `while`, `until`, `done`.

## Use `Iterator::position` with `skip` instead of `find` + tuple destructuring
To find the index of an element after skipping N items, use `.skip(N).position(|pred)? + N` instead of `.skip(N).find(|(_, t)| pred).ok_or(...)?.0`. The `position` method returns the relative index directly, and `+ N` compensates for the skip. This is cleaner than `find` which requires destructuring the `(index, item)` tuple.

## Don't store redundant length constants in const arrays
When a const array contains byte strings (e.g., `&[u8]`), don't pair them with `.len()` values — use `kw.len()` at runtime. The length is always derivable from the data, and storing it creates a maintenance burden (must update both the string and its length). `&[u8]` has `.len()` available in const context.

## Position tracking is contagious across all consumers
When changing `tokenize()` to return position-tagged tuples, every consumer must be updated — not just the one listed in the plan. `for_block.rs`, `while_block.rs`, and `semi.rs` all needed changes even though the plan only mentioned `if_block.rs`. Scope estimates should account for transitive type changes.

## Incremental test-driven reveals hidden scope
The plan treated caret formatting as a simple "show message if present" change. But testing exposed that the caret at position 0 was hidden, and single `^` carets didn't match fish's `^~~~^` pattern. Each discovered mismatch drove incremental expansion. When implementing format-sensitive output, test against the reference (fish) early to catch scope creep before writing code.

## `Result::unwrap_err()` requires `Debug` on the `Ok` type
`unwrap_err()` is defined as `fn unwrap_err(self) -> E where T: Debug`. When the `Ok` type (e.g., `ParsedLine`) doesn't implement `Debug`, use `match result { Ok(_) => panic!(), Err(e) => e }` instead. This applies to all test code that uses `unwrap_err()` on `Result<NonDebugType, _>`.

## Dead code checks in error paths can mask actual behavior
The original `while_block.rs` had `if do_idx < 2 { return Err(EINVAL); }` which never triggers because `find_preceded_by_semi(tokens, 1, b"do")` requires a preceding `;` at index 0 — but index 0 is the keyword (`while`/`until`), not `;`. So `do_idx` can never be 1 for a valid match. This was dead code that never caught empty conditions. When adding error messages, verify which original error paths are actually reachable vs. dead code.

## Never add `#[allow(clippy::...)]` in production code
Clippy lints like `indexing_slicing`, `expect_used`, `unwrap_used` are deny-by-default in this project. When a lint fires in production code, the fix must address the underlying issue — use `.get()` with `.ok_or()` for safe slices, `?` or explicit `match` for fallible operations, or propagate the error up the call chain. `#[allow]` attributes are only permitted in test files (`tests/` modules and `#[cfg(test)]` blocks). Adding an allow in production code bypasses the safety guarantees the lint enforces and is never acceptable.

## `Report<T>` does not implement `From<i32>` — `i32` is not an `Error` type
`error-stack`'s `Report<T>` implements `From<T>` but not `From<i32>` because `i32` does not implement `std::error::Error` (or `core::error::Error`). When converting `Result<_, i32>` into `Result<_, Report<C>>`, every `?` on an `i32`-error source must use explicit `.map_err(|_| C)` or `.ok_or(C)`. The `?` operator cannot auto-convert `i32` — it requires a `From` impl from the source error type to the target error type.

## `error-stack` 0.7 uses `current_context()`, not `as_context()`
`Report<T>` exposes the context via `.current_context() -> &T`. There is no `.as_context()` method. When writing tests that inspect the error type stored in a `Report`, use `e.current_context()` to extract the context for matching.

## `From` impls that discard the source error do not preserve the error chain
A `From<Source> for Target` impl that ignores `Source` (e.g., `fn from(_: ParseErrorInfo) -> Self { Target::Variant }`) does **not** attach the original error to the `error-stack` chain. The comment claiming it does is false. Only `.change_context()` preserves the source error in the chain. `From` is a blind type conversion — use it only when the source error is genuinely irrelevant, or use `change_context` when you want the error chain.

## `Report<T>` requires `T: Error + Send + Sync + 'static`
`error_stack::Report<T>` requires the context type to implement `Error + Send + Sync + 'static`. When defining error enums for use with `Report`, ensure they implement `core::error::Error` (which requires `Debug`). The `Send + Sync` bounds are automatically satisfied by simple enums with no non-send fields.

## `displaydoc` on enums — use doc comments only, never `#[displaydoc("...")]`
`displaydoc` derives `Display` on enums by using the doc comment on each variant as the format string. Do NOT add `#[displaydoc("...")]` attributes on enum variants — they are redundant and incorrect. The `ParseError` enum shows the correct pattern (doc comments only). Neither enums nor structs need `#[displaydoc("...")]` attributes with the derive approach.

## `error_stack::ResultExt` is required for `.change_context()` on `Result`
The `.change_context()` method comes from the `ResultExt` trait in `error_stack`, not from `std`. When converting a `Result<T, E>` into `Result<T, Report<C>>`, you need `use error_stack::ResultExt` and must call `.change_context(C)` — the `?` operator alone cannot bridge `E` → `Report<C>` unless `E` has a `From` impl.

## "or" in a Display message means the error variants are too coarse
If an error variant's `Display` message lists multiple possible causes separated by "or", the enum itself should be split so each variant represents exactly one cause. The caller should be able to match on the variant, not parse a string, to determine what went wrong.

## `Display` on `ShortCStr` is correct for user-facing error messages; forbidden for data paths
`ShortCStr` holds arbitrary bytes (no UTF-8 invariant). `Display` requires writing `&str` through `fmt::Formatter`, so non-UTF-8 bytes must use `from_utf8_lossy`. This is **correct and expected** for user-facing error messages (every shell/Unix tool uses `?` replacement for invalid bytes in error output). For **data paths** (filenames, env vars, `execveat` arguments), use `as_bytes()`/`as_c_str()` — lossless, no `Display`.
- If `.attach()` (requires `Display + Debug`) doesn't show useful info, don't switch to `.attach_opaque()` — that just hides the data. Instead ensure the attached type has a proper `Display` for user-facing contexts.
- `Debug` on `ShortCStr` stays faithful (quoted string for valid UTF-8, byte array for invalid) — never uses lossy conversion.
When `report_*` functions attach `ParsePosition { pos, input: None }` and a wrapper later calls `.attach_opaque(ParsePosition { pos: 0, input: Some(line) })`, the debug hook uses the first `ParsePosition` that has non-empty input — which is position 0. The caret points to the wrong byte. Fix: the report function that knows the position must also attach the input line. `report_unbalanced_quote` and `report_unexpected_eof` take `line: &[u8]` and attach `ParsePosition { pos, input: Some(line.to_vec()) }` together.

## `.change_context()` with same-message variants causes duplicate Display output
When wrapping one error type in another with `.change_context()`, avoid variant pairs where both have the same Display message. The `error_stack` `{:?}` output shows both levels, so if both `ChildError::NotFound` and `ExecError::NotFound` say `"command not found"`, the user sees it twice. The outer variant should describe *what* failed (e.g., `"failed to resolve command path"`), while the inner source describes *why* (e.g., `"command not found"`). This mirrors the natural language pattern: the chain reads as "X failed → because Y" rather than "Y → because Y".

## Removing `From<Error> for i32` bridges requires updating all `?` consumers
When removing `From<SyscallError> for i32` (and similar bridges for `ShortCStrError`, `ForkCellError`), every function that uses `?` on those error types and returns `Result<_, i32>` must be updated. Test files are the most common consumers — they often use `Result<(), i32>` with `?` for convenience. Fix: change the return type to `Result<(), SyscallError>` (or the appropriate typed error) and add the import. For functions that return raw errno from `libc::__errno_location()`, wrap in `SyscallError::Other()`. For `WaitStatus` match arms that return exit codes, use `SyscallError::Other(n)` to preserve the numeric value.

## `dispatch_builtin` exit codes belong in `Ok`, not `Err`
When converting `dispatch_builtin` from `Result<_, i32>` to `Result<i32, BuiltinError>`, exit codes (0 for success, 1 for `false`, errno values for exec failures) belong in the `Ok` position. The `i32` is not an error type here — it's the shell's exit code convention. Consumers must match on `Ok(code)` to extract the exit code, and only `Err(BuiltinError::Unknown)` maps to `NotABuiltin` errors. This satisfies "ban i32 as error type" while preserving shell exit code semantics.

## `fdpass::dispatch` must return `Option<Result<i32, Report<...>>>` for consistency
When `dispatch_builtin` returns `Result<i32, BuiltinError>`, the `fdpass::dispatch` function (called from the fallthrough case) must also return `Result<i32, Report<FdPassError>>` so the match arms are type-compatible. The `import_fd` and `export_fd` helper functions must return `Result<i32, Report<FdPassError>>` and return `Ok(0)` on success instead of `Ok(())`.

## Test files comparing errors with `i32` need derived PartialEq
When builtins return `BuiltinError` instead of `i32`, test files that use `assert_eq!(e, code)` must be updated. Derive `PartialEq` on the error enum — it's safe when all field types already implement it (no contagion cost). `#[cfg_attr(test, derive(PartialEq))]` does NOT work for integration tests because they're separate compilation units that don't see the `test` cfg. Just derive it unconditionally. The `assert_err` helper should take `BuiltinError` as the expected value and use `assert_eq!` directly.
