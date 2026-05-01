use memmap2::Mmap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use super::MmapCacheError;

#[derive(Debug, Clone)]
pub(super) struct CacheRegion {
    pub(super) offset: u64,
    pub(super) size: u64,
    pub(super) file_path: PathBuf,
}

pub(super) fn region_file_path(base_path: &PathBuf, key: &str) -> PathBuf {
    let safe_name = key.replace(['/', '\\', ':'], "_");
    base_path.join(safe_name)
}

pub(super) fn allocate_region(
    key: &str,
    base_path: &PathBuf,
    size: usize,
) -> Result<u64, MmapCacheError> {
    let path = region_file_path(base_path, key);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(true)
        .open(&path)?;
    file.set_len(size as u64)?;
    Ok(0)
}

pub(super) fn write_data_to_region(
    key: &str,
    _offset: u64,
    base_path: &PathBuf,
    data: &[u8],
) -> Result<(), MmapCacheError> {
    let path = region_file_path(base_path, key);
    let mut file = OpenOptions::new().write(true).open(&path)?;
    file.write_all(data)?;
    file.flush()?;
    Ok(())
}

pub(super) fn read_mapped(file: &File, size: u64) -> Result<Vec<u8>, MmapCacheError> {
    let metadata = file.metadata()?;
    if metadata.len() != size {
        return Err(MmapCacheError::InvalidRegion);
    }
    let mmap = unsafe { Mmap::map(file) }.map_err(MmapCacheError::MmapError)?;
    Ok(mmap[..size as usize].to_vec())
}
