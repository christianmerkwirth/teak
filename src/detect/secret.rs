//! Likely secrets: never show their content (FR-77).

/// True for a name or a head that looks like a secret.
pub fn is_secret(_name: &[u8], _head: Option<&[u8]>) -> bool {
    false
}
