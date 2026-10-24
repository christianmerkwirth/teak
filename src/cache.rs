//! The type cache (stub).

use std::path::Path;

pub struct Cache;

impl Cache {
    pub fn load(_dir: &Path, _model_version: &str) -> Cache {
        Cache
    }

    pub fn is_empty(&self) -> bool {
        true
    }
}
