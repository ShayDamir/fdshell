# Lessons Learned

## Recursive glob_match caused stack overflow on long inputs
`match_glob` used two recursive calls — star backtracking loop + literal-character match path. 10K literal chars = 10K frames. Converted to iterative star-backtracking (DFA-style with `star_pi`/`star_si` pointers). When a function recurses linearly on input length, prefer iterative with explicit backtrack state.

## `input.get(pos)` is ambiguous when `pos` comes from a trait object
`pos` from a trait method returning `usize` makes `input.get(pos)` ambiguous (index vs slice). Fix: extract `pos` to local, use `input.get(pos..)` or `input.get(..pos)`.

## `Iterator::rposition` returns index from start, not from end
`rposition` consumes from the right but returns index from the **start**. Verify with a small test.

## `cargo fmt` reformatting can change test formatting
Run `cargo fmt` before `nix flake check`.

## Always show the caret, even at position 0
Remove the `!(is_first_line && caret_col == 0)` guard. Fish shows `^^` at position 0; users benefit from the marker.

## Use `Iterator::position` with `skip` instead of `find` + tuple destructuring
`.skip(N).position(|pred)? + N` is cleaner than `.skip(N).find(|(_, t)| pred).ok_or(...)?.0`.

## Don't store redundant length constants in const arrays
Don't pair byte strings with `.len()` values — `kw.len()` is always available. Avoids maintenance burden.

## `Result::unwrap_err()` requires `Debug` on the `Ok` type
Use `match result { Ok(_) => panic!(), Err(e) => e }` when the Ok type lacks `Debug`.

## Never add `#[allow(clippy::...)]` in production code
Use `.get()` + `.ok_or()`, `?`, or propagate errors. Allow attributes are only for test files.

## `error-stack` 0.7 uses `current_context()`, not `as_context()`
Use `.current_context()` to extract the context from `Report<T>`.

## `From` impls that discard the source error do not preserve the error chain
Only `.change_context()` preserves the source in the chain. `From` is a blind conversion; `map_err` also discards.

## `Report<T>` requires `T: Error + Send + Sync + 'static`
Ensure error enums for `Report` implement `core::error::Error` (which requires `Debug`).

## `displaydoc` on enums — use doc comments only, never `#[displaydoc("...")]`
Doc comments on variants are the format string. Don't add `#[displaydoc("...")]` attributes.

## `error_stack::ResultExt` is required for `.change_context()` on `Result`
`.change_context()` comes from `ResultExt`. Use `use error_stack::ResultExt` before calling it.

## "or" in a Display message means the error variants are too coarse
If a variant's message lists multiple causes separated by "or", split into separate variants.

## `Display` on `ShortCStr` is correct for user-facing error messages; forbidden for data paths
Non-UTF-8 bytes use `from_utf8_lossy` — correct for errors. For data paths, use `as_bytes()`/`as_c_str()` (lossless).
`report_*` functions that know the position must attach the input line alongside `ParsePosition`.

## `.change_context()` with same-message variants causes duplicate Display output
Outer variant should describe *what* failed, inner describes *why*. Avoid "Y → because Y" chains.

## `dispatch_builtin` exit codes belong in `Ok`, not `Err`
Exit codes (0/1/errno) are shell convention, not errors. `Ok(code)`, only `Err(BuiltinError::Unknown)` on failure.

## `become` is a reserved keyword in Rust — rename modules
Rename `mod become` to `mod become_cmd` (file: `become_cmd.rs`). Only the module identifier matters.

## Prefer `Result::is_ok_and()` over `map().unwrap_or(false)`
`result.is_ok_and(|v| pred(v))` vs `result.map(|v| pred(v)).unwrap_or(false)`.

## Variants that differ only by flag name should use a parameter, not separate variants
`MissingValue(&'static str)` instead of `MissingDirfdValue` + `MissingFdValue`.

## Do not use `.map_err(|_| ... )`
This discards the underlying error. Use `.change_context()` instead. This requires changing error type to `Report<Error>`
if it was not done before.

## Do not use `unreachable!()`
If something should never happen, use `Never` variant of Error (like `bail!(Error::Never)` or `ensure!(condition, Error::Never)`).
If there is no such variant - add one, with "impossible" as rustdoc.

## Use `ensure!()` to conditionally return error without attachments where feasible
`if condition { return Err(Report::new(Error::Variant));}` -> `ensure!(!condition, Error::Variant)`. This doesn't work on let chains though.
If condition is complex and negating it would be unreadable, use `if + bail!`.`

## Use `bail!()` to return unconditional errors without attachments
`return Err(Error::Variant)` -> `bail!(Error::Variant)`
`return Err(Report::new(Error::Variant))` -> `bail!(Error::Variant)`
