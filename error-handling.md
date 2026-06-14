# Error Handling Strategy

## Philosophy

Errors carry context as they bubble upward through crate boundaries. Each layer that can't handle an error locally adds its own semantic description, producing a structured chain when the top level formats it for display.

No `panic!` in production code (enforced by `clippy::unwrap_used` and `clippy::expect_used` deny). All fallible paths return `Result<_, _>`. The outermost layer converts the final error chain to a user-readable message.

## Error hierarchy per crate

### `unsafe/sys/` — errno remains raw `i32`

The sys crate has no internal error propagation. Every function is a leaf: it either wraps a libc call via `cvt()` or returns a hardcoded errno constant. Wrapping these in typed enums adds zero value because there's nothing to compose context with inside the crate.

**Rule:** All public functions in `unsafe/sys/src/` return `Result<_, i32>` (positive errno on error). The sys crate is treated as an opaque syscall layer — its callers map `i32` to their own error types at the boundary.

### `safe/builtins/` — parse errors only, also raw `i32`

Builtins have no internal fallible function chains. Parser helpers return `EINVAL` directly (via `.ok_or(EINVAL)` or `.map_err(|_| EINVAL)`). They never call each other through a `Result` boundary that could compose context.

**Rule:** Functions in `safe/builtins/` also return `Result<_, i32>`. No custom error enum needed here either — the `fdshell` crate is where real composition happens.

### `safe/fdshell/` — typed errors with `error-stack::Report`

This is the only crate that needs proper typed errors. Functions chain: parse → dispatch → launch → post-launch state mutation. Each step can fail, and the error message shown to the user should explain what happened at every level.

**Rules:**
- Define a small enum per sub-domain (e.g., `ParseError`, `CaptureError`, `CdError`) using `derive(error_stack::Diagnostic)` with `displaydoc` for Display impl that matches doc strings.
- Enum variants are simple nouns — no embedded data in the enum itself. Attach contextual data via `Report::attach()` at each level as the error bubbles up.
- Cross-crate boundary (`sys::i32` or `builtins::i32` → fdshell): convert using `.map_err()` that maps errno codes to the local enum variant, then optionally `.change_context()` if the location in fdshell adds meaning.
- The top-level handler (`main.rs`, REPL loop) collects into a single `Report<AppError>` (or uses per-cmd error types) and formats via `.to_string()`.

## Example pattern

```rust
// parse/error.rs — variant names only, no data
/// [UnbalancedQuote] An unmatched quote character was found in input.
#[derive(error_stack::Diagnostic)]
enum ParseError {
    /// [UnbalancedQuote] Unmatched quote at byte position {pos}.
    UnbalancedQuote { pos: usize },  // struct variant for attached data, NOT enum payload

    /// [MissingFi] Nested if blocks not terminated by fi.
    MissingFi,

    /// [InvalidChar] Unexpected character 'ch' in input.
    InvalidChar { ch: u8 },  // again, this is a struct variant — data lives in Report.attach(), not the enum
}

// In parsing code:
fn parse_input(s: &str) -> Result<TokenStream, ParseError> {
    if has_unbalanced_quote {
        return Err(ParseError::UnbalancedQuote { pos: ... });
    }
    // ...
    Ok(tokens)
}

// In dispatch code (compose context):
fn run_command(input: &str) -> Result<..., AppError> {
    let tokens = parse_input(input).map_err(AppError::from)?;
    // or with added context at this layer:
    let tokens = parse_input(input)
        .change_context(ParseFailed(format!("while parsing command")))
        .attach("Input was 42 bytes");
}
```

## Error enum design checklist

When defining a new error enum:
- [ ] Variant names are nouns describing the error state, not verbs or full sentences
- [ ] No variants carry meaningful data in their tuple/payload — use struct variants with fields only if needed by `Diagnostic`, and prefer `Report::attach()` for extra context
- [ ] `displaydoc` derives a `Display` impl from doc strings — ensure doc strings describe what the user would see (they should read naturally as error messages)
- [ ] `Debug` is also derived (debug output can be more verbose/technical)
- [ ] Implements `core::error::Error` via derive or manual impl
- [ ] `From<other_error_type>` for cross-crate conversion if applicable

## Error display format

When the top level formats an error chain, the user sees a structured message:

```
Error: Failed to parse command (while parsing statement #3)
  Caused by: Unbalanced quote at byte position 12
  Context: Input was "echo \"hello"
```

The first line is the composed context chain (outermost first). The `Caused by` line is the original error kind. Attachments follow as sub-items.

This replaces the current unhelpful `"exit code: {errno_number}"` display in `repl.rs`.

## Boundary conversion rules

| From | To | Method |
|---|---|---|
| `sys::i32` errno | `AppError` variant (e.g., `NotFound`, `InvalidArg`) | `.map_err(|e| match e { EINVAL => ... })` at each call site, or a central `From<i32>` impl on the top-level error enum |
| `builtins::i32` errno | Same as sys — converted by fdshell layer that calls builtins | Same pattern |
| Parse error → AppError | `.change_context(AppError::ParseFailed(msg))` with original attached | Standard Report composition |
| Capture error → AppError | `.change_context(AppError::CaptureFailed(reason))` | Standard Report composition |

## Migration order (when ready)

1. Add `error-stack = "2"` + `displaydoc = "0.2"` to `safe/fdshell/Cargo.toml`
2. Define `ParseError` enum → wire through `parse.rs`, `cond.rs`, `script.rs`
3. Define `LaunchError` (wrapping sys errors) → wire through `launch.rs`, `postlaunch.rs`
4. Define domain errors (`CaptureError`, `CdError`, `ResolveError`, etc.)
5. Wire top-level `main.rs` / REPL to format the final `Report<AppError>` or equivalent
6. Update `repl.rs` error display (current `"exit code: {code}"`)
