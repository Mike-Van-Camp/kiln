//! Binary diffing engine: function-level and instruction-level comparison.

use crate::analysis::AnalysisDatabase;
use crate::model::{BinaryImage, Function};
use std::collections::BTreeMap;

/// Match status for a function pair across two binaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionMatchStatus {
    /// Same name and identical instructions.
    Matched,
    /// Present only in the new binary.
    Added,
    /// Present only in the old binary.
    Removed,
    /// Same name but instructions differ.
    Modified,
}

/// A pairing of functions between the old and new binaries.
#[derive(Debug, Clone)]
pub struct FunctionMatch {
    pub status: FunctionMatchStatus,
    pub old_function: Option<String>,
    pub new_function: Option<String>,
    pub old_addr: Option<u64>,
    pub new_addr: Option<u64>,
    /// Similarity ratio (0.0–1.0) based on instruction overlap.
    pub similarity: f64,
}

/// Classification of an instruction difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionDiffKind {
    Same,
    Added,
    Removed,
    Modified,
}

/// A snapshot of a single instruction for diffing.
#[derive(Debug, Clone)]
pub struct InstructionSnapshot {
    pub address: u64,
    pub mnemonic: String,
    pub operands: String,
    pub bytes: Vec<u8>,
}

/// One row in the instruction-level diff output.
#[derive(Debug, Clone)]
pub struct InstructionDiff {
    pub kind: InstructionDiffKind,
    pub old_instruction: Option<InstructionSnapshot>,
    pub new_instruction: Option<InstructionSnapshot>,
}

/// Complete diff result between two binaries.
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub old_filename: String,
    pub new_filename: String,
    pub function_matches: Vec<FunctionMatch>,
    /// Function name → instruction-level diffs for matched/modified functions.
    pub instruction_diffs: BTreeMap<String, Vec<InstructionDiff>>,
}

/// Build a canonical key for each instruction (mnemonic + operands).
fn insn_key(mnemonic: &str, operands: &str) -> String {
    if operands.is_empty() {
        mnemonic.to_string()
    } else {
        format!("{} {}", mnemonic, operands)
    }
}

/// Collect instruction snapshots for a function, ordered by block/address.
fn function_instructions(func: &Function) -> Vec<InstructionSnapshot> {
    let mut snapshots = Vec::new();
    let mut sorted_blocks = func.blocks.clone();
    sorted_blocks.sort_by_key(|b| b.start_addr);
    for block in &sorted_blocks {
        for insn in &block.instructions {
            snapshots.push(InstructionSnapshot {
                address: insn.address,
                mnemonic: insn.mnemonic.clone(),
                operands: insn.operands.clone(),
                bytes: insn.bytes.clone(),
            });
        }
    }
    snapshots
}

/// Compute the longest common subsequence length between two sequences of keys.
fn lcs_length(a: &[String], b: &[String]) -> usize {
    let m = a.len();
    let n = b.len();
    if m == 0 || n == 0 {
        return 0;
    }
    // Use two-row optimization to keep memory reasonable.
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = curr[j - 1].max(prev[j]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.iter_mut().for_each(|v| *v = 0);
    }
    prev[n]
}

/// Compute the full LCS table and back-trace to produce instruction diffs.
fn diff_instructions(
    old_insns: &[InstructionSnapshot],
    new_insns: &[InstructionSnapshot],
) -> Vec<InstructionDiff> {
    let old_keys: Vec<String> = old_insns
        .iter()
        .map(|i| insn_key(&i.mnemonic, &i.operands))
        .collect();
    let new_keys: Vec<String> = new_insns
        .iter()
        .map(|i| insn_key(&i.mnemonic, &i.operands))
        .collect();

    let m = old_keys.len();
    let n = new_keys.len();

    // Build full DP table for back-tracking.
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if old_keys[i - 1] == new_keys[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Back-track to produce diff entries.
    let mut diffs = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_keys[i - 1] == new_keys[j - 1] {
            let kind = if old_insns[i - 1].bytes == new_insns[j - 1].bytes {
                InstructionDiffKind::Same
            } else {
                InstructionDiffKind::Modified
            };
            diffs.push(InstructionDiff {
                kind,
                old_instruction: Some(old_insns[i - 1].clone()),
                new_instruction: Some(new_insns[j - 1].clone()),
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            diffs.push(InstructionDiff {
                kind: InstructionDiffKind::Added,
                old_instruction: None,
                new_instruction: Some(new_insns[j - 1].clone()),
            });
            j -= 1;
        } else {
            diffs.push(InstructionDiff {
                kind: InstructionDiffKind::Removed,
                old_instruction: Some(old_insns[i - 1].clone()),
                new_instruction: None,
            });
            i -= 1;
        }
    }
    diffs.reverse();
    diffs
}

/// Compute similarity (0.0–1.0) between two functions by comparing instruction keys.
fn function_similarity(old_func: &Function, new_func: &Function) -> f64 {
    let old_keys: Vec<String> = function_instructions(old_func)
        .iter()
        .map(|i| insn_key(&i.mnemonic, &i.operands))
        .collect();
    let new_keys: Vec<String> = function_instructions(new_func)
        .iter()
        .map(|i| insn_key(&i.mnemonic, &i.operands))
        .collect();

    let total = old_keys.len() + new_keys.len();
    if total == 0 {
        return 1.0;
    }
    let lcs = lcs_length(&old_keys, &new_keys);
    (2.0 * lcs as f64) / total as f64
}

/// Main entry point: compare two analysed binaries and produce a diff report.
pub fn compute_diff(
    old_analysis: &AnalysisDatabase,
    new_analysis: &AnalysisDatabase,
    old_image: &BinaryImage,
    new_image: &BinaryImage,
) -> DiffResult {
    let mut function_matches = Vec::new();
    let mut instruction_diffs: BTreeMap<String, Vec<InstructionDiff>> = BTreeMap::new();

    // Index new functions by name for fast lookup.
    let new_by_name: BTreeMap<&str, &Function> = new_analysis
        .functions
        .values()
        .map(|f| (f.name.as_str(), f))
        .collect();

    // Phase 1 – match by name.
    let mut matched_new_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for old_func in old_analysis.functions.values() {
        if let Some(&new_func) = new_by_name.get(old_func.name.as_str()) {
            let sim = function_similarity(old_func, new_func);
            let status = if (sim - 1.0).abs() < f64::EPSILON {
                FunctionMatchStatus::Matched
            } else {
                FunctionMatchStatus::Modified
            };
            function_matches.push(FunctionMatch {
                status,
                old_function: Some(old_func.name.clone()),
                new_function: Some(new_func.name.clone()),
                old_addr: Some(old_func.entry_addr),
                new_addr: Some(new_func.entry_addr),
                similarity: sim,
            });
            // Compute instruction-level diff for modified functions.
            let old_insns = function_instructions(old_func);
            let new_insns = function_instructions(new_func);
            instruction_diffs.insert(old_func.name.clone(), diff_instructions(&old_insns, &new_insns));
            matched_new_names.insert(new_func.name.clone());
        } else {
            function_matches.push(FunctionMatch {
                status: FunctionMatchStatus::Removed,
                old_function: Some(old_func.name.clone()),
                new_function: None,
                old_addr: Some(old_func.entry_addr),
                new_addr: None,
                similarity: 0.0,
            });
        }
    }

    // Phase 2 – find added functions.
    for new_func in new_analysis.functions.values() {
        if !matched_new_names.contains(&new_func.name) {
            function_matches.push(FunctionMatch {
                status: FunctionMatchStatus::Added,
                old_function: None,
                new_function: Some(new_func.name.clone()),
                old_addr: None,
                new_addr: Some(new_func.entry_addr),
                similarity: 0.0,
            });
        }
    }

    // Sort for stable display: Removed, Modified, Added, Matched.
    function_matches.sort_by(|a, b| {
        fn rank(s: &FunctionMatchStatus) -> u8 {
            match s {
                FunctionMatchStatus::Modified => 0,
                FunctionMatchStatus::Added => 1,
                FunctionMatchStatus::Removed => 2,
                FunctionMatchStatus::Matched => 3,
            }
        }
        rank(&a.status)
            .cmp(&rank(&b.status))
            .then_with(|| {
                let a_name = a.old_function.as_deref().or(a.new_function.as_deref()).unwrap_or("");
                let b_name = b.old_function.as_deref().or(b.new_function.as_deref()).unwrap_or("");
                a_name.cmp(b_name)
            })
    });

    DiffResult {
        old_filename: old_image.filename.clone(),
        new_filename: new_image.filename.clone(),
        function_matches,
        instruction_diffs,
    }
}

/// Produce a human-readable text report of the diff.
pub fn export_diff_report(result: &DiffResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Diff Report\n===========\nOld: {}\nNew: {}\n\n",
        result.old_filename, result.new_filename
    ));

    // Summary counts.
    let matched = result
        .function_matches
        .iter()
        .filter(|m| m.status == FunctionMatchStatus::Matched)
        .count();
    let modified = result
        .function_matches
        .iter()
        .filter(|m| m.status == FunctionMatchStatus::Modified)
        .count();
    let added = result
        .function_matches
        .iter()
        .filter(|m| m.status == FunctionMatchStatus::Added)
        .count();
    let removed = result
        .function_matches
        .iter()
        .filter(|m| m.status == FunctionMatchStatus::Removed)
        .count();
    out.push_str(&format!(
        "Summary: {} matched, {} modified, {} added, {} removed\n\n",
        matched, modified, added, removed
    ));

    // Function list.
    out.push_str("Functions\n---------\n");
    for m in &result.function_matches {
        let icon = match m.status {
            FunctionMatchStatus::Matched => "✓",
            FunctionMatchStatus::Added => "+",
            FunctionMatchStatus::Removed => "−",
            FunctionMatchStatus::Modified => "≠",
        };
        let name = m
            .old_function
            .as_deref()
            .or(m.new_function.as_deref())
            .unwrap_or("<unknown>");
        out.push_str(&format!(
            "  [{}] {} (similarity: {:.0}%)\n",
            icon,
            name,
            m.similarity * 100.0
        ));
    }
    out.push('\n');

    // Instruction diffs for modified functions.
    for m in &result.function_matches {
        if m.status != FunctionMatchStatus::Modified {
            continue;
        }
        let name = m
            .old_function
            .as_deref()
            .or(m.new_function.as_deref())
            .unwrap_or("<unknown>");
        if let Some(diffs) = result.instruction_diffs.get(name) {
            out.push_str(&format!("Function: {}\n", name));
            for d in diffs {
                let prefix = match d.kind {
                    InstructionDiffKind::Same => " ",
                    InstructionDiffKind::Added => "+",
                    InstructionDiffKind::Removed => "-",
                    InstructionDiffKind::Modified => "~",
                };
                match d.kind {
                    InstructionDiffKind::Same | InstructionDiffKind::Modified => {
                        if let (Some(old), Some(new)) =
                            (&d.old_instruction, &d.new_instruction)
                        {
                            out.push_str(&format!(
                                "  {} 0x{:08x}: {} {} | 0x{:08x}: {} {}\n",
                                prefix,
                                old.address,
                                old.mnemonic,
                                old.operands,
                                new.address,
                                new.mnemonic,
                                new.operands,
                            ));
                        }
                    }
                    InstructionDiffKind::Added => {
                        if let Some(new) = &d.new_instruction {
                            out.push_str(&format!(
                                "  {} {:>18} | 0x{:08x}: {} {}\n",
                                prefix, "", new.address, new.mnemonic, new.operands,
                            ));
                        }
                    }
                    InstructionDiffKind::Removed => {
                        if let Some(old) = &d.old_instruction {
                            out.push_str(&format!(
                                "  {} 0x{:08x}: {} {} |\n",
                                prefix, old.address, old.mnemonic, old.operands,
                            ));
                        }
                    }
                }
            }
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn make_empty_image(name: &str) -> BinaryImage {
        BinaryImage {
            filename: name.to_string(),
            format: BinaryFormat::Elf,
            architecture: Architecture::X86_64,
            entry_point: 0x1000,
            bits: 64,
            is_little_endian: true,
            sections: Vec::new(),
            segments: Vec::new(),
            symbols: Vec::new(),
            data: Vec::new(),
        }
    }

    fn make_instruction(addr: u64, mnemonic: &str, operands: &str) -> Instruction {
        Instruction {
            address: addr,
            size: 1,
            bytes: vec![0x90],
            mnemonic: mnemonic.to_string(),
            operands: operands.to_string(),
        }
    }

    fn make_function(name: &str, entry: u64, insns: Vec<Instruction>) -> Function {
        let block = BasicBlock {
            start_addr: entry,
            end_addr: insns.last().map_or(entry, |i| i.address + i.size as u64),
            instructions: insns,
            successors: Vec::new(),
            predecessors: Vec::new(),
        };
        Function {
            name: name.to_string(),
            entry_addr: entry,
            blocks: vec![block],
            xrefs_to: Vec::new(),
            xrefs_from: Vec::new(),
        }
    }

    fn make_analysis(functions: Vec<Function>) -> AnalysisDatabase {
        let mut db = AnalysisDatabase::new();
        for f in functions {
            db.functions.insert(f.entry_addr, f);
        }
        db
    }

    #[test]
    fn test_function_matching_by_name() {
        let old = make_analysis(vec![
            make_function("foo", 0x1000, vec![make_instruction(0x1000, "nop", "")]),
            make_function("bar", 0x2000, vec![make_instruction(0x2000, "ret", "")]),
        ]);
        let new = make_analysis(vec![
            make_function("foo", 0x3000, vec![make_instruction(0x3000, "nop", "")]),
            make_function("baz", 0x4000, vec![make_instruction(0x4000, "ret", "")]),
        ]);
        let old_img = make_empty_image("old.bin");
        let new_img = make_empty_image("new.bin");

        let result = compute_diff(&old, &new, &old_img, &new_img);

        // foo matched, bar removed, baz added
        let foo_match = result
            .function_matches
            .iter()
            .find(|m| m.old_function.as_deref() == Some("foo"))
            .unwrap();
        assert_eq!(foo_match.status, FunctionMatchStatus::Matched);
        assert!((foo_match.similarity - 1.0).abs() < f64::EPSILON);

        let bar_match = result
            .function_matches
            .iter()
            .find(|m| m.old_function.as_deref() == Some("bar"))
            .unwrap();
        assert_eq!(bar_match.status, FunctionMatchStatus::Removed);

        let baz_match = result
            .function_matches
            .iter()
            .find(|m| m.new_function.as_deref() == Some("baz"))
            .unwrap();
        assert_eq!(baz_match.status, FunctionMatchStatus::Added);
    }

    #[test]
    fn test_function_similarity() {
        let f1 = make_function(
            "f",
            0x1000,
            vec![
                make_instruction(0x1000, "push", "rbp"),
                make_instruction(0x1001, "mov", "rbp, rsp"),
                make_instruction(0x1004, "ret", ""),
            ],
        );
        let f2 = make_function(
            "f",
            0x2000,
            vec![
                make_instruction(0x2000, "push", "rbp"),
                make_instruction(0x2001, "mov", "rbp, rsp"),
                make_instruction(0x2004, "ret", ""),
            ],
        );
        let sim = function_similarity(&f1, &f2);
        assert!((sim - 1.0).abs() < f64::EPSILON, "identical functions should have similarity 1.0");

        let f3 = make_function(
            "f",
            0x3000,
            vec![
                make_instruction(0x3000, "push", "rbp"),
                make_instruction(0x3001, "xor", "eax, eax"),
                make_instruction(0x3003, "ret", ""),
            ],
        );
        let sim2 = function_similarity(&f1, &f3);
        assert!(sim2 > 0.0 && sim2 < 1.0, "partially matching functions should have 0 < sim < 1");
    }

    #[test]
    fn test_instruction_diff_same() {
        let insns = vec![
            InstructionSnapshot {
                address: 0x1000,
                mnemonic: "nop".to_string(),
                operands: String::new(),
                bytes: vec![0x90],
            },
        ];
        let diffs = diff_instructions(&insns, &insns);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].kind, InstructionDiffKind::Same);
    }

    #[test]
    fn test_instruction_diff_added_removed() {
        let old = vec![InstructionSnapshot {
            address: 0x1000,
            mnemonic: "nop".to_string(),
            operands: String::new(),
            bytes: vec![0x90],
        }];
        let new = vec![InstructionSnapshot {
            address: 0x2000,
            mnemonic: "ret".to_string(),
            operands: String::new(),
            bytes: vec![0xC3],
        }];
        let diffs = diff_instructions(&old, &new);
        assert_eq!(diffs.len(), 2);
        // One removed (nop) and one added (ret).
        let kinds: Vec<_> = diffs.iter().map(|d| d.kind).collect();
        assert!(kinds.contains(&InstructionDiffKind::Removed));
        assert!(kinds.contains(&InstructionDiffKind::Added));
    }

    #[test]
    fn test_instruction_diff_modified() {
        let old = vec![InstructionSnapshot {
            address: 0x1000,
            mnemonic: "mov".to_string(),
            operands: "eax, 0x1".to_string(),
            bytes: vec![0xB8, 0x01, 0x00, 0x00, 0x00],
        }];
        let new = vec![InstructionSnapshot {
            address: 0x2000,
            mnemonic: "mov".to_string(),
            operands: "eax, 0x1".to_string(),
            bytes: vec![0xB8, 0x01, 0x00, 0x00, 0x01], // different encoding
        }];
        let diffs = diff_instructions(&old, &new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].kind, InstructionDiffKind::Modified);
    }

    #[test]
    fn test_empty_analysis_diff() {
        let old = AnalysisDatabase::new();
        let new = AnalysisDatabase::new();
        let old_img = make_empty_image("a.bin");
        let new_img = make_empty_image("b.bin");

        let result = compute_diff(&old, &new, &old_img, &new_img);
        assert!(result.function_matches.is_empty());
        assert!(result.instruction_diffs.is_empty());
    }

    #[test]
    fn test_diff_report_export() {
        let old = make_analysis(vec![make_function(
            "main",
            0x1000,
            vec![
                make_instruction(0x1000, "push", "rbp"),
                make_instruction(0x1001, "ret", ""),
            ],
        )]);
        let new = make_analysis(vec![make_function(
            "main",
            0x2000,
            vec![
                make_instruction(0x2000, "push", "rbp"),
                make_instruction(0x2001, "nop", ""),
                make_instruction(0x2002, "ret", ""),
            ],
        )]);
        let old_img = make_empty_image("old.bin");
        let new_img = make_empty_image("new.bin");

        let result = compute_diff(&old, &new, &old_img, &new_img);
        let report = export_diff_report(&result);

        assert!(report.contains("Diff Report"));
        assert!(report.contains("old.bin"));
        assert!(report.contains("new.bin"));
        assert!(report.contains("main"));
        assert!(report.contains("modified"));
    }
}
