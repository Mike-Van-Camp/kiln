//! Control flow analysis: function detection, basic blocks, cross-references.

use crate::model::{
    BasicBlock, BinaryImage, CrossReference, Function, Instruction, SymbolKind, XrefType,
};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

/// The central analysis database holding all analysis results.
#[derive(Debug, Default)]
pub struct AnalysisDatabase {
    pub functions: BTreeMap<u64, Function>,
    pub xrefs: Vec<CrossReference>,
    pub instructions: BTreeMap<u64, Instruction>,
    xrefs_to_map: BTreeMap<u64, Vec<CrossReference>>,
    xrefs_from_map: BTreeMap<u64, Vec<CrossReference>>,
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

    /// Add a single cross-reference, updating all lookup structures.
    pub fn add_xref(&mut self, xref: CrossReference) {
        self.xrefs_to_map
            .entry(xref.to_addr)
            .or_default()
            .push(xref.clone());
        self.xrefs_from_map
            .entry(xref.from_addr)
            .or_default()
            .push(xref.clone());
        self.xrefs.push(xref);
    }

    /// Get all cross-references pointing TO a given address.
    pub fn xrefs_to(&self, addr: u64) -> &[CrossReference] {
        self.xrefs_to_map
            .get(&addr)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all cross-references originating FROM a given address.
    pub fn xrefs_from(&self, addr: u64) -> &[CrossReference] {
        self.xrefs_from_map
            .get(&addr)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Run full control flow analysis on a binary image.
    ///
    /// This performs recursive descent disassembly starting from the binary's
    /// entry point and all function symbols, discovering basic blocks, functions,
    /// and cross-references.
    pub fn run_analysis(&mut self, image: &BinaryImage) {
        let mut function_entries: BTreeSet<u64> = BTreeSet::new();

        // Collect entry points: the binary entry point + all function symbols.
        if image.entry_point != 0 {
            function_entries.insert(image.entry_point);
        }
        for sym in &image.symbols {
            if sym.kind == SymbolKind::Function && sym.address != 0 {
                function_entries.insert(sym.address);
            }
        }

        // BFS work queue of function entry addresses to analyze.
        let mut work_queue: VecDeque<u64> = function_entries.iter().copied().collect();
        let mut visited_functions: HashSet<u64> = HashSet::new();

        while let Some(entry_addr) = work_queue.pop_front() {
            if !visited_functions.insert(entry_addr) {
                continue;
            }
            if self.instructions.get(&entry_addr).is_none() {
                continue;
            }

            let (blocks, new_function_entries) = self.analyze_function(entry_addr);

            for new_entry in new_function_entries {
                if !visited_functions.contains(&new_entry) {
                    function_entries.insert(new_entry);
                    work_queue.push_back(new_entry);
                }
            }

            let name = image
                .symbols
                .iter()
                .find(|s| s.address == entry_addr && !s.name.is_empty())
                .map(|s| s.name.clone())
                .unwrap_or_else(|| format!("sub_{:x}", entry_addr));

            let xrefs_to: Vec<CrossReference> = self
                .xrefs_to(entry_addr)
                .to_vec();
            let xrefs_from: Vec<CrossReference> = blocks
                .iter()
                .flat_map(|b| {
                    b.instructions.iter().flat_map(|insn| {
                        self.xrefs_from(insn.address).to_vec()
                    })
                })
                .collect();

            let func = Function {
                name,
                entry_addr,
                blocks,
                xrefs_to,
                xrefs_from,
            };
            self.functions.insert(entry_addr, func);
        }
    }

    /// Analyze a single function starting at `entry_addr`.
    /// Returns the list of basic blocks and any newly discovered function entry points.
    fn analyze_function(&mut self, entry_addr: u64) -> (Vec<BasicBlock>, Vec<u64>) {
        let mut new_function_entries: Vec<u64> = Vec::new();
        // Addresses where basic blocks must start (block leaders).
        let mut block_leaders: BTreeSet<u64> = BTreeSet::new();
        block_leaders.insert(entry_addr);

        // First pass: walk the instruction stream to discover block boundaries and xrefs.
        let mut visited: HashSet<u64> = HashSet::new();
        let mut walk_queue: VecDeque<u64> = VecDeque::new();
        walk_queue.push_back(entry_addr);

        // Track which addresses belong to this function.
        let mut function_addrs: BTreeSet<u64> = BTreeSet::new();

        while let Some(addr) = walk_queue.pop_front() {
            if !visited.insert(addr) {
                continue;
            }

            let mut current = addr;
            loop {
                let insn = match self.instructions.get(&current) {
                    Some(i) => i.clone(),
                    None => break,
                };
                function_addrs.insert(current);
                let next_addr = current + insn.size as u64;

                if is_return_mnemonic(&insn.mnemonic) {
                    // End of block, no successor.
                    break;
                } else if is_call_mnemonic(&insn.mnemonic) {
                    if let Some(target) = parse_target_address(&insn.operands) {
                        self.add_xref(CrossReference {
                            from_addr: current,
                            to_addr: target,
                            xref_type: XrefType::Call,
                        });
                        if self.instructions.contains_key(&target) {
                            new_function_entries.push(target);
                        }
                    }
                    // After a call, execution continues at next instruction.
                    // The next instruction starts a new block.
                    if self.instructions.contains_key(&next_addr) {
                        block_leaders.insert(next_addr);
                    }
                    current = next_addr;
                } else if is_branch_mnemonic(&insn.mnemonic) {
                    if let Some(target) = parse_target_address(&insn.operands) {
                        self.add_xref(CrossReference {
                            from_addr: current,
                            to_addr: target,
                            xref_type: XrefType::Jump,
                        });
                        if self.instructions.contains_key(&target) {
                            block_leaders.insert(target);
                            if !visited.contains(&target) {
                                walk_queue.push_back(target);
                            }
                        }
                    }
                    // Conditional branches also fall through.
                    if is_conditional_branch(&insn.mnemonic)
                        && self.instructions.contains_key(&next_addr)
                    {
                        block_leaders.insert(next_addr);
                        if !visited.contains(&next_addr) {
                            walk_queue.push_back(next_addr);
                        }
                    }
                    // Unconditional branch: block ends, no fall-through.
                    break;
                } else {
                    // Normal instruction: check if next address is a block leader.
                    // If so, we must end the current block here.
                    if block_leaders.contains(&next_addr) {
                        break;
                    }
                    current = next_addr;
                }
            }
        }

        // Second pass: build basic blocks from the discovered leaders.
        let leaders: Vec<u64> = block_leaders
            .iter()
            .copied()
            .filter(|a| function_addrs.contains(a))
            .collect();

        let mut blocks: Vec<BasicBlock> = Vec::new();

        for &leader in &leaders {
            let mut instructions: Vec<Instruction> = Vec::new();
            let mut current = leader;

            loop {
                let insn = match self.instructions.get(&current) {
                    Some(i) => i.clone(),
                    None => break,
                };
                if !function_addrs.contains(&current) {
                    break;
                }
                let next_addr = current + insn.size as u64;
                let is_ret = is_return_mnemonic(&insn.mnemonic);
                let is_br = is_branch_mnemonic(&insn.mnemonic);
                let is_cl = is_call_mnemonic(&insn.mnemonic);
                instructions.push(insn);

                if is_ret || is_br {
                    break;
                }
                if is_cl && block_leaders.contains(&next_addr) {
                    break;
                }
                // If the next address is a different block leader, end this block.
                if block_leaders.contains(&next_addr) {
                    break;
                }
                current = next_addr;
            }

            if instructions.is_empty() {
                continue;
            }

            let start_addr = instructions.first().unwrap().address;
            let last = instructions.last().unwrap();
            let end_addr = last.address + last.size as u64;

            let mut successors: Vec<u64> = Vec::new();
            let last_insn = instructions.last().unwrap();
            if is_return_mnemonic(&last_insn.mnemonic) {
                // No successors.
            } else if is_branch_mnemonic(&last_insn.mnemonic) {
                if let Some(target) = parse_target_address(&last_insn.operands) {
                    if function_addrs.contains(&target) || block_leaders.contains(&target) {
                        successors.push(target);
                    }
                }
                // Conditional branches fall through.
                if is_conditional_branch(&last_insn.mnemonic)
                    && (function_addrs.contains(&end_addr) || block_leaders.contains(&end_addr))
                {
                    successors.push(end_addr);
                }
            } else {
                // Fall-through to next block.
                if block_leaders.contains(&end_addr) && function_addrs.contains(&end_addr) {
                    successors.push(end_addr);
                }
            }

            blocks.push(BasicBlock {
                start_addr,
                end_addr,
                instructions,
                successors,
                predecessors: Vec::new(),
            });
        }

        // Fill in predecessor lists.
        let block_starts: HashSet<u64> = blocks.iter().map(|b| b.start_addr).collect();
        let successors_map: BTreeMap<u64, Vec<u64>> = blocks
            .iter()
            .map(|b| (b.start_addr, b.successors.clone()))
            .collect();

        for block in &mut blocks {
            for (&src, succs) in &successors_map {
                if succs.contains(&block.start_addr) && block_starts.contains(&src) {
                    block.predecessors.push(src);
                }
            }
        }

        (blocks, new_function_entries)
    }
}

/// Parse a target address from an operand string.
///
/// Handles formats like "0x401000", "0x1234", etc.
/// Returns the first hex address found that looks like a code address.
pub fn parse_target_address(operands: &str) -> Option<u64> {
    // Look for 0x-prefixed hex numbers.
    let mut i = 0;
    let bytes = operands.as_bytes();
    while i + 2 < bytes.len() {
        if bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
            let start = i + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            if end > start {
                if let Ok(addr) = u64::from_str_radix(&operands[start..end], 16) {
                    return Some(addr);
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
    None
}

/// Check if a mnemonic is a branch/jump instruction.
pub fn is_branch_mnemonic(m: &str) -> bool {
    let lower = m.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "jmp" | "je" | "jne" | "jz" | "jnz"
            | "jg" | "jge" | "jl" | "jle"
            | "ja" | "jae" | "jb" | "jbe"
            | "jo" | "jno" | "js" | "jns"
            | "jp" | "jnp" | "jpe" | "jpo"
            | "jcxz" | "jecxz" | "jrcxz"
            | "b" | "bne" | "beq" | "blt" | "bgt" | "bge" | "ble"
            | "bx" | "bhi" | "bls" | "bcc" | "bcs"
            | "bpl" | "bmi" | "bvs" | "bvc"
            | "loop" | "loope" | "loopne" | "loopz" | "loopnz"
    )
}

/// Check if a mnemonic is a call instruction.
pub fn is_call_mnemonic(m: &str) -> bool {
    let lower = m.to_ascii_lowercase();
    matches!(lower.as_str(), "call" | "bl" | "blr" | "blx")
}

/// Check if a mnemonic is a return instruction.
pub fn is_return_mnemonic(m: &str) -> bool {
    let lower = m.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "ret" | "retn" | "retf" | "iret" | "sysret"
    )
}

/// Check if a branch mnemonic is conditional (i.e., has a fall-through path).
fn is_conditional_branch(m: &str) -> bool {
    let lower = m.to_ascii_lowercase();
    // Unconditional branches that do NOT fall through.
    // Note: ARM `bx` is treated as unconditional (branch-and-exchange);
    // it is typically used for indirect jumps or returns.
    if matches!(lower.as_str(), "jmp" | "b" | "bx") {
        return false;
    }
    is_branch_mnemonic(&lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Architecture, BinaryFormat, BinaryImage, Instruction, Section, SectionKind, Symbol,
        SymbolKind,
    };

    fn make_insn(address: u64, size: u8, mnemonic: &str, operands: &str) -> Instruction {
        Instruction {
            address,
            size,
            bytes: vec![0; size as usize],
            mnemonic: mnemonic.to_string(),
            operands: operands.to_string(),
        }
    }

    fn make_image(entry_point: u64, symbols: Vec<Symbol>) -> BinaryImage {
        BinaryImage {
            filename: "test".to_string(),
            format: BinaryFormat::Elf,
            architecture: Architecture::X86_64,
            entry_point,
            bits: 64,
            is_little_endian: true,
            sections: vec![Section {
                name: ".text".to_string(),
                address: 0x1000,
                size: 0x1000,
                file_offset: 0,
                kind: SectionKind::Code,
                readable: true,
                writable: false,
                executable: true,
            }],
            segments: vec![],
            symbols,
            data: vec![],
        }
    }

    // ---------------------------------------------------------------
    // parse_target_address tests
    // ---------------------------------------------------------------
    #[test]
    fn test_parse_target_address_simple() {
        assert_eq!(parse_target_address("0x401000"), Some(0x401000));
    }

    #[test]
    fn test_parse_target_address_uppercase() {
        assert_eq!(parse_target_address("0X1A2B"), Some(0x1A2B));
    }

    #[test]
    fn test_parse_target_address_embedded() {
        assert_eq!(
            parse_target_address("qword ptr [rip + 0x2000]"),
            Some(0x2000)
        );
    }

    #[test]
    fn test_parse_target_address_none() {
        assert_eq!(parse_target_address("rax"), None);
        assert_eq!(parse_target_address(""), None);
    }

    // ---------------------------------------------------------------
    // mnemonic classifier tests
    // ---------------------------------------------------------------
    #[test]
    fn test_is_branch_mnemonic() {
        assert!(is_branch_mnemonic("jmp"));
        assert!(is_branch_mnemonic("JNE"));
        assert!(is_branch_mnemonic("je"));
        assert!(is_branch_mnemonic("bne"));
        assert!(is_branch_mnemonic("loop"));
        assert!(!is_branch_mnemonic("call"));
        assert!(!is_branch_mnemonic("ret"));
        assert!(!is_branch_mnemonic("mov"));
    }

    #[test]
    fn test_is_call_mnemonic() {
        assert!(is_call_mnemonic("call"));
        assert!(is_call_mnemonic("CALL"));
        assert!(is_call_mnemonic("bl"));
        assert!(is_call_mnemonic("blr"));
        assert!(!is_call_mnemonic("jmp"));
        assert!(!is_call_mnemonic("ret"));
    }

    #[test]
    fn test_is_return_mnemonic() {
        assert!(is_return_mnemonic("ret"));
        assert!(is_return_mnemonic("RET"));
        assert!(is_return_mnemonic("retn"));
        assert!(is_return_mnemonic("iret"));
        assert!(!is_return_mnemonic("call"));
        assert!(!is_return_mnemonic("jmp"));
    }

    // ---------------------------------------------------------------
    // xref query tests
    // ---------------------------------------------------------------
    #[test]
    fn test_add_and_query_xrefs() {
        let mut db = AnalysisDatabase::new();
        db.add_xref(CrossReference {
            from_addr: 0x1000,
            to_addr: 0x2000,
            xref_type: XrefType::Call,
        });
        db.add_xref(CrossReference {
            from_addr: 0x1010,
            to_addr: 0x2000,
            xref_type: XrefType::Jump,
        });

        let to = db.xrefs_to(0x2000);
        assert_eq!(to.len(), 2);
        assert_eq!(to[0].from_addr, 0x1000);
        assert_eq!(to[1].from_addr, 0x1010);

        let from = db.xrefs_from(0x1000);
        assert_eq!(from.len(), 1);
        assert_eq!(from[0].to_addr, 0x2000);

        assert_eq!(db.xrefs_to(0x9999).len(), 0);
        assert_eq!(db.xrefs.len(), 2);
    }

    // ---------------------------------------------------------------
    // basic block splitting tests
    // ---------------------------------------------------------------
    #[test]
    fn test_basic_block_splitting() {
        let mut db = AnalysisDatabase::new();

        // Simple function:
        //   0x1000: mov rax, 1       (2 bytes)
        //   0x1002: cmp rax, 0       (2 bytes)
        //   0x1004: jne 0x100a       (2 bytes) -- conditional branch
        //   0x1006: mov rbx, 0       (2 bytes) -- fall-through block
        //   0x1008: jmp 0x100c       (2 bytes) -- unconditional jump
        //   0x100a: mov rbx, 1       (2 bytes) -- branch target block
        //   0x100c: ret              (1 byte)  -- return block
        let instructions = vec![
            make_insn(0x1000, 2, "mov", "rax, 1"),
            make_insn(0x1002, 2, "cmp", "rax, 0"),
            make_insn(0x1004, 2, "jne", "0x100a"),
            make_insn(0x1006, 2, "mov", "rbx, 0"),
            make_insn(0x1008, 2, "jmp", "0x100c"),
            make_insn(0x100a, 2, "mov", "rbx, 1"),
            make_insn(0x100c, 1, "ret", ""),
        ];
        db.index_instructions(instructions);

        let image = make_image(0x1000, vec![]);
        db.run_analysis(&image);

        assert_eq!(db.functions.len(), 1);
        let func = db.functions.get(&0x1000).unwrap();
        assert_eq!(func.entry_addr, 0x1000);

        // Expect 4 basic blocks:
        // Block 1: 0x1000..0x1006 (mov, cmp, jne) -> successors: 0x100a, 0x1006
        // Block 2: 0x1006..0x100a (mov, jmp)       -> successors: 0x100c
        // Block 3: 0x100a..0x100c (mov)             -> successors: 0x100c
        // Block 4: 0x100c..0x100d (ret)             -> successors: none
        assert_eq!(func.blocks.len(), 4);

        let b0 = func.blocks.iter().find(|b| b.start_addr == 0x1000).unwrap();
        assert_eq!(b0.instructions.len(), 3);
        assert!(b0.successors.contains(&0x100a));
        assert!(b0.successors.contains(&0x1006));

        let b1 = func.blocks.iter().find(|b| b.start_addr == 0x1006).unwrap();
        assert_eq!(b1.instructions.len(), 2);
        assert!(b1.successors.contains(&0x100c));

        let b2 = func.blocks.iter().find(|b| b.start_addr == 0x100a).unwrap();
        assert_eq!(b2.instructions.len(), 1);
        assert!(b2.successors.contains(&0x100c));

        let b3 = func.blocks.iter().find(|b| b.start_addr == 0x100c).unwrap();
        assert_eq!(b3.instructions.len(), 1);
        assert!(b3.successors.is_empty());
    }

    #[test]
    fn test_function_naming_from_symbol() {
        let mut db = AnalysisDatabase::new();
        db.index_instructions(vec![make_insn(0x2000, 1, "ret", "")]);

        let image = make_image(
            0x2000,
            vec![Symbol {
                name: "main".to_string(),
                address: 0x2000,
                size: 1,
                kind: SymbolKind::Function,
                is_import: false,
                is_export: true,
            }],
        );
        db.run_analysis(&image);

        let func = db.functions.get(&0x2000).unwrap();
        assert_eq!(func.name, "main");
    }

    #[test]
    fn test_function_auto_naming() {
        let mut db = AnalysisDatabase::new();
        db.index_instructions(vec![make_insn(0x3000, 1, "ret", "")]);

        let image = make_image(0x3000, vec![]);
        db.run_analysis(&image);

        let func = db.functions.get(&0x3000).unwrap();
        assert_eq!(func.name, "sub_3000");
    }

    #[test]
    fn test_call_creates_new_function() {
        let mut db = AnalysisDatabase::new();
        db.index_instructions(vec![
            make_insn(0x1000, 2, "call", "0x2000"),
            make_insn(0x1002, 1, "ret", ""),
            make_insn(0x2000, 1, "ret", ""),
        ]);

        let image = make_image(0x1000, vec![]);
        db.run_analysis(&image);

        assert!(db.functions.contains_key(&0x1000));
        assert!(db.functions.contains_key(&0x2000));

        let xrefs = db.xrefs_to(0x2000);
        assert_eq!(xrefs.len(), 1);
        assert_eq!(xrefs[0].xref_type, XrefType::Call);
    }

    #[test]
    fn test_predecessors_filled() {
        let mut db = AnalysisDatabase::new();
        // 0x1000: jne 0x1004
        // 0x1002: nop (fall-through)
        // 0x1004: ret
        db.index_instructions(vec![
            make_insn(0x1000, 2, "jne", "0x1004"),
            make_insn(0x1002, 2, "nop", ""),
            make_insn(0x1004, 1, "ret", ""),
        ]);

        let image = make_image(0x1000, vec![]);
        db.run_analysis(&image);

        let func = db.functions.get(&0x1000).unwrap();
        let ret_block = func.blocks.iter().find(|b| b.start_addr == 0x1004).unwrap();
        // 0x1004 should be reachable from both 0x1000 (branch) and 0x1002 (fall-through).
        assert!(ret_block.predecessors.contains(&0x1000));
        assert!(ret_block.predecessors.contains(&0x1002));
    }

    // ---------------------------------------------------------------
    // existing API tests
    // ---------------------------------------------------------------
    #[test]
    fn test_index_and_get_instructions() {
        let mut db = AnalysisDatabase::new();
        db.index_instructions(vec![
            make_insn(0x100, 2, "mov", "rax, rbx"),
            make_insn(0x102, 3, "add", "rax, 1"),
        ]);
        assert!(db.get_instruction(0x100).is_some());
        assert!(db.get_instruction(0x102).is_some());
        assert!(db.get_instruction(0x105).is_none());
    }

    #[test]
    fn test_instructions_in_range() {
        let mut db = AnalysisDatabase::new();
        db.index_instructions(vec![
            make_insn(0x100, 2, "mov", ""),
            make_insn(0x102, 2, "add", ""),
            make_insn(0x104, 2, "sub", ""),
        ]);
        let range = db.instructions_in_range(0x100, 0x104);
        assert_eq!(range.len(), 2);
    }
}
