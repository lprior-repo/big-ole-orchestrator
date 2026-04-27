use std::fs;
use std::path::Path;

use crate::workspace_swap::types::{SwapError, SwapPhase};

pub(crate) fn sync_dir(path: &Path) -> Result<(), SwapError> {
    std::fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| SwapError::SyncFailed {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok(())
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SwapError> {
    fs::create_dir_all(dst).map_err(|e| SwapError::ShadowCreate {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in fs::read_dir(src).map_err(|e| SwapError::CopyFailed {
        from: src.to_path_buf(),
        to: dst.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| SwapError::CopyFailed {
            from: src.to_path_buf(),
            to: dst.to_path_buf(),
            source: e,
        })?;

        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry.file_type().map_err(|e| SwapError::CopyFailed {
            from: src_path.clone(),
            to: dst_path.clone(),
            source: e,
        })?;

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).map_err(|e| SwapError::CopyFailed {
                from: src_path.clone(),
                to: dst_path.clone(),
                source: e,
            })?;
        }
    }

    Ok(())
}

pub(crate) fn write_journal_at(journal: &Path, phase: SwapPhase) -> Result<(), SwapError> {
    let content = phase.as_str();
    fs::write(journal, content).map_err(|e| SwapError::JournalWrite {
        path: journal.to_path_buf(),
        source: e,
    })?;
    sync_dir(journal.parent().unwrap_or(Path::new(".")))?;
    Ok(())
}
