//! Rhai scripting engine for automating analysis tasks (Sprint 13).

use kiln_core::analysis::AnalysisDatabase;
use rhai::{Dynamic, Engine, ImmutableString, Scope, AST};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

/// Kind of output produced by a script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputKind {
    Normal,
    Error,
    Info,
}

/// A single line of script output with its classification.
#[derive(Debug, Clone)]
pub struct OutputLine {
    pub text: String,
    pub kind: OutputKind,
}

/// Result of executing a script.
#[derive(Debug, Clone, Default)]
pub struct ScriptResult {
    pub output: Vec<OutputLine>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct InstructionInfo {
    address: u64,
    mnemonic: String,
    operands: String,
    size: i64,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct XrefInfo {
    from_addr: u64,
    to_addr: u64,
    xref_type: String,
}

#[derive(Debug, Clone)]
struct FunctionInfo {
    name: String,
    entry_addr: u64,
    block_count: i64,
    instruction_count: i64,
}

/// Execute a script string against the current analysis and project.
/// Mutations (set_comment, set_label) are applied after the script finishes.
pub fn execute_script(
    code: &str,
    analysis: &AnalysisDatabase,
    project: &mut kiln_project::Project,
) -> ScriptResult {
    let output: Rc<RefCell<Vec<OutputLine>>> = Rc::new(RefCell::new(Vec::new()));
    let pending_comments: Rc<RefCell<Vec<(u64, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let pending_labels: Rc<RefCell<Vec<(u64, String)>>> = Rc::new(RefCell::new(Vec::new()));

    let mut engine = Engine::new();
    engine.set_max_operations(1_000_000);

    register_print_functions(&mut engine, Rc::clone(&output));
    register_instruction_api(&mut engine, analysis);
    register_xref_api(&mut engine, analysis);
    register_annotation_read_api(&mut engine, project);

    let pc = Rc::clone(&pending_comments);
    engine.register_fn("set_comment", move |addr: i64, text: ImmutableString| {
        pc.borrow_mut().push((addr as u64, text.to_string()));
    });
    let pl = Rc::clone(&pending_labels);
    engine.register_fn("set_label", move |addr: i64, name: ImmutableString| {
        pl.borrow_mut().push((addr as u64, name.to_string()));
    });

    register_function_api(&mut engine, analysis);
    engine.register_fn("to_hex", |val: i64| -> String {
        format!("0x{:x}", val)
    });

    let mut scope = Scope::new();
    let result = engine.run_with_scope(&mut scope, code);

    // Apply deferred mutations
    for (addr, text) in pending_comments.borrow().iter() {
        project.set_comment(*addr, text.clone());
    }
    for (addr, name) in pending_labels.borrow().iter() {
        project.set_label(*addr, name.clone());
    }

    let output = Rc::try_unwrap(output)
        .map(|cell| cell.into_inner())
        .unwrap_or_else(|rc| rc.borrow().clone());

    match result {
        Ok(()) => ScriptResult {
            output,
            error: None,
        },
        Err(e) => {
            let mut output = output;
            output.push(OutputLine {
                text: format!("Error: {e}"),
                kind: OutputKind::Error,
            });
            ScriptResult {
                output,
                error: Some(format!("{e}")),
            }
        }
    }
}

/// Execute a script from a file path.
pub fn execute_file(
    path: &Path,
    analysis: &AnalysisDatabase,
    project: &mut kiln_project::Project,
) -> ScriptResult {
    match std::fs::read_to_string(path) {
        Ok(code) => execute_script(&code, analysis, project),
        Err(e) => ScriptResult {
            output: vec![OutputLine {
                text: format!("Failed to read script file: {e}"),
                kind: OutputKind::Error,
            }],
            error: Some(format!("Failed to read script file: {e}")),
        },
    }
}

/// Compile a script without executing it, to check for syntax errors.
#[allow(dead_code)]
pub fn check_syntax(code: &str) -> Result<AST, String> {
    let engine = Engine::new();
    engine.compile(code).map_err(|e| format!("{e}"))
}

fn register_print_functions(engine: &mut Engine, output: Rc<RefCell<Vec<OutputLine>>>) {
    // Use Rhai's on_print callback instead of register_fn to avoid
    // conflicting with the built-in print statement semantics.
    let out = Rc::clone(&output);
    engine.on_print(move |text| {
        out.borrow_mut().push(OutputLine {
            text: text.to_string(),
            kind: OutputKind::Normal,
        });
    });

    let out = Rc::clone(&output);
    engine.on_debug(move |text, _source, _pos| {
        out.borrow_mut().push(OutputLine {
            text: text.to_string(),
            kind: OutputKind::Info,
        });
    });
}

fn register_instruction_api(engine: &mut Engine, analysis: &AnalysisDatabase) {
    let insn_map: std::collections::BTreeMap<u64, InstructionInfo> = analysis
        .instructions
        .iter()
        .map(|(&addr, insn)| {
            (addr, InstructionInfo {
                address: insn.address,
                mnemonic: insn.mnemonic.clone(),
                operands: insn.operands.clone(),
                size: insn.size as i64,
                bytes: insn.bytes.clone(),
            })
        })
        .collect();

    let insns = insn_map.clone();
    engine.register_fn("get_instruction", move |addr: i64| -> Dynamic {
        match insns.get(&(addr as u64)) {
            Some(info) => instruction_to_dynamic(info),
            None => Dynamic::UNIT,
        }
    });

    engine.register_fn("get_instructions_in_range", move |start: i64, end: i64| -> rhai::Array {
        insn_map
            .range(start as u64..end as u64)
            .map(|(_, info)| instruction_to_dynamic(info))
            .collect()
    });
}

fn instruction_to_dynamic(info: &InstructionInfo) -> Dynamic {
    let mut map = rhai::Map::new();
    map.insert("address".into(), Dynamic::from(info.address as i64));
    map.insert("mnemonic".into(), Dynamic::from(info.mnemonic.clone()));
    map.insert("operands".into(), Dynamic::from(info.operands.clone()));
    map.insert("size".into(), Dynamic::from(info.size));
    let bytes_str: String = info.bytes.iter().map(|b| format!("{b:02x}")).collect();
    map.insert("bytes".into(), Dynamic::from(bytes_str));
    Dynamic::from(map)
}

fn register_xref_api(engine: &mut Engine, analysis: &AnalysisDatabase) {
    let mut xrefs_to: std::collections::BTreeMap<u64, Vec<XrefInfo>> =
        std::collections::BTreeMap::new();
    let mut xrefs_from: std::collections::BTreeMap<u64, Vec<XrefInfo>> =
        std::collections::BTreeMap::new();
    for xref in &analysis.xrefs {
        let info = XrefInfo {
            from_addr: xref.from_addr,
            to_addr: xref.to_addr,
            xref_type: format!("{:?}", xref.xref_type),
        };
        xrefs_to.entry(xref.to_addr).or_default().push(info.clone());
        xrefs_from.entry(xref.from_addr).or_default().push(info);
    }

    let xt = xrefs_to;
    engine.register_fn("get_xrefs_to", move |addr: i64| -> rhai::Array {
        xref_list_to_array(xt.get(&(addr as u64)))
    });
    let xf = xrefs_from;
    engine.register_fn("get_xrefs_from", move |addr: i64| -> rhai::Array {
        xref_list_to_array(xf.get(&(addr as u64)))
    });
}

fn xref_list_to_array(xrefs: Option<&Vec<XrefInfo>>) -> rhai::Array {
    match xrefs {
        Some(xrefs) => xrefs
            .iter()
            .map(|x| {
                let mut map = rhai::Map::new();
                map.insert("from_addr".into(), Dynamic::from(x.from_addr as i64));
                map.insert("to_addr".into(), Dynamic::from(x.to_addr as i64));
                map.insert("xref_type".into(), Dynamic::from(x.xref_type.clone()));
                Dynamic::from(map)
            })
            .collect(),
        None => rhai::Array::new(),
    }
}

fn register_annotation_read_api(engine: &mut Engine, project: &kiln_project::Project) {
    let comments: std::collections::BTreeMap<u64, String> = project
        .annotations
        .iter()
        .filter_map(|(&addr, ann)| ann.comment.as_ref().map(|c| (addr, c.clone())))
        .collect();
    let labels: std::collections::BTreeMap<u64, String> = project
        .annotations
        .iter()
        .filter_map(|(&addr, ann)| ann.label.as_ref().map(|l| (addr, l.clone())))
        .collect();

    engine.register_fn("get_comment", move |addr: i64| -> Dynamic {
        match comments.get(&(addr as u64)) {
            Some(c) => Dynamic::from(c.clone()),
            None => Dynamic::UNIT,
        }
    });
    engine.register_fn("get_label", move |addr: i64| -> Dynamic {
        match labels.get(&(addr as u64)) {
            Some(l) => Dynamic::from(l.clone()),
            None => Dynamic::UNIT,
        }
    });
}

fn register_function_api(engine: &mut Engine, analysis: &AnalysisDatabase) {
    let func_infos: Vec<FunctionInfo> = analysis
        .functions
        .values()
        .map(|f| {
            let insn_count: usize = f.blocks.iter().map(|b| b.instructions.len()).sum();
            FunctionInfo {
                name: f.name.clone(),
                entry_addr: f.entry_addr,
                block_count: f.blocks.len() as i64,
                instruction_count: insn_count as i64,
            }
        })
        .collect();

    let func_map: std::collections::BTreeMap<u64, FunctionInfo> = func_infos
        .iter()
        .map(|f| (f.entry_addr, f.clone()))
        .collect();

    engine.register_fn("get_function", move |addr: i64| -> Dynamic {
        match func_map.get(&(addr as u64)) {
            Some(info) => function_to_dynamic(info),
            None => Dynamic::UNIT,
        }
    });
    engine.register_fn("get_functions", move || -> rhai::Array {
        func_infos.iter().map(|info| function_to_dynamic(info)).collect()
    });
}

fn function_to_dynamic(info: &FunctionInfo) -> Dynamic {
    let mut map = rhai::Map::new();
    map.insert("name".into(), Dynamic::from(info.name.clone()));
    map.insert("entry_addr".into(), Dynamic::from(info.entry_addr as i64));
    map.insert("block_count".into(), Dynamic::from(info.block_count));
    map.insert("instruction_count".into(), Dynamic::from(info.instruction_count));
    Dynamic::from(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::analysis::AnalysisDatabase;

    fn empty_analysis() -> AnalysisDatabase {
        AnalysisDatabase::new()
    }

    fn empty_project() -> kiln_project::Project {
        kiln_project::Project::new(String::new())
    }

    #[test]
    fn test_print_output() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script(r#"print("hello world");"#, &analysis, &mut project);
        assert!(result.error.is_none());
        assert_eq!(result.output.len(), 1);
        assert_eq!(result.output[0].text, "hello world");
        assert_eq!(result.output[0].kind, OutputKind::Normal);
    }

    #[test]
    fn test_print_integer() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script("print(42);", &analysis, &mut project);
        assert!(result.error.is_none());
        assert_eq!(result.output[0].text, "42");
    }

    #[test]
    fn test_syntax_error() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script("let x = ;", &analysis, &mut project);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_get_functions_empty() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script(
            r#"let funcs = get_functions(); print(funcs.len());"#,
            &analysis, &mut project,
        );
        assert!(result.error.is_none());
        assert_eq!(result.output[0].text, "0");
    }

    #[test]
    fn test_get_instruction_not_found() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script(
            r#"let insn = get_instruction(0x401000); if insn == () { print("not found"); }"#,
            &analysis, &mut project,
        );
        assert!(result.error.is_none());
        assert_eq!(result.output[0].text, "not found");
    }

    #[test]
    fn test_set_comment_deferred() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script(
            r#"set_comment(0x1000, "test comment");"#,
            &analysis, &mut project,
        );
        assert!(result.error.is_none());
        assert_eq!(project.get_comment(0x1000), Some("test comment"));
    }

    #[test]
    fn test_set_label_deferred() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script(
            r#"set_label(0x2000, "my_func");"#,
            &analysis, &mut project,
        );
        assert!(result.error.is_none());
        assert_eq!(project.get_label(0x2000), Some("my_func"));
    }

    #[test]
    fn test_to_hex() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script(r#"print(to_hex(0x401000));"#, &analysis, &mut project);
        assert!(result.error.is_none());
        assert_eq!(result.output[0].text, "0x401000");
    }

    #[test]
    fn test_xrefs_empty() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script(
            r#"let xrefs = get_xrefs_to(0x401000); print(xrefs.len());"#,
            &analysis, &mut project,
        );
        assert!(result.error.is_none());
        assert_eq!(result.output[0].text, "0");
    }

    #[test]
    fn test_check_syntax_valid() {
        assert!(check_syntax(r#"let x = 42; print(x);"#).is_ok());
    }

    #[test]
    fn test_check_syntax_invalid() {
        assert!(check_syntax("let x = ;").is_err());
    }

    #[test]
    fn test_get_comment_existing() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        project.set_comment(0x3000, "existing comment".to_string());
        let result = execute_script(
            r#"let c = get_comment(0x3000); print(c);"#,
            &analysis, &mut project,
        );
        assert!(result.error.is_none());
        assert_eq!(result.output[0].text, "existing comment");
    }

    #[test]
    fn test_execution_limit() {
        let analysis = empty_analysis();
        let mut project = empty_project();
        let result = execute_script("loop { let x = 1; }", &analysis, &mut project);
        assert!(result.error.is_some());
    }
}
