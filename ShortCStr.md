# ShortCStr — design

## Goal

A compact, `no_std + alloc`, owned C-string type with three variants:

- **Inline** (≤30 content bytes + NUL, stack allocated, no heap)
- **Static** (`&'static CStr`, zero-cost borrow, with offset+length for subslicing)
- **Rc** (`Rc<CStr>`, ref-counted heap, with offset+length for subslicing)

Cheap `Clone` (byte copy for inline, pointer copy for static, `Rc::clone` for rc).
`as_bytes()` and `as_c_str()` work on all variants — `as_c_str()` is always infallible.

## Where

`unsafe/sys/src/shortcstr.rs` — needs `extern crate alloc;` in `unsafe/sys/src/lib.rs`.
The safe `fdshell` crate imports via `use sys::ShortCStr`.

## Layout

```rust
#[repr(u8)]
enum InlineSize { _0, _1, _2, ..., _30 }   // 31 of 256 u8 values used
                                            // remaining 225 values = niche discriminants
                                            // for Static and Rc

enum ShortCStr {
    /// Owned inline — 30 content + 1 NUL. NUL always at buf[len].
    Inline { len: InlineSize, buf: [u8; 31] },               // 1 + 31 = 32

    /// Borrows a &'static CStr with offset+length for subslicing.
    /// NUL at original[offset + length].
    Static(&'static CStr, offset: usize, length: usize),     // 16 + 8 + 8 = 32

    /// Owned via Rc with offset+length for subslicing.
    /// NUL at original[offset + length].
    Rc { rc: Rc<CStr>, offset: usize, length: usize },       // 16 + 8 + 8 = 32
}
```

All three variants are self-describing — no manual tag byte. On x86_64 the
`ShortCStr` is 40 bytes (1 discriminant + 7 padding + 32 variant union), and
`Option<ShortCStr>` is also 40 bytes thanks to the `InineSize` niche covering
Static, Rc, and None. The compiler handles dispatch.

## Core API

```rust
impl ShortCStr {
    /// Inline if ≤30 bytes, else Rc. Validates no interior NUL.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, i32>;

    /// Zero-cost. offset=0, length = s.len().
    pub const fn from_static(s: &'static CStr) -> Self;

    /// Always free — returns the subslice view (no NUL).
    pub fn as_bytes(&self) -> &[u8];

    /// Always free — returns a NUL-terminated view into the backing store.
    ///
    /// - Inline: &buf[pos..=len] — NUL at buf[len], valid for any pos.
    /// - Static: &original[offset..][..length+1] — NUL at original[offset+length].
    /// - Rc:     same as Static on the backing Rc<CStr>.
    pub fn as_c_str(&self) -> &CStr;

    /// Allocate a fresh CString copy (any offset, any length).
    pub fn to_c_string(&self) -> CString;

    /// Return a new ShortCStr for a sub-range.
    ///
    /// Cost matrix:
    ///   - result ≤30 bytes       → Inline (copy + NUL, stack only)
    ///   - tail slice (pos..)      → same variant, adjust offset
    ///   - non-tail, >30 bytes    → new Rc<CStr> alloc (one alloc)
    pub fn subslice(&self, range: Range<usize>) -> Self;
}

impl Hash for ShortCStr { ... }
impl Eq for ShortCStr { ... }
impl PartialEq for ShortCStr { ... }
impl Debug for ShortCStr { ... }
impl Clone for ShortCStr {
    fn clone(&self) -> Self {
        match self {
            Inline { .. } => *self,            // byte copy
            Static(s, off, len) => Static(*s, *off, *len),  // pointer copy
            Rc { rc, offset, length } => Rc {                // Rc::clone
                rc: Rc::clone(rc),
                offset: *offset,
                length: *length,
            },
        }
    }
}

// Drop is automatic — Rc variant drops its Rc, others are trivially drop.
```

## `as_c_str()` details

```rust
impl ShortCStr {
    pub fn as_c_str(&self) -> &CStr {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = *len as usize;
                // SAFETY: buf[n] = '\0' by construction in from_bytes / subslice
                unsafe { CStr::from_bytes_with_nul_unchecked(&buf[..=n]) }
            }
            ShortCStr::Static(s, offset, length) => {
                let full = s.to_bytes_with_nul();
                // SAFETY: offset+length ≤ full_len-1, NUL at full[offset+length]
                unsafe { CStr::from_bytes_with_nul_unchecked(&full[*offset..offset + length + 1]) }
            }
            ShortCStr::Arc { arc, offset, length } => {
                let full = arc.to_bytes_with_nul();
                // SAFETY: same as Static
                unsafe { CStr::from_bytes_with_nul_unchecked(&full[*offset..offset + length + 1]) }
            }
        }
    }
}
```

## Parser integration

### Tokenizer

Tokens are `ShortCStr` from the start:

```rust
// tokenize returns Vec<ShortCStr>
// each token built via ShortCStr::from_bytes(token_bytes)
pub fn tokenize(line: &str) -> Result<Vec<ShortCStr>, i32>;
```

### Classification / detection

Work with `&[u8]` via `.as_bytes()`:

```rust
fn detect(tokens: &[ShortCStr]) -> Result<Option<ParsedLine>, i32> {
    let first = match tokens.first() {
        Some(t) => t.as_bytes(),
        None => return Ok(None),
    };
    // rest unchanged — pattern matches on &[u8] slices
}

fn parse_capture(bytes: &[u8]) -> Option<Capture>;
fn parse_redirect(bytes: &[u8]) -> Option<Redirect>;
```

### Types use `ShortCStr` directly

```rust
pub struct CommandLine {
    pub builtin: bool,
    pub command: ShortCStr,
    pub args: Vec<ShortCStr>,
    pub captures: Vec<Capture>,
    pub redirects: Vec<Redirect>,
    pub background: bool,
}

pub struct Capture {
    pub var: ShortCStr,
    pub tag: Option<ShortCStr>,
    pub force: bool,
}

pub struct Redirect {
    pub target_fd: i32,
    pub src_var: ShortCStr,
}

pub enum ParsedLine {
    Cmd(CommandLine),
    Assign { var: ShortCStr, value: ShortCStr },
    Unset(ShortCStr),
}
```

### `FdVars` keyed by `ShortCStr`

```rust
pub struct FdVars {
    map: HashMap<ShortCStr, Fd>,
}

impl FdVars {
    pub fn insert(&mut self, name: ShortCStr, fd: Fd) -> Option<Fd>;
    pub fn resolve(&self, name: &ShortCStr) -> Option<&Fd>;
    pub fn remove(&mut self, name: &ShortCStr) -> Option<Fd>;
    pub fn iter(&self) -> impl Iterator<Item = (&CStr, i32)>;
}
```

No `CString` conversion at lookup time — `ShortCStr` has `Hash + Eq`.

### No `CString` conversions in the parser

`CString::from(c"mkdirat")` → `ShortCStr::from_static(c"mkdirat")` (pointer copy).
Parsed tokens → `ShortCStr::from_bytes(bytes)` (inline or Rc).

The only places that still convert to `CString`:
- Syscall boundaries (`execveat`, `openat2`, etc.) — the callee does `.to_c_string()` if it needs an owned `CString`, or `.as_c_str()` for `&CStr`.
- `substitute_arg` — the output is `Vec<u8>` assembled from fd numbers + literal bytes, converted to `ShortCStr` at the end.

## Implementation steps

1. Add `extern crate alloc;` + `pub mod shortcstr;` to `unsafe/sys/src/lib.rs`
2. Implement `ShortCStr` in `shortcstr.rs` with the three-variant enum + `InlineSize` niche
3. Implement `from_bytes`, `from_static`, `as_bytes`, `as_c_str`, `to_c_string`, `subslice`
4. Implement `Hash`, `Eq`, `PartialEq`, `Debug`, `Clone`
5. Port `token.rs` to return `Vec<ShortCStr>`
6. Port `classify.rs`, `line.rs` to work with `&[u8]` from `.as_bytes()`
7. Port `CommandLine`, `Capture`, `Redirect`, `ParsedLine` to `ShortCStr`
8. Port `FdVars` to `HashMap<ShortCStr, Fd>`
9. Port call sites (`main.rs`, `child.rs`, `resolve.rs`, `tests.rs`)
10. Remove `CString` imports from parser module
