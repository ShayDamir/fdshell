# `fdshell` Style Guide


## 1 Formatting guide

1.1 All code must conform to `cargo fmt`.
1.2 There must be exactly one empty line between logical entities like fn, type declarations, enum blocks etc.
1.3 Code should be split into logically consistent readable blocks delimited by empty lines as paragraphs. Long walls of code are forbidden.

## 2 Code complexity control

2.1 Code must be readable and should not contain complex conditions/decisions packed into one line.
2.2 Each source file except tests must not contain more than 90 lines of code (counted by `tokei`).
2.3 Files with 80-90 lines of code must be flagged for future refactoring.
2.4 If code requires more than 4 levels of logical depth (not counting impl block), it must be flagged for future refactoring.
2.5 Code flagged for refactoring is placed into refactoring section in `TODO.md`.
2.6 Refactoring includes extracting complex logic into helper functions, extracting types/functions/impl blocks into separate files.
2.7 `tokei` metric is authoritative, `wc -l` includes comments, empty lines etc etc.
2.8 Unit tests must live in a separate file in a directory named after the module file (`foo.rs` → `foo/tests.rs`), declared at the end of the source file as `#[cfg(test)] mod tests;`. Inline `#[cfg(test)] mod tests { ... }` blocks are forbidden — they bloat the source file and defeat the §2.2 line limit.

## 3 Use directives

3.1 All external types are imported via `use` at the start of the source file. In case of name clash, use `as` to rename types.
3.2 If external type or function was not imported via `use`, it is not allowed to be used.
3.3 Each `use` directive must fit one line after `cargo fmt` is run.
3.4 Group `use` directives for the same module using {}, for example use crate::module::{Type1, Type2, Type3}. If the line is too long and `cargo fmt` splits it, split it manually into two or more lines.
3.5 `use` directives for different modules must be on different lines. Do not `use crate::{mod1::Type1, mod2::Type2}` - use separate lines for `mod1` and `mod2`.

## 4 Error handling
4.1 Error handling is done by using custom enum types. Fallible functions use `Report` from `error_stack` to wrap those custom enums.
4.2 Plain enum variants are preferred instead of variants with associated data. Possible exceptions - flag/argument name for InvalidFlag/InvalidArgument.
4.3 Encapsulating one error type into another is strictly forbidden.
4.4 Using `.map_err()` and dropping error types without propagating or handling is strictly forbidden.
4.5 Conversion of one error type to another is done via `ResultExt` trait from `error_stack` and using `Report::change_context()`.
4.6 For custom enum types, derive `Debug` and `displaydoc::Display`, then `impl core::error::Error for Error {}`.
4.7 Error variant description must be precise and actionable by user.
4.8 If possible, attach `Suggestion` via `Report::attach_opaque` to provide user with more guidance on how to fix the error. Suggestion will be printed via debug hook when the error is printed.
4.9 `unwrap`, `expect` or `indexing` and other forms of panics are completely forbidden in the production code, but preferred in tests.
4.10 If something should never happen - map it to `Never` variant, and handle as usual. No `unreachable!`.
4.11 Specifically, since indexing is not allowed, use `slice.get(i).ok_or(Error::Never)?` instead of `slice[i]`.
4.12 Prefer `Option` over `Result` if the `None` variant is not a sign of error that user can fix. Prefer `Result` over `Option` if function failure requires different actions from user to fix them. Overall, prefer `Result` over `Option` since `Result` is easier to extend in the future.
4.13 Rustdoc comments for error variants will be rendered as Display implementation, look at existing Error types as examples.
4.14 `bail!` from `error_stack` should be used to return errors instead of `return Err(Report::new(Error))`, unless you need to chain `.change_context()` or `.attach_opaque()`.
4.15 `ensure!` from `error_stack` should be used to provide conditional errors if condition is a simple, positive predicate. For complex and negated conditions use `if` + `bail!()`.
4.16 Never chain multiple `.change_context()` calls in one expression — at most one per expression. The end consumer of every error is the user, who must read the chain of frames to understand (1) when and why the error happened and (2) how it can be fixed — by modifying the script (e.g. a typo) or the environment (e.g. file permissions). A context no longer properly produced by current code is vestigial: it adds nothing to (1) or (2), so delete the variant and flatten the error instead of chaining through a dead frame.
4.17 A produced `Never` variant may only be `.change_context()`-ed to another `Never` variant — an invariant violation must never be relabeled as a domain error the user can fix. By 4.16, a `Never` produced and re-contexted within one expression adds no information: `get(..).ok_or(Error1::Never).change_context(Error2::Never)` collapses to `get(..).ok_or(Error2::Never)`.

## 5 File descriptor types

5.1 Raw file descriptors (`i32`) are only allowed in `unsafe/sys` crate. All other crates should use specific Fd types.
5.2 `LocalFd` is an owned (`close` on `Drop`) file descriptor with `CLOEXEC` flag set on.
5.3 All file descriptors that fdshell produces (except exporting to ExportFd) must be of type `LocalFd`.
5.4 `ImportedFd` is always open file descriptor without `CLOEXEC`, which was passed by the parent process. Examples of `ImportedFd` are file descriptors `0`, `1` and `2`. Those are usually passed to all forked subprocesses.
5.5 New `ImportedFd` is not produced as a result of a syscall — it is passed via arguments, environment variables or scripts. The only source of `ImportedFd` outside of `0`, `1` and `2` is a string, which is parsed to a number.
5.6 `ExportedFd` is always open file descriptor without `CLOEXEC` that `fdshell` passes to subprocesses. It is produced from `LocalFd` after `fork`, but before `exec` or builtin call. It can be rendered to string via `fmt::Display`. It is not considered a leak, the ownership is transferred to the subprocess.
5.7 `ImportedFd` can only be converted to `LocalFd`, and `LocalFd` can only be converted to `ExportedFd`.
5.8 I/O operations are only possible on `LocalFd` and `ImportedFd`.
5.9 Only `LocalFd` can be closed by dropping it.
5.10 Some syscalls in *at family (`openat`, `execveat`) require a special file descriptor type `AtFd`, which is a borrowed view into `LocalFd`.

## 6 String types

6.1 Similar to file descriptors, there are three types of strings - imported, local and exported.
6.2 Imported strings are coming from external sources, such as program arguments, environment variables and read from files. They can be `*const c_char` or `&CStr` (from `libc` calls) or `Vec<u8>`. Imported strings should be converted to local ones as soon as feasible, and all parsing should be done on local strings.
6.3 The type for local strings is `ShortCStr`. It has very cheap `clone()` and subslicing via `get()`, and it does not contain `nul` bytes inside. This is the only type that can be stored in internal structures. To produce literals of such type, use ShortCStr::from(c"literal"), or, if the `ShortCStr` type can be inferred, c"literal".into().
6.4 `ShortCStr` is the primary type that allows operations similar to `&str`. If some operation is needed, but is missing for `ShortCStr`, it must be added, covered by tests and used instead of calling `ShortCStr::as_bytes()` and operating on `&[u8]`.
6.5 `ShortCStr` supports mutations: `push_byte(byte)` and `push_checked(&[u8])` check for NUL bytes and can fail; `push<T: NoNul>(&T)` is infallible because types implementing the `NoNul` trait (e.g. `&ShortCStr`, `&CStr`, `CString`) are guaranteed NUL-free by type invariant. `concat(&[&ShortCStr])` is also infallible.
6.6 `ExportedCStr` is an owned, nul-terminated string, similar to `CString` and can be cheaply constructed from `ShortCStr::export(&self)`, `&ShortCStr`, or via `From<ShortCStr>`. It provides `ExportedCStr::as_ref(&self) -> &CStr`, and is used to pass strings to the kernel.
6.7 Conversion to utf-8 and back should never be done on imported strings. This conversion is lossy and can lead to denial-of-service or security vulnerabilities due to corrupted data.

## 7 Safe Rust

7.1 Unsafe code is only allowed in `unsafe/sys` crate. Every other crate is `forbid(unsafe_code)` for both production code and tests.
7.2 Every unsafe block needs // SAFETY: comment, stating why the unsafe block is safe to use.
7.3 Every unsafe function needs `# Safety` rustdoc section that explains which invariants must be upheld by the caller.
7.4 Owned types with `unsafe` constructors must provide  `verify(&self)` methods that can check the invariants during runtime.

## 8 Commits

8.1 One line only — no body, no trailers. The rationale for a change lives in the subject.
8.2 Capitalized present-imperative verb, no trailing period.
8.3 Use the established shapes from `git log`, e.g. `Add <name> builtin: <semantics>`, `Catch all mutants in <target>`, `Mark <item> done in TODO`, `Fix`/`Track`/`Use`/`Preserve` `<x> so <why>`, `Refactor`/`Extract`/`Split` `<x> into <y>`, `Document <x> in README`.
8.4 One logical change per commit; join the few clauses of one topic with `;`. Keep the `: <why>` detail — with no body it is the whole rationale.
