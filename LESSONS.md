# Lessons Learned

## `input.get(pos)` is ambiguous when `pos` comes from a trait object
When `pos` is returned from a trait method (e.g., `dyn ErrorPosition::source_start()` returning `usize`), `input.get(pos)` is ambiguous — the compiler can't tell if you mean indexing (`&[u8]` → `&u8`) or slicing (`&[u8]` → `Option<&[u8]>`). Fix: extract `pos` into a local `usize` variable first, then use explicit slice syntax like `input.get(pos..)` or `input.get(..pos)`.

## `Iterator::rposition` returns index from start, not from end
`Iterator::rposition` consumes the iterator from the right but returns the index from the **start** of the iterator. This is easy to miscount when calculating byte positions in multi-line strings. Always verify with a small test program before trusting manual calculations.

## `cargo fmt` reformatting can change test formatting
Run `cargo fmt` before `nix flake check` to avoid CI failures from formatting differences.

## Error position 0 is valid for errors at the start of input
When `if` is at position 0, errors like "missing 'then'" will have `source_start == 0`. Tests should not assert `source_start != 0` for all errors — only errors that occur mid-input will have non-zero positions.

## Always show the caret, even at position 0
The `format_parse_error` function previously skipped the caret when `source_start == 0` on the first line, reasoning it was "obvious". Fish shell shows the caret at position 0 (`^^`), and users benefit from the visual marker too. Remove the `!(is_first_line && caret_col == 0)` guard — always show the line and caret.

## Use `Iterator::position` with `skip` instead of `find` + tuple destructuring
To find the index of an element after skipping N items, use `.skip(N).position(|pred)? + N` instead of `.skip(N).find(|(_, t)| pred).ok_or(...)?.0`. The `position` method returns the relative index directly, and `+ N` compensates for the skip. This is cleaner than `find` which requires destructuring the `(index, item)` tuple.

## Don't store redundant length constants in const arrays
When a const array contains byte strings (e.g., `&[u8]`), don't pair them with `.len()` values — use `kw.len()` at runtime. The length is always derivable from the data, and storing it creates a maintenance burden (must update both the string and its length). `&[u8]` has `.len()` available in const context.

## `Result::unwrap_err()` requires `Debug` on the `Ok` type
`unwrap_err()` is defined as `fn unwrap_err(self) -> E where T: Debug`. When the `Ok` type (e.g., `ParsedLine`) doesn't implement `Debug`, use `match result { Ok(_) => panic!(), Err(e) => e }` instead. This applies to all test code that uses `unwrap_err()` on `Result<NonDebugType, _>`.

## Never add `#[allow(clippy::...)]` in production code
Clippy lints like `indexing_slicing`, `expect_used`, `unwrap_used` are deny-by-default in this project. When a lint fires in production code, the fix must address the underlying issue — use `.get()` with `.ok_or()` for safe slices, `?` or explicit `match` for fallible operations, or propagate the error up the call chain. `#[allow]` attributes are only permitted in test files (`tests/` modules and `#[cfg(test)]` blocks). Adding an allow in production code bypasses the safety guarantees the lint enforces and is never acceptable.

## `Report<T>` does not implement `From<i32>` — `i32` is not an `Error` type
`error-stack`'s `Report<T>` implements `From<T>` but not `From<i32>` because `i32` does not implement `std::error::Error` (or `core::error::Error`). When converting `Result<_, i32>` into `Result<_, Report<C>>`, every `?` on an `i32`-error source must use explicit `.map_err(|_| C)` or `.ok_or(C)`. The `?` operator cannot auto-convert `i32` — it requires a `From` impl from the source error type to the target error type.

## `error-stack` 0.7 uses `current_context()`, not `as_context()`
`Report<T>` exposes the context via `.current_context() -> &T`. There is no `.as_context()` method. When writing tests that inspect the error type stored in a `Report`, use `e.current_context()` to extract the context for matching.

## `From` impls that discard the source error do not preserve the error chain
A `From<Source> for Target` impl that ignores `Source` (e.g., `fn from(_: ParseErrorInfo) -> Self { Target::Variant }`) does **not** attach the original error to the `error-stack` chain. The comment claiming it does is false. Only `.change_context()` preserves the source error in the chain. `From` is a blind type conversion —  use `change_context` instead. Same for .map_err().

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

## `dispatch_builtin` exit codes belong in `Ok`, not `Err`
When converting `dispatch_builtin` from `Result<_, i32>` to `Result<i32, BuiltinError>`, exit codes (0 for success, 1 for `false`, errno values for exec failures) belong in the `Ok` position. The `i32` is not an error type here — it's the shell's exit code convention. Consumers must match on `Ok(code)` to extract the exit code, and only `Err(BuiltinError::Unknown)` maps to `NotABuiltin` errors. This satisfies "ban i32 as error type" while preserving shell exit code semantics.

## `become` is a reserved keyword in Rust — rename modules
`become` is an experimental reserved keyword (issue #112788). `mod become;` fails to compile. When a builtin command name conflicts with a Rust reserved keyword, rename the module file and module declaration (e.g., `become_cmd.rs` with `mod become_cmd;`). The file name can be anything — only the module identifier matters.
