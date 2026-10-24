//! Likely secrets: never show their content (FR-77).
//!
//! A node is a secret by name (`.env*`, `*.pem`, `*.key`, `id_rsa*`,
//! `id_ed25519*`, `credentials*`, `.netrc`, `.pgpass`) or by a private-key
//! header in its head.

/// True for a name or a head that looks like a secret.
pub fn is_secret(name: &[u8], head: Option<&[u8]>) -> bool {
    by_name(name) || head.is_some_and(has_private_key)
}

fn by_name(name: &[u8]) -> bool {
    // Public keys are not secrets.
    if name.ends_with(b".pub") {
        return false;
    }
    let starts = |p: &[u8]| name.starts_with(p);
    let ends = |s: &[u8]| name.len() > s.len() && name.ends_with(s);
    starts(b".env")
        || ends(b".pem")
        || ends(b".key")
        || starts(b"id_rsa")
        || starts(b"id_ed25519")
        || starts(b"id_ecdsa")
        || starts(b"id_dsa")
        || starts(b"credentials")
        || name == b".netrc"
        || name == b".pgpass"
}

/// `-----BEGIN ... PRIVATE KEY-----` anywhere in the head.
pub fn has_private_key(head: &[u8]) -> bool {
    const BEGIN: &[u8] = b"-----BEGIN ";
    let mut rest = head;
    while let Some(i) = find(rest, BEGIN) {
        let after = &rest[i + BEGIN.len()..];
        let line_end = after
            .iter()
            .position(|&b| b == b'\n')
            .unwrap_or(after.len());
        if find(&after[..line_end], b"PRIVATE KEY-----").is_some() {
            return true;
        }
        rest = after;
    }
    false
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
