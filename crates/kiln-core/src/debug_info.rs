//! DWARF and PDB debug information parsing.
//!
//! Extracts function signatures, variable names, types, and source-level
//! line number mappings from debug sections embedded in binaries.

use std::borrow::Cow;
use std::collections::BTreeMap;

use gimli::Reader;
use object::Object;

use crate::model::BinaryImage;

/// Where a local variable lives at runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum VariableLocation {
    /// Machine register (e.g. "rdi", "x0").
    Register(String),
    /// Offset from frame/stack pointer.
    StackOffset(i64),
    /// Fixed memory address.
    Address(u64),
}

/// A source-code location (file, line, column).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}

/// A variable (parameter or local) extracted from debug info.
#[derive(Debug, Clone)]
pub struct DebugVariable {
    pub name: String,
    pub type_name: Option<String>,
    pub location: Option<VariableLocation>,
}

/// A function described by debug info.
#[derive(Debug, Clone)]
pub struct DebugFunction {
    pub name: String,
    /// Full source-level signature (e.g. `int main(int argc, char** argv)`).
    pub signature: Option<String>,
    pub parameters: Vec<DebugVariable>,
    pub local_variables: Vec<DebugVariable>,
    pub return_type: Option<String>,
}

/// Parsed debug information for a binary.
#[derive(Debug, Clone, Default)]
pub struct DebugInfo {
    /// Functions keyed by entry address.
    pub functions: BTreeMap<u64, DebugFunction>,
    /// Source-line mappings keyed by instruction address.
    pub source_lines: BTreeMap<u64, SourceLocation>,
    /// Whether any debug info was found at all.
    pub has_debug_info: bool,
    /// Format name (e.g. "DWARF", "PDB").
    pub debug_format: Option<String>,
}

impl DebugInfo {
    /// Look up the debug function whose range contains `addr`.
    pub fn function_at(&self, addr: u64) -> Option<&DebugFunction> {
        self.functions.get(&addr)
    }

    /// Look up the source location for an instruction address.
    pub fn source_location_at(&self, addr: u64) -> Option<&SourceLocation> {
        self.source_lines.get(&addr)
    }

    /// Build a human-readable signature string for the function at `addr`.
    pub fn signature_at(&self, addr: u64) -> Option<&str> {
        self.functions
            .get(&addr)
            .and_then(|f| f.signature.as_deref())
    }
}

// ---------------------------------------------------------------------------
// DWARF parsing (via `gimli` + `object`)
// ---------------------------------------------------------------------------

/// Parse DWARF debug info from raw binary data.
///
/// Returns an empty `DebugInfo` (with `has_debug_info = false`) when the
/// binary contains no DWARF sections or parsing fails.
pub fn parse_dwarf(data: &[u8]) -> DebugInfo {
    match parse_dwarf_inner(data) {
        Ok(info) => info,
        Err(e) => {
            log::debug!("DWARF parsing failed: {}", e);
            DebugInfo::default()
        }
    }
}

/// Helper type aliases for gimli reader used throughout this module.
type GimliReader<'a> = gimli::EndianSlice<'a, gimli::RunTimeEndian>;
type DwarfReader<'a> = gimli::Dwarf<GimliReader<'a>>;

fn parse_dwarf_inner(data: &[u8]) -> Result<DebugInfo, Box<dyn std::error::Error>> {
    let obj = object::File::parse(data)?;

    // Build the gimli::Dwarf from the object file's sections.
    let endian = if obj.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };

    let load_section = |id: gimli::SectionId| -> Result<Cow<'_, [u8]>, gimli::Error> {
        use object::ObjectSection;
        let data: Cow<'_, [u8]> = obj
            .section_by_name(id.name())
            .and_then(|s: object::Section<'_, '_>| s.uncompressed_data().ok())
            .unwrap_or(Cow::Borrowed(&[]));
        Ok(data)
    };

    let dwarf_sections = gimli::DwarfSections::load(load_section)?;
    let dwarf = dwarf_sections.borrow(|section| gimli::EndianSlice::new(section, endian));

    let has_info = {
        use object::ObjectSection;
        obj.section_by_name(".debug_info")
            .map(|s: object::Section<'_, '_>| s.size() > 0)
            .unwrap_or(false)
    };

    if !has_info {
        return Ok(DebugInfo::default());
    }

    let mut info = DebugInfo {
        functions: BTreeMap::new(),
        source_lines: BTreeMap::new(),
        has_debug_info: true,
        debug_format: Some("DWARF".to_string()),
    };

    // ---- Parse compilation units ----
    let mut units = dwarf.units();
    while let Ok(Some(header)) = units.next() {
        let unit = match dwarf.unit(header) {
            Ok(u) => u,
            Err(_) => continue,
        };

        // Parse line-number program for this unit
        parse_line_program(&dwarf, &unit, &mut info.source_lines);

        // Walk DIEs for functions
        let mut entries = unit.entries();
        while let Ok(Some((_depth, entry))) = entries.next_dfs() {
            if entry.tag() == gimli::DW_TAG_subprogram {
                if let Some((addr, func)) = parse_subprogram(&dwarf, &unit, entry) {
                    info.functions.insert(addr, func);
                }
            }
        }
    }

    Ok(info)
}

/// Extract a string attribute value from a DIE.
fn attr_string<'a>(
    dwarf: &'a DwarfReader<'a>,
    unit: &'a gimli::Unit<GimliReader<'a>>,
    entry: &gimli::DebuggingInformationEntry<'a, 'a, GimliReader<'a>>,
    attr_name: gimli::DwAt,
) -> Option<String> {
    let attr = entry.attr_value(attr_name).ok()??;
    match attr {
        gimli::AttributeValue::DebugStrRef(offset) => {
            let s = dwarf.debug_str.get_str(offset).ok()?;
            Some(s.to_string_lossy().into_owned())
        }
        gimli::AttributeValue::String(s) => Some(s.to_string_lossy().into_owned()),
        gimli::AttributeValue::DebugStrOffsetsIndex(index) => {
            let s = dwarf.attr_string(unit, attr).ok()?;
            let _ = index;
            Some(s.to_string_lossy().into_owned())
        }
        _ => None,
    }
}

/// Extract a `u64` address attribute value.
fn attr_address(
    entry: &gimli::DebuggingInformationEntry<'_, '_, GimliReader<'_>>,
    attr_name: gimli::DwAt,
) -> Option<u64> {
    let attr = entry.attr_value(attr_name).ok()??;
    match attr {
        gimli::AttributeValue::Addr(a) => Some(a),
        gimli::AttributeValue::Udata(v) => Some(v),
        _ => None,
    }
}

/// Resolve the name of a type DIE at the given offset (best-effort).
fn resolve_type_name<'a>(
    dwarf: &'a DwarfReader<'a>,
    unit: &'a gimli::Unit<GimliReader<'a>>,
    type_offset: gimli::UnitOffset<usize>,
) -> Option<String> {
    let entry = unit.entry(type_offset).ok()?;
    // Try DW_AT_name first
    if let Some(name) = attr_string(dwarf, unit, &entry, gimli::DW_AT_name) {
        return Some(name);
    }
    // For pointer / reference / const / volatile types, recurse
    match entry.tag() {
        gimli::DW_TAG_pointer_type => {
            let inner = resolve_inner_type_name(dwarf, unit, &entry);
            Some(format!("{}*", inner.unwrap_or_else(|| "void".to_string())))
        }
        gimli::DW_TAG_reference_type => {
            let inner = resolve_inner_type_name(dwarf, unit, &entry);
            Some(format!("{}&", inner.unwrap_or_else(|| "void".to_string())))
        }
        gimli::DW_TAG_const_type => {
            let inner = resolve_inner_type_name(dwarf, unit, &entry);
            Some(format!(
                "const {}",
                inner.unwrap_or_else(|| "void".to_string())
            ))
        }
        gimli::DW_TAG_volatile_type => {
            let inner = resolve_inner_type_name(dwarf, unit, &entry);
            Some(format!(
                "volatile {}",
                inner.unwrap_or_else(|| "void".to_string())
            ))
        }
        gimli::DW_TAG_typedef => attr_string(dwarf, unit, &entry, gimli::DW_AT_name),
        _ => None,
    }
}

/// Follow DW_AT_type to get the inner type name.
fn resolve_inner_type_name<'a>(
    dwarf: &'a DwarfReader<'a>,
    unit: &'a gimli::Unit<GimliReader<'a>>,
    entry: &gimli::DebuggingInformationEntry<'a, 'a, GimliReader<'a>>,
) -> Option<String> {
    let type_attr = entry.attr_value(gimli::DW_AT_type).ok()??;
    match type_attr {
        gimli::AttributeValue::UnitRef(offset) => resolve_type_name(dwarf, unit, offset),
        _ => None,
    }
}

/// Parse a DW_TAG_subprogram into a `DebugFunction`.
fn parse_subprogram<'a>(
    dwarf: &'a DwarfReader<'a>,
    unit: &'a gimli::Unit<GimliReader<'a>>,
    entry: &gimli::DebuggingInformationEntry<'a, 'a, GimliReader<'a>>,
) -> Option<(u64, DebugFunction)> {
    let addr = attr_address(entry, gimli::DW_AT_low_pc)?;

    let name = attr_string(dwarf, unit, entry, gimli::DW_AT_name)
        .or_else(|| attr_string(dwarf, unit, entry, gimli::DW_AT_linkage_name))
        .unwrap_or_else(|| format!("sub_{:x}", addr));

    // Return type
    let return_type =
        entry
            .attr_value(gimli::DW_AT_type)
            .ok()
            .flatten()
            .and_then(|attr| match attr {
                gimli::AttributeValue::UnitRef(offset) => resolve_type_name(dwarf, unit, offset),
                _ => None,
            });

    // Walk children for parameters and local variables
    let mut parameters = Vec::new();
    let mut local_variables = Vec::new();

    let mut tree = match unit.entries_tree(Some(entry.offset())) {
        Ok(t) => t,
        Err(_) => {
            let sig = build_signature(&name, &return_type, &parameters);
            return Some((
                addr,
                DebugFunction {
                    name,
                    signature: Some(sig),
                    parameters,
                    local_variables,
                    return_type,
                },
            ));
        }
    };

    let root = match tree.root() {
        Ok(r) => r,
        Err(_) => {
            let sig = build_signature(&name, &return_type, &parameters);
            return Some((
                addr,
                DebugFunction {
                    name,
                    signature: Some(sig),
                    parameters,
                    local_variables,
                    return_type,
                },
            ));
        }
    };

    let mut children = root.children();
    while let Ok(Some(child)) = children.next() {
        let child_entry = child.entry();
        match child_entry.tag() {
            gimli::DW_TAG_formal_parameter => {
                if let Some(var) = parse_variable(dwarf, unit, child_entry) {
                    parameters.push(var);
                }
            }
            gimli::DW_TAG_variable => {
                if let Some(var) = parse_variable(dwarf, unit, child_entry) {
                    local_variables.push(var);
                }
            }
            _ => {}
        }
    }

    let sig = build_signature(&name, &return_type, &parameters);
    Some((
        addr,
        DebugFunction {
            name,
            signature: Some(sig),
            parameters,
            local_variables,
            return_type,
        },
    ))
}

/// Parse a DW_TAG_formal_parameter or DW_TAG_variable into a `DebugVariable`.
fn parse_variable<'a>(
    dwarf: &'a DwarfReader<'a>,
    unit: &'a gimli::Unit<GimliReader<'a>>,
    entry: &gimli::DebuggingInformationEntry<'a, 'a, GimliReader<'a>>,
) -> Option<DebugVariable> {
    let name = attr_string(dwarf, unit, entry, gimli::DW_AT_name)?;

    let type_name =
        entry
            .attr_value(gimli::DW_AT_type)
            .ok()
            .flatten()
            .and_then(|attr| match attr {
                gimli::AttributeValue::UnitRef(offset) => resolve_type_name(dwarf, unit, offset),
                _ => None,
            });

    // Parse location (simplified: just grab the expression kind)
    let location = parse_location(entry);

    Some(DebugVariable {
        name,
        type_name,
        location,
    })
}

/// Best-effort extraction of a variable location from DW_AT_location.
fn parse_location(
    entry: &gimli::DebuggingInformationEntry<'_, '_, GimliReader<'_>>,
) -> Option<VariableLocation> {
    let attr = entry.attr_value(gimli::DW_AT_location).ok()??;
    match attr {
        gimli::AttributeValue::Exprloc(ref expr) => {
            let mut cursor = expr.0;
            if cursor.is_empty() {
                return None;
            }
            let byte = cursor.read_u8().ok()?;
            match byte {
                // DW_OP_fbreg: signed LEB128 offset from frame base
                0x91 => {
                    let offset = cursor.read_sleb128().ok()?;
                    Some(VariableLocation::StackOffset(offset))
                }
                // DW_OP_addr
                0x03 => {
                    let addr = cursor.read_address(8).ok()?;
                    Some(VariableLocation::Address(addr))
                }
                // DW_OP_reg0 .. DW_OP_reg31
                b if (0x50..=0x6f).contains(&b) => {
                    let regnum = b - 0x50;
                    Some(VariableLocation::Register(format!("reg{}", regnum)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Build a C-style function signature string.
fn build_signature(name: &str, return_type: &Option<String>, params: &[DebugVariable]) -> String {
    let ret = return_type.as_deref().unwrap_or("void");
    let param_strs: Vec<String> = params
        .iter()
        .map(|p| {
            if let Some(ref tn) = p.type_name {
                format!("{} {}", tn, p.name)
            } else {
                p.name.clone()
            }
        })
        .collect();
    format!("{} {}({})", ret, name, param_strs.join(", "))
}

/// Parse the DWARF line-number program for a compilation unit.
fn parse_line_program(
    dwarf: &DwarfReader<'_>,
    unit: &gimli::Unit<GimliReader<'_>>,
    source_lines: &mut BTreeMap<u64, SourceLocation>,
) {
    let program = match unit.line_program.clone() {
        Some(p) => p,
        None => return,
    };

    let (complete_program, sequences) = match program.sequences() {
        Ok(s) => s,
        Err(_) => return,
    };
    let header = complete_program.header();

    for seq in sequences {
        let mut sm = complete_program.resume_from(&seq);
        while let Ok(Some((_, &row))) = sm.next_row() {
            if row.address() == 0 {
                continue;
            }
            let file_entry = match row.file(header) {
                Some(f) => f,
                None => continue,
            };

            let file_name = match dwarf.attr_string(unit, file_entry.path_name()) {
                Ok(s) => s.to_string_lossy().into_owned(),
                Err(_) => continue,
            };

            // Optionally prepend directory
            let dir_name: Option<String> = file_entry
                .directory(header)
                .and_then(|d| dwarf.attr_string(unit, d).ok())
                .map(|s| s.to_string_lossy().into_owned());

            let full_path = if let Some(dir) = dir_name {
                if dir.is_empty() || file_name.starts_with('/') {
                    file_name
                } else {
                    format!("{}/{}", dir, file_name)
                }
            } else {
                file_name
            };

            let line = match row.line() {
                Some(l) => l.get() as u32,
                None => continue,
            };

            let column = match row.column() {
                gimli::ColumnType::LeftEdge => None,
                gimli::ColumnType::Column(c) => Some(c.get() as u32),
            };

            source_lines.insert(
                row.address(),
                SourceLocation {
                    file: full_path,
                    line,
                    column,
                },
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point: detect debug format and dispatch
// ---------------------------------------------------------------------------

/// Parse debug information from a loaded binary image.
///
/// Detects DWARF sections in ELF/Mach-O binaries.  PE/PDB support is
/// detected but not yet implemented (returns indicator only).
pub fn parse_debug_info(image: &BinaryImage) -> DebugInfo {
    // Check for DWARF sections
    let has_dwarf =
        image.find_section(".debug_info").is_some() || image.find_section("__debug_info").is_some();

    if has_dwarf {
        return parse_dwarf(&image.data);
    }

    // Check for PDB reference in PE binaries (stub – full PDB parsing not yet implemented)
    if image.format == crate::model::BinaryFormat::Pe {
        // PE files may reference an external .pdb; detect the CodeView entry
        let has_pdb_ref = image.find_section(".rdata").is_some()
            || image.data.windows(4).any(|w| w == b"RSDS" || w == b"NB10");
        if has_pdb_ref {
            return DebugInfo {
                functions: BTreeMap::new(),
                source_lines: BTreeMap::new(),
                has_debug_info: true,
                debug_format: Some("PDB (not parsed)".to_string()),
            };
        }
    }

    DebugInfo::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_info_default() {
        let info = DebugInfo::default();
        assert!(!info.has_debug_info);
        assert!(info.functions.is_empty());
        assert!(info.source_lines.is_empty());
        assert!(info.debug_format.is_none());
    }

    #[test]
    fn test_build_signature_no_params() {
        let sig = build_signature("main", &Some("int".to_string()), &[]);
        assert_eq!(sig, "int main()");
    }

    #[test]
    fn test_build_signature_with_params() {
        let params = vec![
            DebugVariable {
                name: "argc".to_string(),
                type_name: Some("int".to_string()),
                location: None,
            },
            DebugVariable {
                name: "argv".to_string(),
                type_name: Some("char**".to_string()),
                location: None,
            },
        ];
        let sig = build_signature("main", &Some("int".to_string()), &params);
        assert_eq!(sig, "int main(int argc, char** argv)");
    }

    #[test]
    fn test_build_signature_void_return() {
        let sig = build_signature("foo", &None, &[]);
        assert_eq!(sig, "void foo()");
    }

    #[test]
    fn test_function_at_lookup() {
        let mut info = DebugInfo::default();
        info.functions.insert(
            0x1000,
            DebugFunction {
                name: "test_fn".to_string(),
                signature: Some("void test_fn()".to_string()),
                parameters: vec![],
                local_variables: vec![],
                return_type: None,
            },
        );
        assert!(info.function_at(0x1000).is_some());
        assert_eq!(info.function_at(0x1000).unwrap().name, "test_fn");
        assert!(info.function_at(0x2000).is_none());
    }

    #[test]
    fn test_source_location_at_lookup() {
        let mut info = DebugInfo::default();
        info.source_lines.insert(
            0x1000,
            SourceLocation {
                file: "main.c".to_string(),
                line: 42,
                column: Some(5),
            },
        );
        let loc = info.source_location_at(0x1000).unwrap();
        assert_eq!(loc.file, "main.c");
        assert_eq!(loc.line, 42);
        assert_eq!(loc.column, Some(5));
        assert!(info.source_location_at(0x2000).is_none());
    }

    #[test]
    fn test_signature_at() {
        let mut info = DebugInfo::default();
        info.functions.insert(
            0x400,
            DebugFunction {
                name: "bar".to_string(),
                signature: Some("int bar(float x)".to_string()),
                parameters: vec![],
                local_variables: vec![],
                return_type: Some("int".to_string()),
            },
        );
        assert_eq!(info.signature_at(0x400), Some("int bar(float x)"));
        assert_eq!(info.signature_at(0x500), None);
    }

    #[test]
    fn test_parse_dwarf_empty_data() {
        // Invalid data should return an empty DebugInfo, not panic
        let info = parse_dwarf(&[]);
        assert!(!info.has_debug_info);
    }

    #[test]
    fn test_parse_dwarf_random_data() {
        let info = parse_dwarf(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(!info.has_debug_info);
    }

    #[test]
    fn test_parse_debug_info_no_debug_sections() {
        let image = BinaryImage {
            filename: "test.bin".to_string(),
            format: crate::model::BinaryFormat::Elf,
            architecture: crate::model::Architecture::X86_64,
            entry_point: 0x1000,
            bits: 64,
            is_little_endian: true,
            sections: vec![],
            segments: vec![],
            symbols: vec![],
            data: vec![],
        };
        let info = parse_debug_info(&image);
        assert!(!info.has_debug_info);
    }

    #[test]
    fn test_variable_location_variants() {
        let reg = VariableLocation::Register("rdi".to_string());
        let stack = VariableLocation::StackOffset(-16);
        let addr = VariableLocation::Address(0x4000);
        assert_eq!(reg, VariableLocation::Register("rdi".to_string()));
        assert_eq!(stack, VariableLocation::StackOffset(-16));
        assert_eq!(addr, VariableLocation::Address(0x4000));
    }
}
