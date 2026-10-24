//! Boilerplate templates (stub).

use super::ChaffClass;

pub fn normalize(_text: &[u8]) -> Vec<u8> {
    Vec::new()
}

pub fn fnv1a64(_bytes: &[u8]) -> u64 {
    0
}

pub struct Templates;

impl Templates {
    pub fn builtin() -> Templates {
        Templates
    }

    pub fn lookup(&self, _hash: u64) -> Option<(&str, ChaffClass)> {
        None
    }
}
