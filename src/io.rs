use std::path::Path;

pub fn read_head(_path: &Path, _max: usize) -> std::io::Result<Vec<u8>> {
    unimplemented!()
}

#[cfg(feature = "magika")]
pub struct MagikaInput;

#[cfg(feature = "magika")]
impl MagikaInput {
    pub fn open(_path: &Path) -> std::io::Result<MagikaInput> {
        unimplemented!()
    }
}

#[cfg(feature = "magika")]
impl magika::SyncInput for MagikaInput {
    fn length(&self) -> magika::Result<u64> {
        unimplemented!()
    }
    fn read_at(&mut self, _buffer: &mut [u8], _offset: u64) -> magika::Result<()> {
        unimplemented!()
    }
}

#[cfg(feature = "test-hooks")]
pub mod hooks {
    pub struct StageTimer;
    pub fn stage(_name: &'static str) -> StageTimer {
        unimplemented!()
    }
    pub fn write_report() {
        unimplemented!()
    }
}
