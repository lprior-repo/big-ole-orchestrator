use std::fs;
use std::path::{Path, PathBuf};

use vo_cli::commands::init::{CONFIG_FILE_NAME, VO_DIR_NAME};
use vo_cli::commands::lock::LOCK_FILE_NAME;

pub fn setup_project(dir: &Path) {
    let vo_dir = dir.join(".vo");
    fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    fs::create_dir_all(vo_dir.join("storage")).unwrap();
    fs::write(
        dir.join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

pub fn create_elf_binary(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00]).unwrap();
    path
}

pub fn create_workflow_binary(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let wf_dir = dir.join(".vo/workflows");
    fs::create_dir_all(&wf_dir).unwrap();
    let path = wf_dir.join(name);
    fs::write(&path, content).unwrap();
    path
}
