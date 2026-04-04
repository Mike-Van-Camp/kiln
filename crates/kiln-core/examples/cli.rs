/// CLI harness for Kiln — prints binary info and disassembly.
///
/// Usage: cargo run -p kiln-core --example cli -- <binary-path>
use kiln_core::disasm;
use kiln_core::model::SectionKind;

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <binary-path>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    println!("Loading binary: {}", path);
    println!("{}", "=".repeat(60));

    let image = match kiln_core::load_binary(path) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("Error loading binary: {}", e);
            std::process::exit(1);
        }
    };

    // Print header info
    println!("File:           {}", image.filename);
    println!("Format:         {}", image.format);
    println!("Architecture:   {}", image.architecture);
    println!("Bits:           {}", image.bits);
    println!(
        "Endianness:     {}",
        if image.is_little_endian {
            "Little"
        } else {
            "Big"
        }
    );
    println!("Entry Point:    0x{:016x}", image.entry_point);
    println!("File Size:      {} bytes", image.data.len());
    println!();

    // Print sections
    println!("Sections ({}):", image.sections.len());
    println!("{:-<80}", "");
    println!(
        "{:<20} {:>16} {:>12} {:>5} {:>5}",
        "Name", "Address", "Size", "Type", "Perms"
    );
    println!("{:-<80}", "");
    for sec in &image.sections {
        let kind = match sec.kind {
            SectionKind::Code => "CODE",
            SectionKind::Data => "DATA",
            SectionKind::ReadOnlyData => "RDATA",
            SectionKind::Bss => "BSS",
            SectionKind::Unknown => "???",
        };
        let perms = format!(
            "{}{}{}",
            if sec.readable { "R" } else { "-" },
            if sec.writable { "W" } else { "-" },
            if sec.executable { "X" } else { "-" },
        );
        println!(
            "{:<20} 0x{:014x} {:>12} {:>5} {:>5}",
            sec.name, sec.address, sec.size, kind, perms
        );
    }
    println!();

    // Print segments
    if !image.segments.is_empty() {
        println!("Segments ({}):", image.segments.len());
        println!("{:-<80}", "");
        for seg in &image.segments {
            let perms = format!(
                "{}{}{}",
                if seg.readable { "R" } else { "-" },
                if seg.writable { "W" } else { "-" },
                if seg.executable { "X" } else { "-" },
            );
            println!(
                "  {} @ 0x{:016x}  size: {}  perms: {}",
                seg.name, seg.address, seg.size, perms
            );
        }
        println!();
    }

    // Print symbols
    let imports = image.imports();
    let exports = image.exports();
    let functions = image.function_symbols();

    if !imports.is_empty() {
        println!("Imports ({}):", imports.len());
        for sym in imports.iter().take(20) {
            println!("  {} @ 0x{:016x}", sym.name, sym.address);
        }
        if imports.len() > 20 {
            println!("  ... and {} more", imports.len() - 20);
        }
        println!();
    }

    if !exports.is_empty() {
        println!("Exports ({}):", exports.len());
        for sym in exports.iter().take(20) {
            println!("  {} @ 0x{:016x}", sym.name, sym.address);
        }
        if exports.len() > 20 {
            println!("  ... and {} more", exports.len() - 20);
        }
        println!();
    }

    if !functions.is_empty() {
        println!("Functions ({}):", functions.len());
        for sym in functions.iter().take(30) {
            println!("  0x{:016x}  {}", sym.address, sym.name);
        }
        if functions.len() > 30 {
            println!("  ... and {} more", functions.len() - 30);
        }
        println!();
    }

    // Disassemble
    println!("Disassembling executable sections...");
    match disasm::disassemble_executable_sections(&image) {
        Ok(instructions) => {
            println!("Total instructions: {}\n", instructions.len());
            // Print first 50 instructions
            for insn in instructions.iter().take(50) {
                let bytes_hex: String = insn
                    .bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!(
                    "  0x{:016x}  {:<24} {} {}",
                    insn.address, bytes_hex, insn.mnemonic, insn.operands
                );
            }
            if instructions.len() > 50 {
                println!("  ... ({} more instructions)", instructions.len() - 50);
            }
        }
        Err(e) => {
            eprintln!("Disassembly error: {}", e);
        }
    }
}
