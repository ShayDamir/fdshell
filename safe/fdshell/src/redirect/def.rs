mod ctors;

use super::{RedirectDirection, RedirectSource};

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct RedirectDef {
    pub export_to: i32,
    /// Second target fd sharing this def's single open: the `&>` family's fd 2.
    /// It is a *target*, so it counts in `max_target`, and it is fed by a
    /// second clone of the same source fd, never by a `2>&1`-style dup.
    pub extra_to: Option<i32>,
    pub direction: RedirectDirection,
    pub source: RedirectSource,
}

impl RedirectDef {
    /// The highest fd this command redirects to. Redirect sources must be
    /// cloned/duplicated above it, otherwise a `dup2` into a target fd
    /// turns the `dup2` into a no-op whose source drop then closes the target.
    pub fn max_target(redirects: &[RedirectDef]) -> i32 {
        redirects
            .iter()
            .flat_map(|r| [r.export_to, r.extra_to.unwrap_or(0)])
            .max()
            .unwrap_or(0)
    }

    /// `noclobber` applies to a plain write (`>`); `>|` is the bypass (POSIX
    /// #2.2), so only the `Write` direction is gated (redirect/open.rs).
    pub fn write_flag(&self) -> bool {
        matches!(self.direction, RedirectDirection::Write)
    }

    /// The `&>` family (`&>`, `&>>`, attached and bare): **one** operator, so
    /// **one** open, shared by fd 1 and fd 2 as `export_to` + `extra_to`
    /// (bash's `> f 2>&1`, where both streams append to each other). Two defs
    /// of the same path would be two descriptions at independent offsets.
    pub fn combined(direction: RedirectDirection, source: RedirectSource) -> Self {
        RedirectDef {
            export_to: 1,
            extra_to: Some(2),
            direction,
            source,
        }
    }
}
