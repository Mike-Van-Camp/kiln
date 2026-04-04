//! Core data types for the Kiln disassembler.

/// CPU architecture detected from binary headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86,
    X86_64,
    Arm,
    Aarch64,
    RiscV32,
    RiscV64,
    Unknown,
}

impl std::fmt::Display for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Architecture::X86 => write!(f, "x86"),
            Architecture::X86_64 => write!(f, "x86-64"),
            Architecture::Arm => write!(f, "ARM"),
            Architecture::Aarch64 => write!(f, "AArch64"),
            Architecture::RiscV32 => write!(f, "RISC-V 32"),
            Architecture::RiscV64 => write!(f, "RISC-V 64"),
            Architecture::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Binary file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    Elf,
    Pe,
    MachO,
    Unknown,
}

impl std::fmt::Display for BinaryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryFormat::Elf => write!(f, "ELF"),
            BinaryFormat::Pe => write!(f, "PE"),
            BinaryFormat::MachO => write!(f, "Mach-O"),
            BinaryFormat::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Classification of a section's contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Code,
    Data,
    ReadOnlyData,
    Bss,
    Unknown,
}

/// A section within the binary.
#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub file_offset: u64,
    pub kind: SectionKind,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

/// A segment (program header / load command).
#[derive(Debug, Clone)]
pub struct Segment {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub file_offset: u64,
    pub file_size: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

/// Kind of symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Object,
    Section,
    File,
    Unknown,
}

/// A symbol from the binary's symbol table.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub kind: SymbolKind,
    pub is_import: bool,
    pub is_export: bool,
}

/// A loaded binary image with all parsed metadata.
#[derive(Debug, Clone)]
pub struct BinaryImage {
    pub filename: String,
    pub format: BinaryFormat,
    pub architecture: Architecture,
    pub entry_point: u64,
    pub bits: u32,
    pub is_little_endian: bool,
    pub sections: Vec<Section>,
    pub segments: Vec<Segment>,
    pub symbols: Vec<Symbol>,
    pub data: Vec<u8>,
}

impl BinaryImage {
    /// Find a section by name.
    pub fn find_section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Get all executable sections.
    pub fn executable_sections(&self) -> Vec<&Section> {
        self.sections.iter().filter(|s| s.executable).collect()
    }

    /// Get all imported symbols.
    pub fn imports(&self) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.is_import).collect()
    }

    /// Get all exported symbols.
    pub fn exports(&self) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.is_export).collect()
    }

    /// Get the raw bytes for a section from the binary data.
    pub fn section_data(&self, section: &Section) -> Option<&[u8]> {
        let start = section.file_offset as usize;
        let end = start + section.size as usize;
        if end <= self.data.len() {
            Some(&self.data[start..end])
        } else {
            None
        }
    }

    /// Get function symbols sorted by address.
    pub fn function_symbols(&self) -> Vec<&Symbol> {
        let mut funcs: Vec<&Symbol> = self
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && !s.name.is_empty())
            .collect();
        funcs.sort_by_key(|s| s.address);
        funcs
    }
}

/// A disassembled instruction.
#[derive(Debug, Clone)]
pub struct Instruction {
    pub address: u64,
    pub size: u8,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operands: String,
}

impl std::fmt::Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "0x{:08x}: {} {}",
            self.address, self.mnemonic, self.operands
        )
    }
}

/// Cross-reference type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrefType {
    Call,
    Jump,
    Data,
}

/// A cross-reference between two addresses.
#[derive(Debug, Clone)]
pub struct CrossReference {
    pub from_addr: u64,
    pub to_addr: u64,
    pub xref_type: XrefType,
}

/// A basic block: a straight-line sequence of instructions.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub start_addr: u64,
    pub end_addr: u64,
    pub instructions: Vec<Instruction>,
    pub successors: Vec<u64>,
    pub predecessors: Vec<u64>,
}

/// A detected function.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub entry_addr: u64,
    pub blocks: Vec<BasicBlock>,
    pub xrefs_to: Vec<CrossReference>,
    pub xrefs_from: Vec<CrossReference>,
}
