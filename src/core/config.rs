//! Engine configuration and performance budget parameters.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LakehouseConfig {
    pub base_dir: PathBuf,
    pub lake_dir: PathBuf,
    pub hot_buffer_dir: PathBuf,
    pub micro_batch_flush_seconds: u64,
    pub micro_batch_max_rows: usize,
    pub row_group_size: usize,
}

impl Default for LakehouseConfig {
    fn default() -> Self {
        let base = PathBuf::from("data");
        Self {
            lake_dir: base.join("lake"),
            hot_buffer_dir: base.join("hot_buffer"),
            base_dir: base,
            micro_batch_flush_seconds: 5,
            micro_batch_max_rows: 10_000,
            row_group_size: 32_768,
        }
    }
}

impl LakehouseConfig {
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.lake_dir)?;
        std::fs::create_dir_all(&self.hot_buffer_dir)?;
        Ok(())
    }
}
