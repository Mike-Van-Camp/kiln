//! Control flow analysis: function detection, basic blocks, cross-references.
//! (Stub for Sprint 5 — will be fully implemented then.)

use crate::model::{BinaryImage, CrossReference, Function, Instruction};
use std::collections::BTreeMap;

/// The central analysis database holding all analysis results.
#[derive(Debug, Default)]
pub struct AnalysisDatabase {
    pub functions: BTreeMap<u64, Function>,
    pub xrefs: Vec<CrossReference>,
    pub instructions: BTreeMap<u64, Instruction>,
}

impl AnalysisDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Index a set of linearly-disassembled instructions into the database.
    pub fn index_instructions(&mut self, instructions: Vec<Instruction>) {
        for insn in instructions {
            self.instructions.insert(insn.address, insn);
        }
    }

    /// Get an instruction at the given address.
    pub fn get_instruction(&self, addr: u64) -> Option<&Instruction> {
        self.instructions.get(&addr)
    }

    /// Get instructions in an address range (inclusive start, exclusive end).
    pub fn instructions_in_range(&self, start: u64, end: u64) -> Vec<&Instruction> {
        self.instructions
            .range(start..end)
            .map(|(_, insn)| insn)
            .collect()
    }

    /// Look up a symbol name at the given address from a binary image.
    pub fn symbol_at_address<'a>(&self, image: &'a BinaryImage, addr: u64) -> Option<&'a str> {
        image
            .symbols
            .iter()
            .find(|s| s.address == addr && !s.name.is_empty())
            .map(|s| s.name.as_str())
    }
}
