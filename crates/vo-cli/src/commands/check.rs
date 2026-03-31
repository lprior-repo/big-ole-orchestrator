use std::path::{Path, PathBuf};

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
            Self::MachO32BigEndian => "valid Mach-O 32-bit binary",
            Self::MachO32LittleEndian => "valid Mach-O 32-bit binary",
            Self::MachO64BigEndian => "valid Mach-O 64-bit binary",
            Self::MachO64LittleEndian => "valid Mach-O 64-bit binary",
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
}

impl PartialEq for CheckError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FileNotFound { path: a }, Self::FileNotFound { path: b }) => a == b,
            (Self::NotRegularFile { path: a }, Self::NotRegularFile { path: b }) => a == b,
            (Self::FileTooSmall { path: a }, Self::FileTooSmall { path: b }) => a == b,
            (
                Self::InvalidMagic { path: a, magic: am },
                Self::InvalidMagic { path: b, magic: bm },
            ) => a == b && am == bm,
            (Self::PermissionDenied { path: a }, Self::PermissionDenied { path: b }) => a == b,
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

pub fn validate_binary_header(path: &Path) -> Result<BinaryFormat, CheckError> {
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

    use std::io::Read;
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

pub fn run_check(path: &Path) -> Result<(), CheckError> {
    let fmt = validate_binary_header(path)?;
    println!("{}: {}", path.display(), fmt.display_name());
    Ok(())
}
