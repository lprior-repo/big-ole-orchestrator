use std::path::{Path, PathBuf};

use vo_types::WorkflowDefinition;

pub const ELF_MAGIC: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46];
pub const MACHO_MAGIC_32_BE: [u8; 4] = [0xFE, 0xED, 0xFA, 0xCE];
pub const MACHO_MAGIC_32_LE: [u8; 4] = [0xCE, 0xFA, 0xED, 0xFE];
pub const MACHO_MAGIC_64_BE: [u8; 4] = [0xFE, 0xED, 0xFA, 0xCF];
pub const MACHO_MAGIC_64_LE: [u8; 4] = [0xCF, 0xFA, 0xED, 0xFE];

pub const KNOWN_MAGICS: &[[u8; 4]] = &[
    ELF_MAGIC,
    MACHO_MAGIC_32_BE,
    MACHO_MAGIC_32_LE,
    MACHO_MAGIC_64_BE,
    MACHO_MAGIC_64_LE,
];

pub const ELF_MACHINE_X86_64: u16 = 0x3E;
pub const ELF_MACHINE_AARCH64: u16 = 0xB7;
pub const ELF_MACHINE_ARM: u16 = 0x28;
pub const ELF_MACHINE_X86: u16 = 0x03;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryFormat {
    Elf,
    MachO32BigEndian,
    MachO32LittleEndian,
    MachO64BigEndian,
    MachO64LittleEndian,
}

impl BinaryFormat {
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Elf => "valid ELF binary",
            Self::MachO32BigEndian | Self::MachO32LittleEndian => "valid Mach-O 32-bit binary",
            Self::MachO64BigEndian | Self::MachO64LittleEndian => "valid Mach-O 64-bit binary",
        }
    }
}

/// ELF architecture detected from the machine type field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfMachine {
    X86_64,
    AArch64,
    Arm,
    X86,
    Unknown(u16),
}

impl ElfMachine {
    #[must_use]
    pub fn from_u16(value: u16) -> Self {
        match value {
            ELF_MACHINE_X86_64 => Self::X86_64,
            ELF_MACHINE_AARCH64 => Self::AArch64,
            ELF_MACHINE_ARM => Self::Arm,
            ELF_MACHINE_X86 => Self::X86,
            other => Self::Unknown(other),
        }
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::X86_64 => "x86_64".to_string(),
            Self::AArch64 => "AArch64".to_string(),
            Self::Arm => "ARM".to_string(),
            Self::X86 => "x86".to_string(),
            Self::Unknown(n) => format!("unknown-machine-{n}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("file not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("not a regular file: {path}")]
    NotRegularFile { path: PathBuf },

    #[error("file too small to contain a valid binary header (expected at least 4 bytes): {path}")]
    FileTooSmall { path: PathBuf },

    #[error("invalid binary format: {path} -- magic bytes {} do not match ELF or Mach-O", format!("[{:#04x}, {:#04x}, {:#04x}, {:#04x}]", magic[0], magic[1], magic[2], magic[3]))]
    InvalidMagic { path: PathBuf, magic: [u8; 4] },

    #[error("permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("workflow spec validation failed at {path}: {message}")]
    WorkflowSpec { path: PathBuf, message: String },
}

impl PartialEq for CheckError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FileNotFound { path: a }, Self::FileNotFound { path: b })
            | (Self::NotRegularFile { path: a }, Self::NotRegularFile { path: b })
            | (Self::FileTooSmall { path: a }, Self::FileTooSmall { path: b })
            | (Self::PermissionDenied { path: a }, Self::PermissionDenied { path: b }) => a == b,
            (
                Self::InvalidMagic { path: a, magic: am },
                Self::InvalidMagic { path: b, magic: bm },
            ) => a == b && am == bm,
            _ => false,
        }
    }
}

fn identify_magic(magic: [u8; 4]) -> Option<BinaryFormat> {
    match magic {
        ELF_MAGIC => Some(BinaryFormat::Elf),
        MACHO_MAGIC_32_BE => Some(BinaryFormat::MachO32BigEndian),
        MACHO_MAGIC_32_LE => Some(BinaryFormat::MachO32LittleEndian),
        MACHO_MAGIC_64_BE => Some(BinaryFormat::MachO64BigEndian),
        MACHO_MAGIC_64_LE => Some(BinaryFormat::MachO64LittleEndian),
        _ => None,
    }
}

/// Validate a binary header.
///
/// # Errors
/// Returns an error if the file does not exist, lacks permission, is not a regular file, or has invalid magic.
pub fn validate_binary_header(path: &Path) -> Result<BinaryFormat, CheckError> {
    use std::io::Read;
    let sym_meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CheckError::FileNotFound {
                path: path.to_path_buf(),
            });
        }
        Err(e) => {
            return Err(if e.kind() == std::io::ErrorKind::PermissionDenied {
                CheckError::PermissionDenied {
                    path: path.to_path_buf(),
                }
            } else {
                CheckError::Io {
                    path: path.to_path_buf(),
                    source: e,
                }
            });
        }
    };

    if sym_meta.file_type().is_symlink() {
        return Err(CheckError::NotRegularFile {
            path: path.to_path_buf(),
        });
    }

    if !sym_meta.file_type().is_file() {
        return Err(CheckError::NotRegularFile {
            path: path.to_path_buf(),
        });
    }

    let file = std::fs::File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            CheckError::PermissionDenied {
                path: path.to_path_buf(),
            }
        } else {
            CheckError::Io {
                path: path.to_path_buf(),
                source: e,
            }
        }
    })?;

    let mut reader = std::io::BufReader::new(file);

    let mut buf = [0u8; 4];

    let bytes_read = reader.read(&mut buf).map_err(|e| CheckError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    if bytes_read < 4 {
        return Err(CheckError::FileTooSmall {
            path: path.to_path_buf(),
        });
    }

    identify_magic(buf).ok_or_else(|| CheckError::InvalidMagic {
        path: path.to_path_buf(),
        magic: buf,
    })
}

/// Detect the architecture of an ELF binary by reading the machine type field.
///
/// For ELF binaries, the machine type is at offset 18 (2 bytes, little-endian).
/// For non-ELF binaries or when the machine type is not recognized, returns `None`.
///
/// # Errors
/// Returns an error if the file cannot be read.
pub fn detect_elf_architecture(path: &Path) -> Result<Option<ElfMachine>, CheckError> {
    use std::io::Read;

    let format = validate_binary_header(path)?;

    if format != BinaryFormat::Elf {
        return Ok(None);
    }

    let file = std::fs::File::open(path).map_err(|e| CheckError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut reader = std::io::BufReader::new(file);

    // Skip to offset 18 where ELF e_machine field is located
    let mut header = [0u8; 20];
    reader.read_exact(&mut header).map_err(|e| CheckError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    // ELF machine type is little-endian u16 at offset 18
    let machine = u16::from_le_bytes([header[18], header[19]]);

    Ok(Some(ElfMachine::from_u16(machine)))
}

pub fn validate_workflow_spec(path: &Path) -> Result<WorkflowDefinition, CheckError> {
    let contents = std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CheckError::FileNotFound {
                path: path.to_path_buf(),
            }
        } else {
            CheckError::Io {
                path: path.to_path_buf(),
                source: e,
            }
        }
    })?;

    let mut de = serde_json::Deserializer::from_slice(&contents);
    WorkflowDefinition::from_deserializer(&mut de).map_err(|e| CheckError::WorkflowSpec {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Run the check command.
///
/// # Errors
/// Returns an error if the file does not exist, lacks permission, is not a regular file, or has invalid magic.
pub fn run_check(path: &Path) -> Result<(), CheckError> {
    let fmt = validate_binary_header(path)?;
    println!("{}: {}", path.display(), fmt.display_name());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_error_eq_returns_false_for_different_errors() {
        let err1 = CheckError::FileNotFound {
            path: PathBuf::from("a"),
        };
        let err2 = CheckError::FileNotFound {
            path: PathBuf::from("b"),
        };
        assert_ne!(err1, err2);

        let err3 = CheckError::NotRegularFile {
            path: PathBuf::from("a"),
        };
        assert_ne!(err1, err3);
    }
}
