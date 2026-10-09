/// Simple `*` wildcard glob match.
///
/// Supports `*` (match any sequence) and the literal escape pair `\X` (the
/// backslash is dropped and `X` matches itself; a trailing `\` matches a
/// literal backslash). No `?` or `[...]` (task #165 owns those).
/// Iterative with backtracking — no stack overflow risk.
pub(crate) fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    let (mut pi, mut si) = (0, 0);
    let (mut star_pi, mut star_si) = (None, 0);

    while si < name.len() {
        if pattern.get(pi) == Some(&b'*') {
            star_pi = Some(pi);
            star_si = si;
            while pattern.get(pi) == Some(&b'*') {
                pi += 1;
            }
            if pi == pattern.len() {
                return true;
            }
        } else if literal(pattern, pi) == name.get(si).copied() {
            pi += step(pattern, pi);
            si += 1;
        } else if let Some(sp) = star_pi {
            si = star_si + 1;
            star_si = si;
            pi = sp;
        } else {
            return false;
        }
    }

    while pattern.get(pi) == Some(&b'*') {
        pi += 1;
    }
    pi == pattern.len()
}

/// The byte pattern position `pi` matches: an escape pair `\X` matches the
/// literal `X`, a trailing `\` matches a literal backslash, any other byte
/// matches itself (`None` past the pattern end).
fn literal(pattern: &[u8], pi: usize) -> Option<u8> {
    match pattern.get(pi) {
        Some(&b'\\') => Some(pattern.get(pi + 1).copied().unwrap_or(b'\\')),
        Some(&p) => Some(p),
        None => None,
    }
}

/// The number of pattern bytes `pi` consumes: the pair `\X` is two, a single
/// byte is one.
fn step(pattern: &[u8], pi: usize) -> usize {
    usize::from(pattern.get(pi) == Some(&b'\\') && pattern.get(pi + 1).is_some()) + 1
}
