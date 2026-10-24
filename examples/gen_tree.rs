use std::path::PathBuf;

#[derive(Default)]
pub struct Opts {
    pub files: u64,
    pub seed: u64,
    pub out: PathBuf,
    pub depth: u32,
    pub no_ext_share: f64,
}

pub struct Stats {
    pub files: u64,
}

pub fn generate(_opts: &Opts) -> std::io::Result<Stats> {
    unimplemented!()
}

#[allow(dead_code)]
fn main() {}
