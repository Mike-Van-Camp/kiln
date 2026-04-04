use crate::model::{Architecture, BinaryImage, Instruction, Section};
/// Disassembly engine using capstone-rs for multi-architecture support.
use capstone::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DisasmError {
    #[error("Capstone error: {0}")]
    Capstone(String),
    #[error("Unsupported architecture: {0}")]
    UnsupportedArch(String),
    #[error("No data available for section '{0}'")]
    NoSectionData(String),
}

/// Trait for architecture-specific disassemblers.
pub trait Disassembler {
    /// Disassemble a buffer of code bytes starting at the given virtual address.
    fn disassemble(&self, code: &[u8], address: u64) -> Result<Vec<Instruction>, DisasmError>;
}

/// Create a disassembler for the given architecture.
pub fn create_disassembler(arch: Architecture) -> Result<Box<dyn Disassembler>, DisasmError> {
    match arch {
        Architecture::X86 => Ok(Box::new(CapstoneDisassembler::new_x86()?)),
        Architecture::X86_64 => Ok(Box::new(CapstoneDisassembler::new_x86_64()?)),
        Architecture::Arm => Ok(Box::new(CapstoneDisassembler::new_arm()?)),
        Architecture::Aarch64 => Ok(Box::new(CapstoneDisassembler::new_aarch64()?)),
        Architecture::RiscV32 => Ok(Box::new(CapstoneDisassembler::new_riscv32()?)),
        Architecture::RiscV64 => Ok(Box::new(CapstoneDisassembler::new_riscv64()?)),
        Architecture::Unknown => Err(DisasmError::UnsupportedArch("Unknown".to_string())),
    }
}

/// Disassemble all executable sections of a binary image.
pub fn disassemble_executable_sections(
    image: &BinaryImage,
) -> Result<Vec<Instruction>, DisasmError> {
    let disasm = create_disassembler(image.architecture)?;
    let mut all_instructions = Vec::new();

    for section in image.executable_sections() {
        if let Some(data) = image.section_data(section) {
            let instructions = disasm.disassemble(data, section.address)?;
            all_instructions.extend(instructions);
        }
    }

    all_instructions.sort_by_key(|i| i.address);
    Ok(all_instructions)
}

/// Disassemble a specific section.
pub fn disassemble_section(
    image: &BinaryImage,
    section: &Section,
) -> Result<Vec<Instruction>, DisasmError> {
    let disasm = create_disassembler(image.architecture)?;
    let data = image
        .section_data(section)
        .ok_or_else(|| DisasmError::NoSectionData(section.name.clone()))?;
    disasm.disassemble(data, section.address)
}

/// Capstone-based disassembler supporting multiple architectures.
struct CapstoneDisassembler {
    cs: capstone::Capstone,
}

impl CapstoneDisassembler {
    fn new_x86() -> Result<Self, DisasmError> {
        let cs = capstone::Capstone::new()
            .x86()
            .mode(capstone::arch::x86::ArchMode::Mode32)
            .detail(true)
            .build()
            .map_err(|e| DisasmError::Capstone(e.to_string()))?;
        Ok(Self { cs })
    }

    fn new_x86_64() -> Result<Self, DisasmError> {
        let cs = capstone::Capstone::new()
            .x86()
            .mode(capstone::arch::x86::ArchMode::Mode64)
            .detail(true)
            .build()
            .map_err(|e| DisasmError::Capstone(e.to_string()))?;
        Ok(Self { cs })
    }

    fn new_arm() -> Result<Self, DisasmError> {
        let cs = capstone::Capstone::new()
            .arm()
            .mode(capstone::arch::arm::ArchMode::Arm)
            .detail(true)
            .build()
            .map_err(|e| DisasmError::Capstone(e.to_string()))?;
        Ok(Self { cs })
    }

    fn new_aarch64() -> Result<Self, DisasmError> {
        let cs = capstone::Capstone::new()
            .arm64()
            .mode(capstone::arch::arm64::ArchMode::Arm)
            .detail(true)
            .build()
            .map_err(|e| DisasmError::Capstone(e.to_string()))?;
        Ok(Self { cs })
    }

    fn new_riscv32() -> Result<Self, DisasmError> {
        let cs = capstone::Capstone::new()
            .riscv()
            .mode(capstone::arch::riscv::ArchMode::RiscV32)
            .detail(true)
            .build()
            .map_err(|e| DisasmError::Capstone(e.to_string()))?;
        Ok(Self { cs })
    }

    fn new_riscv64() -> Result<Self, DisasmError> {
        let cs = capstone::Capstone::new()
            .riscv()
            .mode(capstone::arch::riscv::ArchMode::RiscV64)
            .detail(true)
            .build()
            .map_err(|e| DisasmError::Capstone(e.to_string()))?;
        Ok(Self { cs })
    }
}

impl Disassembler for CapstoneDisassembler {
    fn disassemble(&self, code: &[u8], address: u64) -> Result<Vec<Instruction>, DisasmError> {
        let insns = self
            .cs
            .disasm_all(code, address)
            .map_err(|e| DisasmError::Capstone(e.to_string()))?;

        let instructions = insns
            .iter()
            .map(|insn| Instruction {
                address: insn.address(),
                size: insn.len() as u8,
                bytes: insn.bytes().to_vec(),
                mnemonic: insn.mnemonic().unwrap_or("").to_string(),
                operands: insn.op_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(instructions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_x86_64_nop() {
        let disasm = CapstoneDisassembler::new_x86_64().unwrap();
        // NOP instruction
        let code = [0x90];
        let result = disasm.disassemble(&code, 0x1000).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].mnemonic, "nop");
        assert_eq!(result[0].address, 0x1000);
        assert_eq!(result[0].size, 1);
    }

    #[test]
    fn test_disassemble_x86_64_mov() {
        let disasm = CapstoneDisassembler::new_x86_64().unwrap();
        // mov eax, 0x42
        let code = [0xb8, 0x42, 0x00, 0x00, 0x00];
        let result = disasm.disassemble(&code, 0x1000).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].mnemonic, "mov");
        assert!(result[0].operands.contains("eax"));
        assert_eq!(result[0].size, 5);
    }

    #[test]
    fn test_disassemble_x86_64_ret() {
        let disasm = CapstoneDisassembler::new_x86_64().unwrap();
        // ret
        let code = [0xc3];
        let result = disasm.disassemble(&code, 0x2000).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].mnemonic, "ret");
    }

    #[test]
    fn test_disassemble_x86_64_sequence() {
        let disasm = CapstoneDisassembler::new_x86_64().unwrap();
        // push rbp; mov rbp, rsp; pop rbp; ret
        let code = [0x55, 0x48, 0x89, 0xe5, 0x5d, 0xc3];
        let result = disasm.disassemble(&code, 0x1000).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].mnemonic, "push");
        assert_eq!(result[1].mnemonic, "mov");
        assert_eq!(result[2].mnemonic, "pop");
        assert_eq!(result[3].mnemonic, "ret");
        // Addresses should be sequential
        assert_eq!(result[0].address, 0x1000);
        assert_eq!(result[3].address, 0x1005);
    }

    #[test]
    fn test_disassemble_x86_32() {
        let disasm = CapstoneDisassembler::new_x86().unwrap();
        // nop; ret
        let code = [0x90, 0xc3];
        let result = disasm.disassemble(&code, 0x400000).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].mnemonic, "nop");
        assert_eq!(result[1].mnemonic, "ret");
    }

    #[test]
    fn test_disassemble_arm() {
        let disasm = CapstoneDisassembler::new_arm().unwrap();
        // NOP in ARM: mov r0, r0 = 0xe1a00000
        let code: [u8; 4] = [0x00, 0x00, 0xa0, 0xe1];
        let result = disasm.disassemble(&code, 0x8000).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].size, 4);
    }

    #[test]
    fn test_disassemble_aarch64() {
        let disasm = CapstoneDisassembler::new_aarch64().unwrap();
        // NOP in AArch64: 0xd503201f
        let code: [u8; 4] = [0x1f, 0x20, 0x03, 0xd5];
        let result = disasm.disassemble(&code, 0x10000).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].mnemonic, "nop");
        assert_eq!(result[0].size, 4);
    }

    #[test]
    fn test_disassemble_empty_input() {
        let disasm = CapstoneDisassembler::new_x86_64().unwrap();
        let result = disasm.disassemble(&[], 0x1000).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_create_disassembler_all_archs() {
        assert!(create_disassembler(Architecture::X86).is_ok());
        assert!(create_disassembler(Architecture::X86_64).is_ok());
        assert!(create_disassembler(Architecture::Arm).is_ok());
        assert!(create_disassembler(Architecture::Aarch64).is_ok());
        assert!(create_disassembler(Architecture::RiscV32).is_ok());
        assert!(create_disassembler(Architecture::RiscV64).is_ok());
        assert!(create_disassembler(Architecture::Unknown).is_err());
    }

    #[test]
    fn test_disassemble_self_binary() {
        let exe_path = std::env::current_exe().unwrap();
        let image = crate::load_binary(&exe_path).unwrap();
        let instructions = disassemble_executable_sections(&image).unwrap();
        assert!(
            !instructions.is_empty(),
            "Should disassemble some instructions"
        );
        // Check instructions are sorted by address
        for window in instructions.windows(2) {
            assert!(window[0].address <= window[1].address);
        }
    }

    #[test]
    fn test_instruction_display() {
        let insn = Instruction {
            address: 0x401000,
            size: 1,
            bytes: vec![0x90],
            mnemonic: "nop".to_string(),
            operands: String::new(),
        };
        let display = format!("{}", insn);
        assert!(display.contains("0x00401000"));
        assert!(display.contains("nop"));
    }
}
