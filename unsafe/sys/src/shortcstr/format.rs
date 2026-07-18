/// Format arguments into a [`ShortCStr`], returning `Result<ShortCStr, Report<ShortCStrError>>`.
///
/// Like `alloc::format!()` but stores the result in a `ShortCStr` instead of a `String`.
/// NUL bytes in formatted output produce an error; overflow falls back to Arc allocation.
#[macro_export]
macro_rules! format {
    ($($arg:tt)*) => {{
        use $crate::shortcstr::ShortCStr;
        use $crate::shortcstr::ShortCStrError;
        use error_stack::{Report, ResultExt};

        let mut buf = ShortCStr::new();
        core::fmt::write(&mut buf, format_args!($($arg)*)).map(|_| buf).change_context(ShortCStrError::NulByte)
    }};
}

/// Append formatted arguments to an existing [`ShortCStr`].
///
/// Like `core::fmt::write!()` but returns `Result<(), Report<ShortCStrError>>`.
#[macro_export]
macro_rules! write {
    ($buf:expr, $($arg:tt)*) => {{
        use $crate::shortcstr::ShortCStrError;
        use error_stack::{Report, ResultExt};

        core::fmt::write(&mut $buf, format_args!($($arg)*)).change_context(ShortCStrError::NulByte)
    }};
}
