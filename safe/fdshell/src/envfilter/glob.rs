/// Simple `*` wildcard glob match.
///
/// Supports only `*` (match any sequence). No `?`, `[...]`, or escapes.
/// Iterative with backtracking — no stack overflow risk.
pub(crate) fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    let (mut pi, mut si) = (0, 0);
    let (mut star_pi, mut star_si) = (None, 0);

    while si < name.len() {
        if pattern.get(pi) == Some(&b'*') {
            star_pi = Some(pi + 1);
            star_si = si;
            pi += 1;
            while pattern.get(pi) == Some(&b'*') {
                pi += 1;
            }
            if pi == pattern.len() {
                return true;
            }
        } else if pattern.get(pi) == name.get(si) {
            pi += 1;
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
