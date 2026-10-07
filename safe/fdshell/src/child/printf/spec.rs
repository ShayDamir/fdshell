//! Format-specifier types and the conversion-character table.

mod parse;

pub(super) use parse::parse;

/// Upper bound applied to width and precision field values.
pub(super) const MAX_FIELD: usize = 1 << 20;

/// A field operand: a literal value from the format, or `*` (taken from the
/// argument list at render time).
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(super) enum Field {
    /// A literal field value.
    Lit(usize),
    /// `*`: the field value is the next argument.
    Star,
}

/// A parsed `%` specifier: the flags, the (unresolved) fields, and the
/// conversion character.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct Spec {
    pub(super) left: bool,
    pub(super) plus: bool,
    pub(super) space: bool,
    pub(super) alt: bool,
    pub(super) zero: bool,
    pub(super) width: Option<Field>,
    pub(super) precision: Option<Field>,
    pub(super) conv: u8,
}

impl Spec {
    const fn initial() -> Self {
        Self {
            left: false,
            plus: false,
            space: false,
            alt: false,
            zero: false,
            width: None,
            precision: None,
            conv: 0,
        }
    }
}

/// A specifier with its fields resolved to concrete values, ready to render.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct Fmt {
    pub(super) conv: u8,
    pub(super) left: bool,
    pub(super) plus: bool,
    pub(super) space: bool,
    pub(super) alt: bool,
    pub(super) zero: bool,
    pub(super) width: Option<usize>,
    pub(super) precision: Option<usize>,
}

/// The conversion characters `printf` accepts.
pub(super) fn is_conv(c: u8) -> bool {
    matches!(
        c,
        b's' | b'c'
            | b'd'
            | b'i'
            | b'u'
            | b'o'
            | b'x'
            | b'X'
            | b'b'
            | b'q'
            | b'a'
            | b'A'
            | b'e'
            | b'E'
            | b'f'
            | b'F'
            | b'g'
            | b'G'
    )
}
