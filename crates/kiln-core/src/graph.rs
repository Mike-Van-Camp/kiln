//! Graph algorithms for CFG analysis: Sugiyama layout, dominance trees, call graphs.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::model::{BasicBlock, CrossReference, Function, XrefType};

// ---------------------------------------------------------------------------
// Sugiyama layered layout
// ---------------------------------------------------------------------------

/// Assign layers using longest-path layering from the entry node.
///
/// Returns a map from node address to layer index (0 = top).
pub fn longest_path_layering(entry: u64, blocks: &[BasicBlock]) -> HashMap<u64, usize> {
    let block_set: HashSet<u64> = blocks.iter().map(|b| b.start_addr).collect();
    let succ_map: HashMap<u64, &[u64]> = blocks
        .iter()
        .map(|b| (b.start_addr, b.successors.as_slice()))
        .collect();

    // Topological order via DFS post-order; also record back edges to skip
    let mut visited = HashSet::new();
    let mut on_stack = HashSet::new();
    let mut topo_order = Vec::new();
    let mut back_edges: HashSet<(u64, u64)> = HashSet::new();
    let mut dfs_stack: Vec<(u64, bool)> = vec![(entry, false)];

    while let Some((addr, processed)) = dfs_stack.pop() {
        if processed {
            on_stack.remove(&addr);
            topo_order.push(addr);
            continue;
        }
        if !visited.insert(addr) {
            continue;
        }
        on_stack.insert(addr);
        dfs_stack.push((addr, true));
        if let Some(succs) = succ_map.get(&addr) {
            for &s in succs.iter().rev() {
                if !block_set.contains(&s) {
                    continue;
                }
                if on_stack.contains(&s) {
                    back_edges.insert((addr, s));
                } else if !visited.contains(&s) {
                    dfs_stack.push((s, false));
                }
            }
        }
    }

    // Add unreachable nodes
    for b in blocks {
        if !visited.contains(&b.start_addr) {
            topo_order.push(b.start_addr);
        }
    }

    topo_order.reverse();

    // Longest path from entry, skipping back edges
    let mut dist: HashMap<u64, usize> = HashMap::new();
    dist.insert(entry, 0);

    for &addr in &topo_order {
        let d = dist.get(&addr).copied().unwrap_or(0);
        if let Some(succs) = succ_map.get(&addr) {
            for &s in *succs {
                if block_set.contains(&s) && !back_edges.contains(&(addr, s)) {
                    let nd = d + 1;
                    let cur = dist.entry(s).or_insert(0);
                    if nd > *cur {
                        *cur = nd;
                    }
                }
            }
        }
    }

    // Ensure unreachable blocks get a layer
    let max_layer = dist.values().copied().max().unwrap_or(0);
    for b in blocks {
        dist.entry(b.start_addr).or_insert(max_layer + 1);
    }

    dist
}

/// Minimize edge crossings within a layered graph using the barycenter heuristic.
///
/// `layer_blocks` is modified in place. Each inner Vec is reordered to reduce crossings.
/// `blocks` provides the successor information.
pub fn minimize_crossings(layer_blocks: &mut [Vec<u64>], blocks: &[BasicBlock], passes: usize) {
    let succ_map: HashMap<u64, &[u64]> = blocks
        .iter()
        .map(|b| (b.start_addr, b.successors.as_slice()))
        .collect();
    let pred_map: HashMap<u64, &[u64]> = blocks
        .iter()
        .map(|b| (b.start_addr, b.predecessors.as_slice()))
        .collect();

    for _ in 0..passes {
        // Top-down pass: order each layer by barycenter of predecessors in the layer above
        for layer_idx in 1..layer_blocks.len() {
            let prev_pos: HashMap<u64, usize> = layer_blocks[layer_idx - 1]
                .iter()
                .enumerate()
                .map(|(i, &a)| (a, i))
                .collect();

            let mut bary: Vec<(u64, f64)> = layer_blocks[layer_idx]
                .iter()
                .map(|&addr| {
                    let preds = pred_map.get(&addr).copied().unwrap_or(&[]);
                    let positions: Vec<f64> = preds
                        .iter()
                        .filter_map(|p| prev_pos.get(p).map(|&i| i as f64))
                        .collect();
                    let bc = if positions.is_empty() {
                        f64::MAX
                    } else {
                        positions.iter().sum::<f64>() / positions.len() as f64
                    };
                    (addr, bc)
                })
                .collect();

            bary.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            layer_blocks[layer_idx] = bary.into_iter().map(|(a, _)| a).collect();
        }

        // Bottom-up pass: order each layer by barycenter of successors in the layer below
        if layer_blocks.len() < 2 {
            continue;
        }
        for layer_idx in (0..layer_blocks.len() - 1).rev() {
            let next_pos: HashMap<u64, usize> = layer_blocks[layer_idx + 1]
                .iter()
                .enumerate()
                .map(|(i, &a)| (a, i))
                .collect();

            let mut bary: Vec<(u64, f64)> = layer_blocks[layer_idx]
                .iter()
                .map(|&addr| {
                    let succs = succ_map.get(&addr).copied().unwrap_or(&[]);
                    let positions: Vec<f64> = succs
                        .iter()
                        .filter_map(|s| next_pos.get(s).map(|&i| i as f64))
                        .collect();
                    let bc = if positions.is_empty() {
                        f64::MAX
                    } else {
                        positions.iter().sum::<f64>() / positions.len() as f64
                    };
                    (addr, bc)
                })
                .collect();

            bary.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            layer_blocks[layer_idx] = bary.into_iter().map(|(a, _)| a).collect();
        }
    }
}

// ---------------------------------------------------------------------------
// Dominance tree
// ---------------------------------------------------------------------------

/// Compute immediate dominators for the blocks of a function using the iterative
/// Cooper-Harvey-Kennedy algorithm.
///
/// Returns a map from block address to its immediate dominator address.
/// The entry block is not included (it has no immediate dominator).
pub fn compute_dominators(entry: u64, blocks: &[BasicBlock]) -> HashMap<u64, u64> {
    let block_set: HashSet<u64> = blocks.iter().map(|b| b.start_addr).collect();

    // Build RPO (reverse post-order)
    let mut visited = HashSet::new();
    let mut post_order = Vec::new();
    let succ_map: HashMap<u64, &[u64]> = blocks
        .iter()
        .map(|b| (b.start_addr, b.successors.as_slice()))
        .collect();

    {
        let mut stack: Vec<(u64, bool)> = vec![(entry, false)];
        while let Some((addr, processed)) = stack.pop() {
            if processed {
                post_order.push(addr);
                continue;
            }
            if !visited.insert(addr) {
                continue;
            }
            stack.push((addr, true));
            if let Some(succs) = succ_map.get(&addr) {
                for &s in succs.iter().rev() {
                    if block_set.contains(&s) && !visited.contains(&s) {
                        stack.push((s, false));
                    }
                }
            }
        }
    }

    post_order.reverse(); // now RPO
    let rpo_index: HashMap<u64, usize> = post_order
        .iter()
        .enumerate()
        .map(|(i, &a)| (a, i))
        .collect();

    let pred_map: HashMap<u64, Vec<u64>> = blocks
        .iter()
        .map(|b| {
            let preds: Vec<u64> = b
                .predecessors
                .iter()
                .copied()
                .filter(|p| block_set.contains(p))
                .collect();
            (b.start_addr, preds)
        })
        .collect();

    // Initialize: idom[entry] = entry, rest undefined
    let mut idom: HashMap<u64, u64> = HashMap::new();
    idom.insert(entry, entry);

    let intersect = |mut a: u64, mut b: u64, idom: &HashMap<u64, u64>| -> u64 {
        while a != b {
            let a_idx = rpo_index.get(&a).copied().unwrap_or(usize::MAX);
            let b_idx = rpo_index.get(&b).copied().unwrap_or(usize::MAX);
            if a_idx > b_idx {
                a = match idom.get(&a) {
                    Some(&d) => d,
                    None => return entry,
                };
            } else {
                b = match idom.get(&b) {
                    Some(&d) => d,
                    None => return entry,
                };
            }
        }
        a
    };

    let mut changed = true;
    while changed {
        changed = false;
        for &b in &post_order {
            if b == entry {
                continue;
            }
            let preds = match pred_map.get(&b) {
                Some(p) => p,
                None => continue,
            };
            // Find first processed predecessor
            let mut new_idom = None;
            for &p in preds {
                if idom.contains_key(&p) {
                    new_idom = Some(p);
                    break;
                }
            }
            let mut new_idom = match new_idom {
                Some(d) => d,
                None => continue,
            };
            for &p in preds {
                if p == new_idom {
                    continue;
                }
                if idom.contains_key(&p) {
                    new_idom = intersect(new_idom, p, &idom);
                }
            }
            if idom.get(&b) != Some(&new_idom) {
                idom.insert(b, new_idom);
                changed = true;
            }
        }
    }

    // Remove the self-loop for the entry node
    idom.remove(&entry);
    idom
}

/// Build a dominance tree from immediate dominators.
///
/// Returns a map from dominator address to the list of blocks it immediately dominates.
pub fn build_dominator_tree(idom: &HashMap<u64, u64>) -> HashMap<u64, Vec<u64>> {
    let mut tree: HashMap<u64, Vec<u64>> = HashMap::new();
    for (&child, &parent) in idom {
        tree.entry(parent).or_default().push(child);
    }
    // Sort children for deterministic output
    for children in tree.values_mut() {
        children.sort();
    }
    tree
}

// ---------------------------------------------------------------------------
// Call graph extraction
// ---------------------------------------------------------------------------

/// A node in the call graph.
#[derive(Debug, Clone)]
pub struct CallGraphNode {
    pub addr: u64,
    pub name: String,
}

/// An edge in the call graph.
#[derive(Debug, Clone)]
pub struct CallGraphEdge {
    pub from: u64,
    pub to: u64,
}

/// Extract a call graph from the analysis database.
///
/// Returns (nodes, edges) representing function-to-function call relationships.
pub fn extract_call_graph(
    functions: &BTreeMap<u64, Function>,
    xrefs: &[CrossReference],
) -> (Vec<CallGraphNode>, Vec<CallGraphEdge>) {
    let func_addrs: HashSet<u64> = functions.keys().copied().collect();

    // Build a map from any address to the function that contains it
    let mut addr_to_func: HashMap<u64, u64> = HashMap::new();
    for func in functions.values() {
        for block in &func.blocks {
            for insn in &block.instructions {
                addr_to_func.insert(insn.address, func.entry_addr);
            }
        }
    }

    let nodes: Vec<CallGraphNode> = functions
        .values()
        .map(|f| CallGraphNode {
            addr: f.entry_addr,
            name: f.name.clone(),
        })
        .collect();

    let mut edge_set: HashSet<(u64, u64)> = HashSet::new();
    let mut edges = Vec::new();

    for xref in xrefs {
        if xref.xref_type != XrefType::Call {
            continue;
        }
        let from_func = addr_to_func.get(&xref.from_addr).copied();
        let to_func = if func_addrs.contains(&xref.to_addr) {
            Some(xref.to_addr)
        } else {
            addr_to_func.get(&xref.to_addr).copied()
        };

        if let (Some(from), Some(to)) = (from_func, to_func) {
            if edge_set.insert((from, to)) {
                edges.push(CallGraphEdge { from, to });
            }
        }
    }

    (nodes, edges)
}

/// Apply Sugiyama layout to a call graph. Returns node positions keyed by address.
pub fn layout_call_graph(
    nodes: &[CallGraphNode],
    edges: &[CallGraphEdge],
) -> HashMap<u64, (f32, f32)> {
    if nodes.is_empty() {
        return HashMap::new();
    }

    let node_set: HashSet<u64> = nodes.iter().map(|n| n.addr).collect();
    let succs: HashMap<u64, Vec<u64>> = {
        let mut m: HashMap<u64, Vec<u64>> = HashMap::new();
        for e in edges {
            if node_set.contains(&e.from) && node_set.contains(&e.to) {
                m.entry(e.from).or_default().push(e.to);
            }
        }
        m
    };
    let preds: HashMap<u64, Vec<u64>> = {
        let mut m: HashMap<u64, Vec<u64>> = HashMap::new();
        for e in edges {
            if node_set.contains(&e.from) && node_set.contains(&e.to) {
                m.entry(e.to).or_default().push(e.from);
            }
        }
        m
    };

    // BFS layering from roots (nodes with no predecessors, or first node)
    let roots: Vec<u64> = nodes
        .iter()
        .filter(|n| preds.get(&n.addr).is_none_or(|p| p.is_empty()))
        .map(|n| n.addr)
        .collect();

    let mut layers: HashMap<u64, usize> = HashMap::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    for &r in &roots {
        if visited.insert(r) {
            layers.insert(r, 0);
            queue.push_back(r);
        }
    }
    // Ensure all nodes are queued
    for n in nodes {
        if visited.insert(n.addr) {
            layers.insert(n.addr, 0);
            queue.push_back(n.addr);
        }
    }

    while let Some(addr) = queue.pop_front() {
        let layer = layers[&addr];
        if let Some(s) = succs.get(&addr) {
            for &succ in s {
                if visited.insert(succ) {
                    layers.insert(succ, layer + 1);
                    queue.push_back(succ);
                } else {
                    // Ensure successor is at least one layer below
                    let cur = layers.get(&succ).copied().unwrap_or(0);
                    if cur <= layer {
                        layers.insert(succ, layer + 1);
                    }
                }
            }
        }
    }

    // Group by layer
    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut layer_nodes: Vec<Vec<u64>> = vec![Vec::new(); max_layer + 1];
    for (&addr, &layer) in &layers {
        layer_nodes[layer].push(addr);
    }
    for l in &mut layer_nodes {
        l.sort();
    }

    // Position nodes
    let node_w = 180.0_f32;
    let node_h = 40.0_f32;
    let gap_x = 40.0_f32;
    let gap_y = 60.0_f32;

    let mut positions: HashMap<u64, (f32, f32)> = HashMap::new();
    let mut y = 20.0_f32;
    for layer in &layer_nodes {
        let total_w = layer.len() as f32 * node_w + (layer.len().saturating_sub(1)) as f32 * gap_x;
        let mut x = -total_w / 2.0;
        for &addr in layer {
            positions.insert(addr, (x, y));
            x += node_w + gap_x;
        }
        y += node_h + gap_y;
    }

    positions
}

// ---------------------------------------------------------------------------
// SVG export
// ---------------------------------------------------------------------------

/// Represents a renderable graph for SVG export.
pub struct SvgGraph {
    pub nodes: Vec<SvgNode>,
    pub edges: Vec<SvgEdge>,
}

/// A node for SVG export.
pub struct SvgNode {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: String,
    pub sub_lines: Vec<String>,
}

/// An edge for SVG export.
pub struct SvgEdge {
    pub points: Vec<(f32, f32)>,
    pub color: String,
}

/// Generate an SVG string from a graph description.
pub fn export_svg(graph: &SvgGraph) -> String {
    // Compute bounding box
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for n in &graph.nodes {
        min_x = min_x.min(n.x);
        min_y = min_y.min(n.y);
        max_x = max_x.max(n.x + n.width);
        max_y = max_y.max(n.y + n.height);
    }
    for e in &graph.edges {
        for &(x, y) in &e.points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    let margin = 20.0;
    let ox = -min_x + margin;
    let oy = -min_y + margin;
    let w = (max_x - min_x + margin * 2.0).max(100.0);
    let h = (max_y - min_y + margin * 2.0).max(100.0);

    let mut svg = String::new();
    svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"");
    svg.push_str(&format!("{w}"));
    svg.push_str("\" height=\"");
    svg.push_str(&format!("{h}"));
    svg.push_str(&format!("\" viewBox=\"0 0 {w} {h}\">"));
    svg.push('\n');
    svg.push_str("<style>text { font-family: monospace; font-size: 12px; fill: #ccc; }</style>");
    svg.push('\n');

    // Background
    svg.push_str(&format!(
        "<rect width=\"{w}\" height=\"{h}\" fill=\"#1e1e28\"/>"
    ));
    svg.push('\n');

    // Edges
    for e in &graph.edges {
        if e.points.len() >= 2 {
            let mut d = String::new();
            for (i, &(x, y)) in e.points.iter().enumerate() {
                let cmd = if i == 0 { "M" } else { "L" };
                d.push_str(&format!("{} {:.1} {:.1} ", cmd, x + ox, y + oy));
            }
            svg.push_str(&format!(
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"/>",
                d.trim(),
                e.color
            ));
            svg.push('\n');
        }
    }

    // Nodes
    for n in &graph.nodes {
        let nx = n.x + ox;
        let ny = n.y + oy;
        svg.push_str(&format!(
            "<rect x=\"{nx:.1}\" y=\"{ny:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"4\" fill=\"#1e1e28\" stroke=\"#50506e\"/>",
            n.width, n.height
        ));
        svg.push('\n');
        // Header
        svg.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#64b4ff\">{}</text>",
            nx + 4.0,
            ny + 14.0,
            svg_escape(&n.label)
        ));
        svg.push('\n');
        // Sub lines
        let line_y_start = ny + 30.0;
        for (i, line) in n.sub_lines.iter().enumerate() {
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\">{}</text>",
                nx + 4.0,
                line_y_start + i as f32 * 15.0,
                svg_escape(line)
            ));
            svg.push('\n');
        }
    }

    svg.push_str("</svg>");
    svg
}

fn svg_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BasicBlock, CrossReference, Function, Instruction, XrefType};

    fn make_block(addr: u64, succs: Vec<u64>, preds: Vec<u64>) -> BasicBlock {
        BasicBlock {
            start_addr: addr,
            end_addr: addr + 4,
            instructions: vec![Instruction {
                address: addr,
                size: 4,
                bytes: vec![0; 4],
                mnemonic: "nop".to_string(),
                operands: String::new(),
            }],
            successors: succs,
            predecessors: preds,
        }
    }

    // --- Sugiyama layering tests ---

    #[test]
    fn test_longest_path_linear_chain() {
        // A -> B -> C
        let blocks = vec![
            make_block(0x100, vec![0x200], vec![]),
            make_block(0x200, vec![0x300], vec![0x100]),
            make_block(0x300, vec![], vec![0x200]),
        ];
        let layers = longest_path_layering(0x100, &blocks);
        assert_eq!(layers[&0x100], 0);
        assert_eq!(layers[&0x200], 1);
        assert_eq!(layers[&0x300], 2);
    }

    #[test]
    fn test_longest_path_diamond() {
        //     A
        //    / \
        //   B   C
        //    \ /
        //     D
        let blocks = vec![
            make_block(0x100, vec![0x200, 0x300], vec![]),
            make_block(0x200, vec![0x400], vec![0x100]),
            make_block(0x300, vec![0x400], vec![0x100]),
            make_block(0x400, vec![], vec![0x200, 0x300]),
        ];
        let layers = longest_path_layering(0x100, &blocks);
        assert_eq!(layers[&0x100], 0);
        assert_eq!(layers[&0x200], 1);
        assert_eq!(layers[&0x300], 1);
        assert_eq!(layers[&0x400], 2);
    }

    #[test]
    fn test_longest_path_with_back_edge() {
        // A -> B -> C -> A (loop)
        // The back edge C -> A is detected and skipped during layering.
        let blocks = vec![
            make_block(0x100, vec![0x200], vec![0x300]),
            make_block(0x200, vec![0x300], vec![0x100]),
            make_block(0x300, vec![0x100], vec![0x200]),
        ];
        let layers = longest_path_layering(0x100, &blocks);
        assert_eq!(layers[&0x100], 0);
        assert_eq!(layers[&0x200], 1);
        assert_eq!(layers[&0x300], 2);
    }

    // --- Crossing minimization tests ---

    #[test]
    fn test_minimize_crossings_simple() {
        // Layer 0: [A, B], Layer 1: [C, D]
        // A -> D, B -> C  (crossed edges)
        // After minimization, layer 1 should be [D, C] or the order that reduces crossings
        let blocks = vec![
            make_block(0x100, vec![0x400], vec![]), // A -> D
            make_block(0x200, vec![0x300], vec![]), // B -> C
            make_block(0x300, vec![], vec![0x200]), // C
            make_block(0x400, vec![], vec![0x100]), // D
        ];
        let mut layer_blocks = vec![vec![0x100, 0x200], vec![0x300, 0x400]];
        minimize_crossings(&mut layer_blocks, &blocks, 2);
        // After minimization, D should come before C to avoid crossing
        assert_eq!(layer_blocks[1], vec![0x400, 0x300]);
    }

    // --- Dominance tree tests ---

    #[test]
    fn test_dominators_linear() {
        // A -> B -> C
        let blocks = vec![
            make_block(0x100, vec![0x200], vec![]),
            make_block(0x200, vec![0x300], vec![0x100]),
            make_block(0x300, vec![], vec![0x200]),
        ];
        let idom = compute_dominators(0x100, &blocks);
        assert_eq!(idom[&0x200], 0x100);
        assert_eq!(idom[&0x300], 0x200);
    }

    #[test]
    fn test_dominators_diamond() {
        //     A
        //    / \
        //   B   C
        //    \ /
        //     D
        let blocks = vec![
            make_block(0x100, vec![0x200, 0x300], vec![]),
            make_block(0x200, vec![0x400], vec![0x100]),
            make_block(0x300, vec![0x400], vec![0x100]),
            make_block(0x400, vec![], vec![0x200, 0x300]),
        ];
        let idom = compute_dominators(0x100, &blocks);
        assert_eq!(idom[&0x200], 0x100);
        assert_eq!(idom[&0x300], 0x100);
        // D is dominated by A (the common dominator of B and C)
        assert_eq!(idom[&0x400], 0x100);
    }

    #[test]
    fn test_dominators_if_then() {
        //   A
        //  / \
        // B   |
        //  \ /
        //   C
        let blocks = vec![
            make_block(0x100, vec![0x200, 0x300], vec![]),
            make_block(0x200, vec![0x300], vec![0x100]),
            make_block(0x300, vec![], vec![0x100, 0x200]),
        ];
        let idom = compute_dominators(0x100, &blocks);
        assert_eq!(idom[&0x200], 0x100);
        assert_eq!(idom[&0x300], 0x100);
    }

    #[test]
    fn test_dominator_tree_structure() {
        let mut idom = HashMap::new();
        idom.insert(0x200, 0x100);
        idom.insert(0x300, 0x100);
        idom.insert(0x400, 0x200);

        let tree = build_dominator_tree(&idom);
        assert_eq!(tree[&0x100], vec![0x200, 0x300]);
        assert_eq!(tree[&0x200], vec![0x400]);
        assert!(!tree.contains_key(&0x300));
    }

    // --- Call graph tests ---

    #[test]
    fn test_call_graph_extraction() {
        let mut functions = BTreeMap::new();
        functions.insert(
            0x1000,
            Function {
                name: "main".to_string(),
                entry_addr: 0x1000,
                blocks: vec![BasicBlock {
                    start_addr: 0x1000,
                    end_addr: 0x1010,
                    instructions: vec![Instruction {
                        address: 0x1000,
                        size: 4,
                        bytes: vec![0; 4],
                        mnemonic: "call".to_string(),
                        operands: "0x2000".to_string(),
                    }],
                    successors: vec![],
                    predecessors: vec![],
                }],
                xrefs_to: vec![],
                xrefs_from: vec![],
            },
        );
        functions.insert(
            0x2000,
            Function {
                name: "helper".to_string(),
                entry_addr: 0x2000,
                blocks: vec![BasicBlock {
                    start_addr: 0x2000,
                    end_addr: 0x2010,
                    instructions: vec![Instruction {
                        address: 0x2000,
                        size: 4,
                        bytes: vec![0; 4],
                        mnemonic: "ret".to_string(),
                        operands: String::new(),
                    }],
                    successors: vec![],
                    predecessors: vec![],
                }],
                xrefs_to: vec![],
                xrefs_from: vec![],
            },
        );

        let xrefs = vec![CrossReference {
            from_addr: 0x1000,
            to_addr: 0x2000,
            xref_type: XrefType::Call,
        }];

        let (nodes, edges) = extract_call_graph(&functions, &xrefs);
        assert_eq!(nodes.len(), 2);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, 0x1000);
        assert_eq!(edges[0].to, 0x2000);
    }

    #[test]
    fn test_call_graph_deduplication() {
        let mut functions = BTreeMap::new();
        functions.insert(
            0x1000,
            Function {
                name: "main".to_string(),
                entry_addr: 0x1000,
                blocks: vec![BasicBlock {
                    start_addr: 0x1000,
                    end_addr: 0x1020,
                    instructions: vec![
                        Instruction {
                            address: 0x1000,
                            size: 4,
                            bytes: vec![0; 4],
                            mnemonic: "call".to_string(),
                            operands: "0x2000".to_string(),
                        },
                        Instruction {
                            address: 0x1010,
                            size: 4,
                            bytes: vec![0; 4],
                            mnemonic: "call".to_string(),
                            operands: "0x2000".to_string(),
                        },
                    ],
                    successors: vec![],
                    predecessors: vec![],
                }],
                xrefs_to: vec![],
                xrefs_from: vec![],
            },
        );
        functions.insert(
            0x2000,
            Function {
                name: "helper".to_string(),
                entry_addr: 0x2000,
                blocks: vec![],
                xrefs_to: vec![],
                xrefs_from: vec![],
            },
        );

        let xrefs = vec![
            CrossReference {
                from_addr: 0x1000,
                to_addr: 0x2000,
                xref_type: XrefType::Call,
            },
            CrossReference {
                from_addr: 0x1010,
                to_addr: 0x2000,
                xref_type: XrefType::Call,
            },
        ];

        let (_, edges) = extract_call_graph(&functions, &xrefs);
        // Two calls from main to helper should produce only one edge
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_call_graph_ignores_jump_xrefs() {
        let mut functions = BTreeMap::new();
        functions.insert(
            0x1000,
            Function {
                name: "f1".to_string(),
                entry_addr: 0x1000,
                blocks: vec![BasicBlock {
                    start_addr: 0x1000,
                    end_addr: 0x1010,
                    instructions: vec![Instruction {
                        address: 0x1000,
                        size: 4,
                        bytes: vec![0; 4],
                        mnemonic: "jmp".to_string(),
                        operands: "0x2000".to_string(),
                    }],
                    successors: vec![],
                    predecessors: vec![],
                }],
                xrefs_to: vec![],
                xrefs_from: vec![],
            },
        );
        functions.insert(
            0x2000,
            Function {
                name: "f2".to_string(),
                entry_addr: 0x2000,
                blocks: vec![],
                xrefs_to: vec![],
                xrefs_from: vec![],
            },
        );

        let xrefs = vec![CrossReference {
            from_addr: 0x1000,
            to_addr: 0x2000,
            xref_type: XrefType::Jump,
        }];

        let (_, edges) = extract_call_graph(&functions, &xrefs);
        assert_eq!(edges.len(), 0);
    }

    // --- SVG export tests ---

    #[test]
    fn test_svg_export_basic() {
        let graph = SvgGraph {
            nodes: vec![SvgNode {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 40.0,
                label: "test".to_string(),
                sub_lines: vec![],
            }],
            edges: vec![],
        };
        let svg = export_svg(&graph);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("test"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn test_svg_escape_special_chars() {
        assert_eq!(svg_escape("<test>"), "&lt;test&gt;");
        assert_eq!(svg_escape("a & b"), "a &amp; b");
    }
}
