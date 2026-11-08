//! Width/precision field resolution for the conversions: a `*` field consumes
//! an argument (reported through the `Sink`), a literal field is its own value.

// mutants: -C --lib -C --test -C printf -- -E 'test(/^child::printf::/) | binary(printf)'

use core::ffi::CStr;

use super::super::spec::Field;
use super::{Sink, num};

/// Resolve a width/precision field to its value and whether a negative `*`
/// width forced left-justification. A negative `*` precision means "absent".
pub(super) fn resolve_field(
    field: Option<Field>,
    rest: &mut &[&CStr],
    sink: &mut Sink,
    is_precision: bool,
) -> (Option<usize>, bool) {
    let Some(f) = field else {
        return (None, false);
    };
    match f {
        Field::Lit(n) => (Some(n), false),
        Field::Star => {
            let v = num::next_int(rest, sink);
            if is_precision {
                if v < 0 {
                    (None, false)
                } else {
                    (Some(v as usize), false)
                }
            } else if v.is_negative() {
                // A negative `*` width left-justifies at `|width|`. The
                // predicate has no `v == 0` boundary to hide behind: `v < 0`
                // mutates to `v <= 0` (unobservable at 0, where `pad` fills
                // nothing), `is_negative` mutates only to a value that the
                // two-sided `%*d` pins kill.
                (Some(v.unsigned_abs() as usize), true)
            } else {
                (Some(v as usize), false)
            }
        }
    }
}
