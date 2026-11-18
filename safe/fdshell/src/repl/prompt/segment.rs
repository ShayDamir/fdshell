//! The recorded shape of a decoded prompt, one run per kind. The data
//! contract epic #198's display-width math consumes is stated in the module
//! doc of `decode.rs`, the decoder that produces these runs.

use alloc::vec::Vec;

/// One run of the decoded prompt: `bytes` classified by `kind`. Bytes, not
/// `ShortCStr`: a decoded prompt can hold a NUL (`\400`, measured), and the
/// record is a transient draw-path value, not a stored structure — STYLE
/// §6.3's storage rule is not bent by it.
#[derive(Debug, PartialEq)]
pub(crate) struct Segment {
    pub(crate) kind: SegmentKind,
    pub(crate) bytes: Vec<u8>,
}

/// What a run is, for the width math (epic #198).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SegmentKind {
    /// Bytes that count toward the display width: literal text, decoded
    /// mechanical/octal/hex bytes, and the bytes of non-toggling markers.
    Visible,
    /// The bytes between a matched `\[`…`\]` pair: drawn, but excluded from
    /// the width sum. Runs split exactly at the toggling markers, so the
    /// record alone answers which bytes are visible.
    NonPrinting,
    /// A value-escape placeholder: `bytes` holds the raw escape text and the
    /// provider substitutes the real value at render time.
    Value(ValueEscape),
}

/// The bash PROMPTING table's value escapes, recognised by the decoder as
/// placeholders; their values are wired by epic #197's sibling subtasks, and
/// each variant names its owner. `\D{…}` stays unrecognised (that is #618's).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ValueEscape {
    /// The user name (#617).
    Username,
    /// The hostname up to the first dot (#617).
    Hostname,
    /// The full hostname (#617).
    FullHostname,
    /// The tty name (#617).
    TtyName,
    /// `$`, or `#` for the superuser (#617).
    Dollars,
    /// The shell name (#617).
    ShellName,
    /// The shell version (#617).
    Version,
    /// The full shell release (#617).
    PlusVersion,
    /// The number of jobs (#617).
    Jobs,
    /// The history number (#617).
    History,
    /// The command number, field-aligned (#617).
    AlignedNumber,
    /// The date (#618).
    Date,
    /// The time, `HH:MM:SS` (#618).
    Time,
    /// The time, `HH:MM:SS` in the 24-hour spelling (#618).
    Time24,
    /// The time, `HH:MM AM/PM` (#618).
    AmPmTime,
    /// The time, `HH:MM` (#618).
    MinuteTime,
    /// The working directory (#619).
    WorkingDir,
    /// The working directory's basename (#619).
    ShortWorkingDir,
}
