//! Pattern-matching decompiler: lifts x86/x64 instructions to C-like pseudo-code.
//!
//! Performs simple pattern matching on basic blocks and control flow graphs to
//! produce a readable pseudo-code representation of disassembled functions.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::analysis::AnalysisDatabase;
use crate::debug_info::DebugInfo;
use crate::model::{BasicBlock, Function, Instruction};

// ---------------------------------------------------------------------------
// IR types
// ---------------------------------------------------------------------------

/// An expression in the intermediate representation.
#[derive(Debug, Clone)]
pub enum IrExpr {
    /// A named variable or register.
    Var(String),
    /// A signed integer literal.
    Literal(i64),
    /// An unsigned hex literal.
    HexLiteral(u64),
    /// Binary operation: `left op right`.
    BinaryOp {
        op: String,
        left: Box<IrExpr>,
        right: Box<IrExpr>,
    },
    /// Unary operation: `op operand`.
    UnaryOp { op: String, operand: Box<IrExpr> },
    /// Pointer dereference: `*operand`.
    Deref(Box<IrExpr>),
    /// Function call: `name(args...)`.
    FunctionCall { name: String, args: Vec<IrExpr> },
}

/// A statement in the intermediate representation.
#[derive(Debug, Clone)]
pub enum IrStatement {
    /// `dst = src;`
    Assignment { dst: String, src: IrExpr },
    /// `if (condition) { then_body } else { else_body }`
    If {
        condition: IrExpr,
        then_body: Vec<IrStatement>,
        else_body: Vec<IrStatement>,
    },
    /// `while (condition) { body }`
    While {
        condition: IrExpr,
        body: Vec<IrStatement>,
    },
    /// `do { body } while (condition);`
    DoWhile {
        body: Vec<IrStatement>,
        condition: IrExpr,
    },
    /// `return expr;` or `return;`
    Return(Option<IrExpr>),
    /// Bare function call: `target(args...);`
    Call { target: String, args: Vec<IrExpr> },
    /// `goto label;`
    Goto(String),
    /// `label:`
    Label(String),
    /// `// comment`
    Comment(String),
    /// Fallback for unrecognised instructions.
    RawAsm(String),
}

/// A fully decompiled function.
#[derive(Debug, Clone)]
pub struct DecompiledFunction {
    /// Human-readable function name.
    pub name: String,
    /// C-like signature (e.g. `int main(int argc, char** argv)`).
    pub signature: String,
    /// Decompiled statement list.
    pub statements: Vec<IrStatement>,
    /// Maps statement index → original instruction address.
    pub address_map: BTreeMap<usize, u64>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return `true` if the mnemonic is a function prologue / epilogue
/// instruction that should be elided from pseudo-code.
fn is_prologue_epilogue(mnemonic: &str) -> bool {
    matches!(
        mnemonic,
        "push" | "pop" | "endbr64" | "endbr32" | "nop" | "leave"
    )
}

/// Return `true` if the mnemonic is an unconditional jump.
fn is_unconditional_jump(mnemonic: &str) -> bool {
    mnemonic == "jmp"
}

/// Return `true` if the mnemonic is a conditional jump.
fn is_conditional_jump(mnemonic: &str) -> bool {
    matches!(
        mnemonic,
        "je" | "jne"
            | "jz"
            | "jnz"
            | "jg"
            | "jge"
            | "jl"
            | "jle"
            | "ja"
            | "jae"
            | "jb"
            | "jbe"
            | "jo"
            | "jno"
            | "js"
            | "jns"
            | "jc"
            | "jnc"
    )
}

/// Convert a conditional-jump mnemonic to its C condition operator
/// (assuming the comparison set flags just before).
fn jcc_to_condition(mnemonic: &str, lhs: &str, rhs: &str) -> IrExpr {
    let op = match mnemonic {
        "je" | "jz" => "==",
        "jne" | "jnz" => "!=",
        "jg" => ">",
        "jge" => ">=",
        "jl" => "<",
        "jle" => "<=",
        "ja" => ">",  // unsigned
        "jae" => ">=", // unsigned
        "jb" => "<",   // unsigned
        "jbe" => "<=", // unsigned
        _ => "??",
    };
    IrExpr::BinaryOp {
        op: op.to_string(),
        left: Box::new(IrExpr::Var(lhs.to_string())),
        right: Box::new(parse_operand(rhs)),
    }
}

/// Try to parse an operand string into an `IrExpr`.
fn parse_operand(op: &str) -> IrExpr {
    let op = op.trim();
    if op.is_empty() {
        return IrExpr::Var(String::new());
    }

    // Hex immediate: 0x...
    if let Some(hex) = op.strip_prefix("0x") {
        if let Ok(v) = u64::from_str_radix(hex, 16) {
            return IrExpr::HexLiteral(v);
        }
    }

    // Negative hex: -0x...
    if let Some(rest) = op.strip_prefix("-0x") {
        if let Ok(v) = u64::from_str_radix(rest, 16) {
            return IrExpr::Literal(-(v as i64));
        }
    }

    // Plain decimal
    if let Ok(v) = op.parse::<i64>() {
        return IrExpr::Literal(v);
    }

    // Memory operand: [...] → dereference
    if op.starts_with('[') && op.ends_with(']') {
        let inner = &op[1..op.len() - 1];
        return IrExpr::Deref(Box::new(parse_operand(inner)));
    }

    // Size-prefixed memory: dword ptr [...], qword ptr [...], etc.
    for prefix in &[
        "byte ptr ",
        "word ptr ",
        "dword ptr ",
        "qword ptr ",
        "xmmword ptr ",
    ] {
        if let Some(rest) = op.strip_prefix(prefix) {
            return parse_operand(rest);
        }
    }

    IrExpr::Var(op.to_string())
}

/// Split a two-operand string like "rax, rbx" into `(dst, src)`.
fn split_operands(operands: &str) -> (&str, &str) {
    if let Some(pos) = operands.find(", ") {
        (&operands[..pos], &operands[pos + 2..])
    } else if let Some(pos) = operands.find(',') {
        (&operands[..pos], operands[pos + 1..].trim())
    } else {
        (operands, "")
    }
}

// ---------------------------------------------------------------------------
// Single-instruction lifting
// ---------------------------------------------------------------------------

/// Lift a single instruction into zero or more IR statements.
/// Returns `None` when the instruction should be silently skipped.
fn lift_instruction(insn: &Instruction) -> Option<IrStatement> {
    let mn = insn.mnemonic.as_str();
    let ops = insn.operands.as_str();

    // Skip prologue / epilogue noise
    if is_prologue_epilogue(mn) {
        return None;
    }

    // Skip unconditional jumps – handled at the CFG level
    if is_unconditional_jump(mn) {
        return None;
    }

    // Skip conditional jumps – handled at the CFG level
    if is_conditional_jump(mn) {
        return None;
    }

    // Skip compare/test – these set flags consumed by the following jcc
    if mn == "cmp" || mn == "test" {
        return None;
    }

    match mn {
        // --- MOV variants ---------------------------------------------------
        "mov" | "movzx" | "movsx" | "movsxd" | "movabs" | "cmove" | "cmovne" | "cmovg"
        | "cmovl" | "cmovge" | "cmovle" => {
            let (dst, src) = split_operands(ops);
            Some(IrStatement::Assignment {
                dst: dst.to_string(),
                src: parse_operand(src),
            })
        }

        // --- XOR reg, reg → zero --------------------------------------------
        "xor" => {
            let (dst, src) = split_operands(ops);
            if dst == src {
                Some(IrStatement::Assignment {
                    dst: dst.to_string(),
                    src: IrExpr::Literal(0),
                })
            } else {
                Some(IrStatement::Assignment {
                    dst: dst.to_string(),
                    src: IrExpr::BinaryOp {
                        op: "^".to_string(),
                        left: Box::new(IrExpr::Var(dst.to_string())),
                        right: Box::new(parse_operand(src)),
                    },
                })
            }
        }

        // --- Arithmetic / bitwise -------------------------------------------
        "add" => binop_stmt(ops, "+"),
        "sub" => binop_stmt(ops, "-"),
        "and" => binop_stmt(ops, "&"),
        "or" => binop_stmt(ops, "|"),
        "shl" | "sal" => binop_stmt(ops, "<<"),
        "shr" | "sar" => binop_stmt(ops, ">>"),
        "imul" => binop_stmt(ops, "*"),

        // --- INC / DEC ------------------------------------------------------
        "inc" => Some(IrStatement::Assignment {
            dst: ops.to_string(),
            src: IrExpr::BinaryOp {
                op: "+".to_string(),
                left: Box::new(IrExpr::Var(ops.to_string())),
                right: Box::new(IrExpr::Literal(1)),
            },
        }),
        "dec" => Some(IrStatement::Assignment {
            dst: ops.to_string(),
            src: IrExpr::BinaryOp {
                op: "-".to_string(),
                left: Box::new(IrExpr::Var(ops.to_string())),
                right: Box::new(IrExpr::Literal(1)),
            },
        }),

        // --- NOT / NEG ------------------------------------------------------
        "not" => Some(IrStatement::Assignment {
            dst: ops.to_string(),
            src: IrExpr::UnaryOp {
                op: "~".to_string(),
                operand: Box::new(IrExpr::Var(ops.to_string())),
            },
        }),
        "neg" => Some(IrStatement::Assignment {
            dst: ops.to_string(),
            src: IrExpr::UnaryOp {
                op: "-".to_string(),
                operand: Box::new(IrExpr::Var(ops.to_string())),
            },
        }),

        // --- LEA: load effective address ------------------------------------
        "lea" => {
            let (dst, src) = split_operands(ops);
            Some(IrStatement::Assignment {
                dst: dst.to_string(),
                src: parse_operand(src),
            })
        }

        // --- CALL -----------------------------------------------------------
        "call" => {
            let target = ops.trim().to_string();
            Some(IrStatement::Call {
                target,
                args: vec![],
            })
        }

        // --- RET ------------------------------------------------------------
        "ret" | "retn" => Some(IrStatement::Return(None)),

        // --- Everything else ------------------------------------------------
        _ => Some(IrStatement::RawAsm(format!("{} {}", mn, ops))),
    }
}

/// Helper: create an assignment of `dst = dst op src`.
fn binop_stmt(ops: &str, op: &str) -> Option<IrStatement> {
    let (dst, src) = split_operands(ops);
    Some(IrStatement::Assignment {
        dst: dst.to_string(),
        src: IrExpr::BinaryOp {
            op: op.to_string(),
            left: Box::new(IrExpr::Var(dst.to_string())),
            right: Box::new(parse_operand(src)),
        },
    })
}

// ---------------------------------------------------------------------------
// Control-flow structuring
// ---------------------------------------------------------------------------

/// Detect back-edges (loops) in the CFG.
/// Returns a set of (source_block_addr, target_block_addr) pairs where the
/// target dominates the source (i.e. the edge goes "back" in the CFG).
fn detect_back_edges(func: &Function) -> BTreeSet<(u64, u64)> {
    let mut back_edges = BTreeSet::new();
    if func.blocks.is_empty() {
        return back_edges;
    }

    // Build block address → index map
    let block_index: HashMap<u64, usize> = func
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.start_addr, i))
        .collect();

    // Simple DFS to detect back-edges
    let mut visited = vec![false; func.blocks.len()];
    let mut on_stack = vec![false; func.blocks.len()];

    fn dfs(
        idx: usize,
        blocks: &[BasicBlock],
        block_index: &HashMap<u64, usize>,
        visited: &mut [bool],
        on_stack: &mut [bool],
        back_edges: &mut BTreeSet<(u64, u64)>,
    ) {
        visited[idx] = true;
        on_stack[idx] = true;
        for &succ_addr in &blocks[idx].successors {
            if let Some(&succ_idx) = block_index.get(&succ_addr) {
                if !visited[succ_idx] {
                    dfs(succ_idx, blocks, block_index, visited, on_stack, back_edges);
                } else if on_stack[succ_idx] {
                    back_edges.insert((blocks[idx].start_addr, succ_addr));
                }
            }
        }
        on_stack[idx] = false;
    }

    if let Some(&entry_idx) = block_index.get(&func.entry_addr) {
        dfs(
            entry_idx,
            &func.blocks,
            &block_index,
            &mut visited,
            &mut on_stack,
            &mut back_edges,
        );
    }

    back_edges
}

/// Try to extract a `cmp`/`test` + `jcc` condition from the tail of a block.
fn extract_condition(block: &BasicBlock) -> Option<(IrExpr, &str)> {
    let insns = &block.instructions;
    if insns.len() < 2 {
        return None;
    }
    let last = &insns[insns.len() - 1];
    let prev = &insns[insns.len() - 2];

    if !is_conditional_jump(&last.mnemonic) {
        return None;
    }

    let jcc = last.mnemonic.as_str();

    match prev.mnemonic.as_str() {
        "cmp" => {
            let (lhs, rhs) = split_operands(&prev.operands);
            Some((jcc_to_condition(jcc, lhs, rhs), jcc))
        }
        "test" => {
            let (lhs, rhs) = split_operands(&prev.operands);
            if lhs == rhs {
                // `test reg, reg` followed by jz/jnz
                let cond = match jcc {
                    "jz" | "je" => IrExpr::BinaryOp {
                        op: "==".to_string(),
                        left: Box::new(IrExpr::Var(lhs.to_string())),
                        right: Box::new(IrExpr::Literal(0)),
                    },
                    "jnz" | "jne" => IrExpr::BinaryOp {
                        op: "!=".to_string(),
                        left: Box::new(IrExpr::Var(lhs.to_string())),
                        right: Box::new(IrExpr::Literal(0)),
                    },
                    _ => IrExpr::Var(format!("flags(test {}, {})", lhs, rhs)),
                };
                Some((cond, jcc))
            } else {
                let cond = IrExpr::BinaryOp {
                    op: "&".to_string(),
                    left: Box::new(IrExpr::Var(lhs.to_string())),
                    right: Box::new(parse_operand(rhs)),
                };
                Some((cond, jcc))
            }
        }
        _ => {
            // No recognized flag-setting instruction; emit generic condition
            Some((
                IrExpr::Var(format!("cond_{}", jcc)),
                jcc,
            ))
        }
    }
}

/// Lift a basic block's instructions into a list of IR statements.
fn lift_block(block: &BasicBlock) -> Vec<IrStatement> {
    let mut stmts = Vec::new();
    for insn in &block.instructions {
        if let Some(stmt) = lift_instruction(insn) {
            stmts.push(stmt);
        }
    }
    stmts
}

// ---------------------------------------------------------------------------
// Top-level decompilation
// ---------------------------------------------------------------------------

/// Decompile a single function into a `DecompiledFunction`.
///
/// Uses simple pattern matching on instructions and CFG structure to
/// produce C-like pseudo-code with `if`/`while` constructs where possible.
pub fn decompile_function(
    func: &Function,
    _analysis: &AnalysisDatabase,
    debug_info: &DebugInfo,
) -> DecompiledFunction {
    // Build signature from debug info or fallback
    let debug_func = debug_info.function_at(func.entry_addr);
    let signature = if let Some(df) = debug_func {
        df.signature
            .clone()
            .unwrap_or_else(|| format!("void {}()", func.name))
    } else {
        format!("void {}()", func.name)
    };

    if func.blocks.is_empty() {
        return DecompiledFunction {
            name: func.name.clone(),
            signature,
            statements: vec![IrStatement::Comment("empty function".to_string())],
            address_map: BTreeMap::new(),
        };
    }

    let back_edges = detect_back_edges(func);
    let block_map: HashMap<u64, &BasicBlock> = func
        .blocks
        .iter()
        .map(|b| (b.start_addr, b))
        .collect();

    let mut statements: Vec<IrStatement> = Vec::new();
    let mut address_map: BTreeMap<usize, u64> = BTreeMap::new();
    let mut visited: BTreeSet<u64> = BTreeSet::new();

    // Walk blocks in address order to produce linear output.
    let mut ordered_addrs: Vec<u64> = func.blocks.iter().map(|b| b.start_addr).collect();
    ordered_addrs.sort();

    for &block_addr in &ordered_addrs {
        let block = match block_map.get(&block_addr) {
            Some(b) => b,
            None => continue,
        };

        if visited.contains(&block_addr) {
            continue;
        }
        visited.insert(block_addr);

        // Check if this block is a loop header (target of a back-edge)
        let is_loop_header = back_edges.iter().any(|&(_, target)| target == block_addr);

        if is_loop_header {
            // Emit while loop
            let condition = extract_condition(block)
                .map(|(c, _)| c)
                .unwrap_or_else(|| IrExpr::Literal(1));

            let body_stmts = lift_block(block);

            // Record addresses for the block instructions
            let base_idx = statements.len();
            if let Some(first_insn) = block.instructions.first() {
                address_map.insert(base_idx, first_insn.address);
            }

            statements.push(IrStatement::While {
                condition,
                body: body_stmts,
            });
            continue;
        }

        // Check for if/else pattern: block ends with conditional jump
        if let Some((condition, _jcc)) = extract_condition(block) {
            // Emit the body of this block (excluding cmp+jcc)
            let block_body = lift_block(block);
            for stmt in &block_body {
                let idx = statements.len();
                if let Some(first_insn) = block.instructions.first() {
                    address_map.insert(idx, first_insn.address);
                }
                statements.push(stmt.clone());
            }

            // The "then" branch is the fall-through successor
            // The "else" branch is the jump target
            let then_stmts: Vec<IrStatement> = if block.successors.len() >= 2 {
                let fall_through = block.successors[0];
                if let Some(tb) = block_map.get(&fall_through) {
                    visited.insert(fall_through);
                    lift_block(tb)
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            let else_stmts: Vec<IrStatement> = if block.successors.len() >= 2 {
                let jump_target = block.successors[1];
                if !visited.contains(&jump_target) && !back_edges.iter().any(|&(_, t)| t == jump_target) {
                    if let Some(eb) = block_map.get(&jump_target) {
                        visited.insert(jump_target);
                        lift_block(eb)
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            let idx = statements.len();
            if let Some(last_insn) = block.instructions.last() {
                address_map.insert(idx, last_insn.address);
            }
            statements.push(IrStatement::If {
                condition,
                then_body: then_stmts,
                else_body: else_stmts,
            });

            continue;
        }

        // Plain block – just lift instructions
        for insn in &block.instructions {
            if let Some(stmt) = lift_instruction(insn) {
                let idx = statements.len();
                address_map.insert(idx, insn.address);
                statements.push(stmt);
            }
        }
    }

    DecompiledFunction {
        name: func.name.clone(),
        signature,
        statements,
        address_map,
    }
}

// ---------------------------------------------------------------------------
// Pseudo-code renderer
// ---------------------------------------------------------------------------

/// Render an `IrExpr` to a C-like string.
fn render_expr(expr: &IrExpr) -> String {
    match expr {
        IrExpr::Var(name) => name.clone(),
        IrExpr::Literal(v) => v.to_string(),
        IrExpr::HexLiteral(v) => format!("0x{:x}", v),
        IrExpr::BinaryOp { op, left, right } => {
            format!("({} {} {})", render_expr(left), op, render_expr(right))
        }
        IrExpr::UnaryOp { op, operand } => {
            format!("{}({})", op, render_expr(operand))
        }
        IrExpr::Deref(inner) => format!("*({})", render_expr(inner)),
        IrExpr::FunctionCall { name, args } => {
            let arg_strs: Vec<String> = args.iter().map(render_expr).collect();
            format!("{}({})", name, arg_strs.join(", "))
        }
    }
}

/// Render statements at a given indentation level.
fn render_statements(stmts: &[IrStatement], indent: usize) -> String {
    let mut out = String::new();
    let pad = "    ".repeat(indent);
    for stmt in stmts {
        match stmt {
            IrStatement::Assignment { dst, src } => {
                out.push_str(&format!("{}{} = {};\n", pad, dst, render_expr(src)));
            }
            IrStatement::If {
                condition,
                then_body,
                else_body,
            } => {
                out.push_str(&format!("{}if ({}) {{\n", pad, render_expr(condition)));
                out.push_str(&render_statements(then_body, indent + 1));
                if !else_body.is_empty() {
                    out.push_str(&format!("{}}} else {{\n", pad));
                    out.push_str(&render_statements(else_body, indent + 1));
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            IrStatement::While { condition, body } => {
                out.push_str(&format!("{}while ({}) {{\n", pad, render_expr(condition)));
                out.push_str(&render_statements(body, indent + 1));
                out.push_str(&format!("{}}}\n", pad));
            }
            IrStatement::DoWhile { body, condition } => {
                out.push_str(&format!("{}do {{\n", pad));
                out.push_str(&render_statements(body, indent + 1));
                out.push_str(&format!("{}}} while ({});\n", pad, render_expr(condition)));
            }
            IrStatement::Return(None) => {
                out.push_str(&format!("{}return;\n", pad));
            }
            IrStatement::Return(Some(expr)) => {
                out.push_str(&format!("{}return {};\n", pad, render_expr(expr)));
            }
            IrStatement::Call { target, args } => {
                let arg_strs: Vec<String> = args.iter().map(render_expr).collect();
                out.push_str(&format!("{}{}({});\n", pad, target, arg_strs.join(", ")));
            }
            IrStatement::Goto(label) => {
                out.push_str(&format!("{}goto {};\n", pad, label));
            }
            IrStatement::Label(label) => {
                out.push_str(&format!("{}:\n", label));
            }
            IrStatement::Comment(text) => {
                out.push_str(&format!("{}// {}\n", pad, text));
            }
            IrStatement::RawAsm(asm) => {
                out.push_str(&format!("{}__asm(\"{}\");\n", pad, asm));
            }
        }
    }
    out
}

/// Generate C-like pseudo-code from a decompiled function.
pub fn generate_pseudo_code(func: &DecompiledFunction) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} {{\n", func.signature));
    out.push_str(&render_statements(&func.statements, 1));
    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BasicBlock, Function, Instruction};

    /// Helper: build an Instruction.
    fn insn(addr: u64, mnemonic: &str, operands: &str) -> Instruction {
        Instruction {
            address: addr,
            size: 1,
            bytes: vec![0x90],
            mnemonic: mnemonic.to_string(),
            operands: operands.to_string(),
        }
    }

    /// Helper: build a simple one-block function.
    fn one_block_func(name: &str, addr: u64, instructions: Vec<Instruction>) -> Function {
        let end = instructions
            .last()
            .map(|i| i.address + i.size as u64)
            .unwrap_or(addr);
        Function {
            name: name.to_string(),
            entry_addr: addr,
            blocks: vec![BasicBlock {
                start_addr: addr,
                end_addr: end,
                instructions,
                successors: vec![],
                predecessors: vec![],
            }],
            xrefs_to: vec![],
            xrefs_from: vec![],
        }
    }

    #[test]
    fn test_mov_ret() {
        let func = one_block_func(
            "simple",
            0x1000,
            vec![
                insn(0x1000, "mov", "eax, 0x0"),
                insn(0x1003, "ret", ""),
            ],
        );
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        assert_eq!(result.name, "simple");
        assert!(result.signature.contains("simple"));
        assert!(result.statements.len() >= 2);

        // First statement: assignment
        match &result.statements[0] {
            IrStatement::Assignment { dst, .. } => assert_eq!(dst, "eax"),
            other => panic!("expected Assignment, got {:?}", other),
        }
        // Second statement: return
        assert!(matches!(result.statements[1], IrStatement::Return(None)));
    }

    #[test]
    fn test_xor_reg_reg_zero() {
        let func = one_block_func(
            "zero",
            0x2000,
            vec![insn(0x2000, "xor", "eax, eax"), insn(0x2002, "ret", "")],
        );
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        match &result.statements[0] {
            IrStatement::Assignment { dst, src } => {
                assert_eq!(dst, "eax");
                match src {
                    IrExpr::Literal(0) => {} // correct
                    other => panic!("expected Literal(0), got {:?}", other),
                }
            }
            other => panic!("expected Assignment, got {:?}", other),
        }
    }

    #[test]
    fn test_call_pattern() {
        let func = one_block_func(
            "caller",
            0x3000,
            vec![
                insn(0x3000, "call", "printf"),
                insn(0x3005, "ret", ""),
            ],
        );
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        match &result.statements[0] {
            IrStatement::Call { target, .. } => assert_eq!(target, "printf"),
            other => panic!("expected Call, got {:?}", other),
        }
    }

    #[test]
    fn test_if_else_pattern() {
        // Block 0: cmp eax, 0 ; je block_2
        // Block 1 (fall-through / then): mov ebx, 1 ; ret
        // Block 2 (jump target / else): mov ebx, 2 ; ret
        let func = Function {
            name: "if_else".to_string(),
            entry_addr: 0x4000,
            blocks: vec![
                BasicBlock {
                    start_addr: 0x4000,
                    end_addr: 0x4006,
                    instructions: vec![
                        insn(0x4000, "cmp", "eax, 0x0"),
                        insn(0x4003, "je", "0x4010"),
                    ],
                    successors: vec![0x4006, 0x4010],
                    predecessors: vec![],
                },
                BasicBlock {
                    start_addr: 0x4006,
                    end_addr: 0x400A,
                    instructions: vec![
                        insn(0x4006, "mov", "ebx, 0x1"),
                        insn(0x4009, "ret", ""),
                    ],
                    successors: vec![],
                    predecessors: vec![0x4000],
                },
                BasicBlock {
                    start_addr: 0x4010,
                    end_addr: 0x4014,
                    instructions: vec![
                        insn(0x4010, "mov", "ebx, 0x2"),
                        insn(0x4013, "ret", ""),
                    ],
                    successors: vec![],
                    predecessors: vec![0x4000],
                },
            ],
            xrefs_to: vec![],
            xrefs_from: vec![],
        };
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        // Should contain an If statement
        let has_if = result
            .statements
            .iter()
            .any(|s| matches!(s, IrStatement::If { .. }));
        assert!(has_if, "expected an If statement in: {:?}", result.statements);
    }

    #[test]
    fn test_pseudo_code_generation() {
        let func = one_block_func(
            "gen",
            0x5000,
            vec![
                insn(0x5000, "mov", "eax, 0x1"),
                insn(0x5003, "ret", ""),
            ],
        );
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let decomp = decompile_function(&func, &db, &di);
        let code = generate_pseudo_code(&decomp);

        assert!(code.contains("gen"), "signature should contain function name");
        assert!(code.contains("eax = "), "should contain assignment");
        assert!(code.contains("return;"), "should contain return");
        assert!(code.contains('{'), "should contain opening brace");
        assert!(code.contains('}'), "should contain closing brace");
    }

    #[test]
    fn test_empty_function() {
        let func = Function {
            name: "empty".to_string(),
            entry_addr: 0x6000,
            blocks: vec![],
            xrefs_to: vec![],
            xrefs_from: vec![],
        };
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        assert_eq!(result.name, "empty");
        assert!(!result.statements.is_empty());
        match &result.statements[0] {
            IrStatement::Comment(text) => assert!(text.contains("empty")),
            other => panic!("expected Comment, got {:?}", other),
        }
    }

    #[test]
    fn test_prologue_filtering() {
        let func = one_block_func(
            "prologue",
            0x7000,
            vec![
                insn(0x7000, "push", "rbp"),
                insn(0x7001, "nop", ""),
                insn(0x7002, "mov", "eax, 0x0"),
                insn(0x7005, "pop", "rbp"),
                insn(0x7006, "ret", ""),
            ],
        );
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        // push/nop/pop should be filtered; only mov and ret remain
        assert_eq!(result.statements.len(), 2);
        assert!(matches!(&result.statements[0], IrStatement::Assignment { .. }));
        assert!(matches!(&result.statements[1], IrStatement::Return(None)));
    }

    #[test]
    fn test_arithmetic_ops() {
        let func = one_block_func(
            "arith",
            0x8000,
            vec![
                insn(0x8000, "add", "eax, 0x5"),
                insn(0x8003, "sub", "ebx, ecx"),
                insn(0x8005, "ret", ""),
            ],
        );
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        let code = generate_pseudo_code(&result);
        assert!(code.contains('+'), "should contain + for add");
        assert!(code.contains('-'), "should contain - for sub");
    }

    #[test]
    fn test_test_reg_reg_jz() {
        // test eax, eax ; jz target → if (eax == 0)
        let func = Function {
            name: "test_jz".to_string(),
            entry_addr: 0x9000,
            blocks: vec![
                BasicBlock {
                    start_addr: 0x9000,
                    end_addr: 0x9006,
                    instructions: vec![
                        insn(0x9000, "test", "eax, eax"),
                        insn(0x9002, "jz", "0x9010"),
                    ],
                    successors: vec![0x9006, 0x9010],
                    predecessors: vec![],
                },
                BasicBlock {
                    start_addr: 0x9006,
                    end_addr: 0x900A,
                    instructions: vec![insn(0x9006, "ret", "")],
                    successors: vec![],
                    predecessors: vec![0x9000],
                },
                BasicBlock {
                    start_addr: 0x9010,
                    end_addr: 0x9014,
                    instructions: vec![insn(0x9010, "ret", "")],
                    successors: vec![],
                    predecessors: vec![0x9000],
                },
            ],
            xrefs_to: vec![],
            xrefs_from: vec![],
        };
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);
        let code = generate_pseudo_code(&result);
        assert!(
            code.contains("== 0"),
            "should contain '== 0' for test reg, reg + jz, got:\n{}",
            code
        );
    }

    #[test]
    fn test_address_map_populated() {
        let func = one_block_func(
            "mapped",
            0xA000,
            vec![
                insn(0xA000, "mov", "eax, 0x1"),
                insn(0xA003, "ret", ""),
            ],
        );
        let db = AnalysisDatabase::new();
        let di = DebugInfo::default();
        let result = decompile_function(&func, &db, &di);

        assert!(
            !result.address_map.is_empty(),
            "address_map should not be empty"
        );
        // All mapped addresses should be within the function
        for &addr in result.address_map.values() {
            assert!((0xA000..=0xA003).contains(&addr));
        }
    }
}
