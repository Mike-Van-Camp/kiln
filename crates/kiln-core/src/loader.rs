/// Binary loader using goblin for ELF, PE, and Mach-O parsing.
use std::path::Path;

use thiserror::Error;

use crate::model::{
    Architecture, BinaryFormat, BinaryImage, Section, SectionKind, Segment, Symbol, SymbolKind,
};

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Unsupported binary format")]
    UnsupportedFormat,
}

/// Load a binary file from the given path and parse its metadata.
pub fn load_binary<P: AsRef<Path>>(path: P) -> Result<BinaryImage, LoadError> {
    let path = path.as_ref();
    let data = std::fs::read(path)?;
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    load_binary_from_bytes(data, filename)
}

/// Load a binary from raw bytes (useful for testing).
pub fn load_binary_from_bytes(data: Vec<u8>, filename: String) -> Result<BinaryImage, LoadError> {
    // Detect format first, then parse with ownership of data
    let magic = if data.len() >= 4 {
        [data[0], data[1], data[2], data[3]]
    } else {
        return Err(LoadError::Parse("File too small".to_string()));
    };

    // Check for ELF magic
    if magic == [0x7f, b'E', b'L', b'F'] {
        let elf = goblin::elf::Elf::parse(&data).map_err(|e| LoadError::Parse(e.to_string()))?;
        let mut image = parse_elf(&elf, &filename)?;
        image.data = data;
        return Ok(image);
    }

    // Check for PE magic (MZ header)
    if magic[0] == b'M' && magic[1] == b'Z' {
        let pe = goblin::pe::PE::parse(&data).map_err(|e| LoadError::Parse(e.to_string()))?;
        let mut image = parse_pe(&pe, &filename)?;
        image.data = data;
        return Ok(image);
    }

    // Check for Mach-O magic
    let magic32 = u32::from_le_bytes(magic);
    if magic32 == goblin::mach::header::MH_MAGIC
        || magic32 == goblin::mach::header::MH_MAGIC_64
        || magic32 == goblin::mach::header::MH_CIGAM
        || magic32 == goblin::mach::header::MH_CIGAM_64
        || magic32 == goblin::mach::fat::FAT_MAGIC
    {
        let mach = goblin::mach::Mach::parse(&data).map_err(|e| LoadError::Parse(e.to_string()))?;
        match mach {
            goblin::mach::Mach::Binary(ref macho) => {
                let mut image = parse_single_macho(macho, &filename)?;
                image.data = data;
                return Ok(image);
            }
            goblin::mach::Mach::Fat(fat) => {
                // For fat binaries, parse the first architecture
                let arch = fat.iter_arches().next();
                match arch {
                    Some(Ok(arch_entry)) => {
                        let start = arch_entry.offset as usize;
                        let end = start + arch_entry.size as usize;
                        if end <= data.len() {
                            let slice = data[start..end].to_vec();
                            return load_binary_from_bytes(slice, filename);
                        } else {
                            return Err(LoadError::Parse(
                                "Fat binary arch entry out of bounds".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(LoadError::Parse(
                            "No architectures in fat binary".to_string(),
                        ));
                    }
                }
            }
        }
    }

    Err(LoadError::UnsupportedFormat)
}

fn parse_elf(elf: &goblin::elf::Elf, filename: &str) -> Result<BinaryImage, LoadError> {
    let architecture = match elf.header.e_machine {
        goblin::elf::header::EM_386 => Architecture::X86,
        goblin::elf::header::EM_X86_64 => Architecture::X86_64,
        goblin::elf::header::EM_ARM => Architecture::Arm,
        goblin::elf::header::EM_AARCH64 => Architecture::Aarch64,
        goblin::elf::header::EM_RISCV => {
            if elf.is_64 {
                Architecture::RiscV64
            } else {
                Architecture::RiscV32
            }
        }
        _ => Architecture::Unknown,
    };

    let bits = if elf.is_64 { 64 } else { 32 };
    let is_little_endian = elf.little_endian;
    let entry_point = elf.entry;

    let sections = elf
        .section_headers
        .iter()
        .map(|sh| {
            let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("").to_string();
            let flags = sh.sh_flags as u32;
            let executable = flags & goblin::elf::section_header::SHF_EXECINSTR != 0;
            let writable = flags & goblin::elf::section_header::SHF_WRITE != 0;
            let alloc = flags & goblin::elf::section_header::SHF_ALLOC != 0;

            let kind = if executable {
                SectionKind::Code
            } else if sh.sh_type == goblin::elf::section_header::SHT_NOBITS {
                SectionKind::Bss
            } else if !writable && alloc {
                SectionKind::ReadOnlyData
            } else if writable && alloc {
                SectionKind::Data
            } else {
                SectionKind::Unknown
            };

            Section {
                name,
                address: sh.sh_addr,
                size: sh.sh_size,
                file_offset: sh.sh_offset,
                kind,
                readable: alloc,
                writable,
                executable,
            }
        })
        .collect();

    let segments = elf
        .program_headers
        .iter()
        .map(|ph| {
            let readable = ph.p_flags & goblin::elf::program_header::PF_R != 0;
            let writable = ph.p_flags & goblin::elf::program_header::PF_W != 0;
            let executable = ph.p_flags & goblin::elf::program_header::PF_X != 0;

            Segment {
                name: format!("{:?}", ph.p_type),
                address: ph.p_vaddr,
                size: ph.p_memsz,
                file_offset: ph.p_offset,
                file_size: ph.p_filesz,
                readable,
                writable,
                executable,
            }
        })
        .collect();

    let symbols = elf
        .syms
        .iter()
        .chain(elf.dynsyms.iter())
        .filter_map(|sym| {
            let name = elf
                .strtab
                .get_at(sym.st_name)
                .or_else(|| elf.dynstrtab.get_at(sym.st_name))
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                return None;
            }

            let kind = match sym.st_type() {
                goblin::elf::sym::STT_FUNC => SymbolKind::Function,
                goblin::elf::sym::STT_OBJECT => SymbolKind::Object,
                goblin::elf::sym::STT_SECTION => SymbolKind::Section,
                goblin::elf::sym::STT_FILE => SymbolKind::File,
                _ => SymbolKind::Unknown,
            };

            let is_import = sym.st_shndx == goblin::elf::section_header::SHN_UNDEF as usize
                && sym.st_value == 0;
            let is_export = sym.st_bind() == goblin::elf::sym::STB_GLOBAL
                && sym.st_shndx != goblin::elf::section_header::SHN_UNDEF as usize;

            Some(Symbol {
                name,
                address: sym.st_value,
                size: sym.st_size,
                kind,
                is_import,
                is_export,
            })
        })
        .collect();

    Ok(BinaryImage {
        filename: filename.to_string(),
        format: BinaryFormat::Elf,
        architecture,
        entry_point,
        bits,
        is_little_endian,
        sections,
        segments,
        symbols,
        data: Vec::new(),
    })
}

fn parse_pe(pe: &goblin::pe::PE, filename: &str) -> Result<BinaryImage, LoadError> {
    let architecture = match pe.header.coff_header.machine {
        goblin::pe::header::COFF_MACHINE_X86 => Architecture::X86,
        goblin::pe::header::COFF_MACHINE_X86_64 => Architecture::X86_64,
        goblin::pe::header::COFF_MACHINE_ARM64 => Architecture::Aarch64,
        _ => Architecture::Unknown,
    };

    let bits = if pe.is_64 { 64 } else { 32 };
    let entry_point = pe
        .header
        .optional_header
        .map(|oh| oh.standard_fields.address_of_entry_point)
        .unwrap_or(0);

    let image_base = pe.image_base as u64;

    let sections = pe
        .sections
        .iter()
        .map(|sec| {
            let name = String::from_utf8_lossy(
                &sec.name[..sec
                    .name
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(sec.name.len())],
            )
            .to_string();

            let chars = sec.characteristics;
            let executable = chars & goblin::pe::section_table::IMAGE_SCN_MEM_EXECUTE != 0;
            let readable = chars & goblin::pe::section_table::IMAGE_SCN_MEM_READ != 0;
            let writable = chars & goblin::pe::section_table::IMAGE_SCN_MEM_WRITE != 0;
            let has_code = chars & goblin::pe::section_table::IMAGE_SCN_CNT_CODE != 0;
            let has_data = chars & goblin::pe::section_table::IMAGE_SCN_CNT_INITIALIZED_DATA != 0;
            let has_uninit =
                chars & goblin::pe::section_table::IMAGE_SCN_CNT_UNINITIALIZED_DATA != 0;

            let kind = if has_code || executable {
                SectionKind::Code
            } else if has_uninit {
                SectionKind::Bss
            } else if has_data && !writable {
                SectionKind::ReadOnlyData
            } else if has_data {
                SectionKind::Data
            } else {
                SectionKind::Unknown
            };

            Section {
                name,
                address: sec.virtual_address as u64 + image_base,
                size: sec.virtual_size as u64,
                file_offset: sec.pointer_to_raw_data as u64,
                kind,
                readable,
                writable,
                executable,
            }
        })
        .collect();

    let symbols = pe
        .imports
        .iter()
        .map(|imp| Symbol {
            name: imp.name.to_string(),
            address: imp.offset as u64 + image_base,
            size: 0,
            kind: SymbolKind::Function,
            is_import: true,
            is_export: false,
        })
        .chain(pe.exports.iter().map(|exp| Symbol {
            name: exp.name.unwrap_or("").to_string(),
            address: exp.rva as u64 + image_base,
            size: exp.size as u64,
            kind: SymbolKind::Function,
            is_import: false,
            is_export: true,
        }))
        .filter(|s| !s.name.is_empty())
        .collect();

    Ok(BinaryImage {
        filename: filename.to_string(),
        format: BinaryFormat::Pe,
        architecture,
        entry_point: entry_point + image_base,
        bits,
        is_little_endian: true,
        sections,
        segments: Vec::new(),
        symbols,
        data: Vec::new(),
    })
}

fn parse_single_macho(
    macho: &goblin::mach::MachO,
    filename: &str,
) -> Result<BinaryImage, LoadError> {
    let architecture = match macho.header.cputype() {
        goblin::mach::cputype::CPU_TYPE_X86 => Architecture::X86,
        goblin::mach::cputype::CPU_TYPE_X86_64 => Architecture::X86_64,
        goblin::mach::cputype::CPU_TYPE_ARM => Architecture::Arm,
        goblin::mach::cputype::CPU_TYPE_ARM64 => Architecture::Aarch64,
        _ => Architecture::Unknown,
    };

    let bits = if macho.is_64 { 64 } else { 32 };
    let is_little_endian = macho.little_endian;
    let entry_point = macho.entry;

    let sections: Vec<Section> = macho
        .segments
        .sections()
        .flat_map(|sec_iter| {
            sec_iter.filter_map(|result| {
                let (sec, _data) = result.ok()?;
                let seg_name = sec.segname().unwrap_or("");
                let sec_name = sec.name().unwrap_or("");
                let name = format!("{},{}", seg_name, sec_name);
                let executable = sec.flags & 0x80000000 != 0 || seg_name == "__TEXT";
                let writable = seg_name == "__DATA";

                let kind = if executable {
                    SectionKind::Code
                } else if writable {
                    SectionKind::Data
                } else {
                    SectionKind::ReadOnlyData
                };

                Some(Section {
                    name,
                    address: sec.addr,
                    size: sec.size,
                    file_offset: sec.offset as u64,
                    kind,
                    readable: true,
                    writable,
                    executable,
                })
            })
        })
        .collect();

    let segments: Vec<Segment> = macho
        .segments
        .iter()
        .map(|seg| {
            let name = seg.name().unwrap_or("").to_string();
            let initprot = seg.initprot;
            Segment {
                name,
                address: seg.vmaddr,
                size: seg.vmsize,
                file_offset: seg.fileoff,
                file_size: seg.filesize,
                readable: initprot & 1 != 0,
                writable: initprot & 2 != 0,
                executable: initprot & 4 != 0,
            }
        })
        .collect();

    let symbols: Vec<Symbol> = macho
        .symbols()
        .filter_map(|result| {
            let (name, nlist) = result.ok()?;
            let name = name.strip_prefix('_').unwrap_or(name).to_string();
            if name.is_empty() {
                return None;
            }

            let is_external = nlist.is_global();
            let is_undefined = nlist.is_undefined();

            let kind = if nlist.get_type() == goblin::mach::symbols::N_SECT {
                SymbolKind::Function
            } else {
                SymbolKind::Unknown
            };

            Some(Symbol {
                name,
                address: nlist.n_value,
                size: 0,
                kind,
                is_import: is_undefined,
                is_export: is_external && !is_undefined,
            })
        })
        .collect();

    Ok(BinaryImage {
        filename: filename.to_string(),
        format: BinaryFormat::MachO,
        architecture,
        entry_point,
        bits,
        is_little_endian,
        sections,
        segments,
        symbols,
        data: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_self_binary() {
        // Load the test binary itself (should be ELF on Linux)
        let exe_path = std::env::current_exe().expect("Failed to get current exe path");
        let image = load_binary(&exe_path).expect("Failed to load binary");

        assert_eq!(image.format, BinaryFormat::Elf);
        assert_eq!(image.architecture, Architecture::X86_64);
        assert_eq!(image.bits, 64);
        assert!(image.entry_point > 0);
        assert!(!image.sections.is_empty());
        assert!(!image.data.is_empty());
    }

    #[test]
    fn test_load_self_has_text_section() {
        let exe_path = std::env::current_exe().unwrap();
        let image = load_binary(&exe_path).unwrap();

        let text = image.find_section(".text");
        assert!(text.is_some(), "Should have a .text section");
        let text = text.unwrap();
        assert!(text.executable, ".text should be executable");
        assert!(text.size > 0, ".text should have non-zero size");
    }

    #[test]
    fn test_executable_sections() {
        let exe_path = std::env::current_exe().unwrap();
        let image = load_binary(&exe_path).unwrap();

        let exec_sections = image.executable_sections();
        assert!(
            !exec_sections.is_empty(),
            "Should have at least one executable section"
        );
        for sec in &exec_sections {
            assert!(sec.executable);
        }
    }

    #[test]
    fn test_section_data() {
        let exe_path = std::env::current_exe().unwrap();
        let image = load_binary(&exe_path).unwrap();

        let text = image.find_section(".text").unwrap();
        let data = image.section_data(text);
        assert!(data.is_some(), "Should be able to read .text data");
        assert_eq!(data.unwrap().len(), text.size as usize);
    }

    #[test]
    fn test_load_invalid_file() {
        let result = load_binary_from_bytes(vec![0, 1, 2, 3], "invalid.bin".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_empty_data() {
        let result = load_binary_from_bytes(vec![], "empty.bin".to_string());
        assert!(result.is_err());
    }
}
