//! Control flow graph view: renders function CFGs with interactive pan/zoom.

use std::collections::{HashMap, HashSet, VecDeque};

use egui::{Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use kiln_core::analysis::{is_branch_mnemonic, AnalysisDatabase};
use kiln_core::model::Function;

/// Edge type for graph edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeType {
    TrueBranch,
    FalseBranch,
    Unconditional,
}

/// Layout information for a single basic block node.
struct NodeLayout {
    block_addr: u64,
    rect: Rect,
    instructions: Vec<(u64, String)>,
}

/// Layout information for an edge between blocks.
struct EdgeLayout {
    points: Vec<Pos2>,
    color: Color32,
}

/// Cached graph layout for a function.
struct GraphLayout {
    nodes: Vec<NodeLayout>,
    edges: Vec<EdgeLayout>,
}

/// State for the CFG graph view.
pub struct GraphView {
    /// Entry address of the currently selected function.
    pub selected_function: Option<u64>,
    /// Pan offset for the canvas.
    pan_offset: Vec2,
    /// Zoom level (default 1.0).
    zoom: f32,
    /// Cached layout.
    layout: Option<GraphLayout>,
    /// Whether the user is currently dragging to pan.
    dragging: bool,
}

impl Default for GraphView {
    fn default() -> Self {
        Self {
            selected_function: None,
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            layout: None,
            dragging: false,
        }
    }
}

impl GraphView {
    /// Select a function and invalidate the layout.
    pub fn select_function(&mut self, addr: u64) {
        if self.selected_function != Some(addr) {
            self.selected_function = Some(addr);
            self.layout = None;
            self.pan_offset = Vec2::ZERO;
            self.zoom = 1.0;
        }
    }

    /// Render the graph view.
    pub fn render(&mut self, ui: &mut Ui, analysis: &AnalysisDatabase) {
        if analysis.functions.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No functions detected");
            });
            return;
        }

        let func_addr = match self.selected_function {
            Some(addr) => addr,
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a function from the sidebar");
                });
                return;
            }
        };

        let func = match analysis.functions.get(&func_addr) {
            Some(f) => f,
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("Selected function not found");
                });
                return;
            }
        };

        // Build layout if needed
        if self.layout.is_none() {
            self.layout = Some(build_layout(func, ui));
        }

        let layout = self.layout.as_ref().unwrap();

        // Handle pan/zoom input
        let response = ui.allocate_rect(
            ui.available_rect_before_wrap(),
            egui::Sense::click_and_drag(),
        );

        if response.dragged_by(egui::PointerButton::Primary) {
            self.pan_offset += response.drag_delta();
            self.dragging = true;
        } else {
            self.dragging = false;
        }

        // Zoom with scroll wheel
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            let zoom_factor = 1.0 + scroll_delta * 0.002;
            self.zoom = (self.zoom * zoom_factor).clamp(0.2, 5.0);
        }

        let painter = ui.painter();
        let canvas_origin = response.rect.min.to_vec2() + self.pan_offset;

        // Draw edges first (behind nodes)
        for edge in &layout.edges {
            let points: Vec<Pos2> = edge
                .points
                .iter()
                .map(|p| {
                    Pos2::new(
                        p.x * self.zoom + canvas_origin.x,
                        p.y * self.zoom + canvas_origin.y,
                    )
                })
                .collect();

            if points.len() >= 2 {
                let stroke = Stroke::new(1.5 * self.zoom, edge.color);
                for window in points.windows(2) {
                    painter.line_segment([window[0], window[1]], stroke);
                }
                // Draw arrowhead at the last segment
                draw_arrowhead(
                    painter,
                    points[points.len() - 2],
                    points[points.len() - 1],
                    edge.color,
                    self.zoom,
                );
            }
        }

        // Draw nodes
        let mono_font = FontId::monospace((12.0 * self.zoom).max(6.0));
        let header_font = FontId::monospace((13.0 * self.zoom).max(7.0));

        for node in &layout.nodes {
            let rect = Rect::from_min_size(
                Pos2::new(
                    node.rect.min.x * self.zoom + canvas_origin.x,
                    node.rect.min.y * self.zoom + canvas_origin.y,
                ),
                node.rect.size() * self.zoom,
            );

            // Node background
            painter.rect_filled(rect, 4.0 * self.zoom, Color32::from_rgb(30, 30, 40));
            // Node border
            painter.rect_stroke(
                rect,
                4.0 * self.zoom,
                Stroke::new(1.0 * self.zoom, Color32::from_rgb(80, 80, 100)),
                StrokeKind::Outside,
            );

            // Block header (address)
            let header_pos = Pos2::new(rect.min.x + 4.0 * self.zoom, rect.min.y + 2.0 * self.zoom);
            painter.text(
                header_pos,
                egui::Align2::LEFT_TOP,
                format!("0x{:08x}", node.block_addr),
                header_font.clone(),
                Color32::from_rgb(100, 180, 255),
            );

            // Separator line below header
            let sep_y = rect.min.y + 18.0 * self.zoom;
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + 2.0 * self.zoom, sep_y),
                    Pos2::new(rect.max.x - 2.0 * self.zoom, sep_y),
                ],
                Stroke::new(0.5 * self.zoom, Color32::from_rgb(60, 60, 80)),
            );

            // Instructions
            let text_start_y = sep_y + 3.0 * self.zoom;
            let line_height = 15.0 * self.zoom;

            for (i, (addr, text)) in node.instructions.iter().enumerate() {
                let y = text_start_y + i as f32 * line_height;
                if y > rect.max.y {
                    break;
                }
                let text_pos = Pos2::new(rect.min.x + 4.0 * self.zoom, y);
                let display = format!("{:08x}: {}", addr, text);
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    display,
                    mono_font.clone(),
                    Color32::from_rgb(200, 200, 200),
                );
            }
        }

        // Draw zoom indicator
        let zoom_text = format!("{:.0}%", self.zoom * 100.0);
        painter.text(
            Pos2::new(response.rect.max.x - 10.0, response.rect.min.y + 10.0),
            egui::Align2::RIGHT_TOP,
            zoom_text,
            FontId::proportional(12.0),
            Color32::from_rgb(150, 150, 150),
        );
    }
}

/// Draw an arrowhead at the end of an edge.
fn draw_arrowhead(painter: &egui::Painter, from: Pos2, to: Pos2, color: Color32, zoom: f32) {
    let dir = (to - from).normalized();
    let arrow_size = 6.0 * zoom;
    let perp = Vec2::new(-dir.y, dir.x);

    let tip = to;
    let left = tip - dir * arrow_size + perp * arrow_size * 0.5;
    let right = tip - dir * arrow_size - perp * arrow_size * 0.5;

    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        color,
        Stroke::NONE,
    ));
}

/// Check if a branch mnemonic is conditional.
fn is_conditional_branch(m: &str) -> bool {
    let lower = m.to_ascii_lowercase();
    if matches!(lower.as_str(), "jmp" | "b" | "bx") {
        return false;
    }
    is_branch_mnemonic(&lower)
}

/// Build a graph layout for a function using layered (BFS) positioning.
fn build_layout(func: &Function, ui: &Ui) -> GraphLayout {
    if func.blocks.is_empty() {
        return GraphLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    }

    // Build block lookup
    let block_map: HashMap<u64, usize> = func
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.start_addr, i))
        .collect();

    // Assign layers via BFS from the entry block
    let entry_addr = func.entry_addr;
    let mut layers: HashMap<u64, usize> = HashMap::new();
    let mut queue: VecDeque<u64> = VecDeque::new();
    let mut visited: HashSet<u64> = HashSet::new();

    if block_map.contains_key(&entry_addr) {
        queue.push_back(entry_addr);
        visited.insert(entry_addr);
        layers.insert(entry_addr, 0);
    }

    while let Some(addr) = queue.pop_front() {
        let layer = layers[&addr];
        if let Some(&idx) = block_map.get(&addr) {
            for &succ in &func.blocks[idx].successors {
                if visited.insert(succ) && block_map.contains_key(&succ) {
                    layers.insert(succ, layer + 1);
                    queue.push_back(succ);
                }
            }
        }
    }

    // Also add any blocks not reachable from entry (orphan blocks)
    for block in &func.blocks {
        if !layers.contains_key(&block.start_addr) {
            let max_layer = layers.values().copied().max().unwrap_or(0);
            layers.insert(block.start_addr, max_layer + 1);
        }
    }

    // Group blocks by layer
    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut layer_blocks: Vec<Vec<u64>> = vec![Vec::new(); max_layer + 1];
    for (&addr, &layer) in &layers {
        layer_blocks[layer].push(addr);
    }
    // Sort blocks within each layer by address for deterministic layout
    for layer in &mut layer_blocks {
        layer.sort();
    }

    // Calculate node sizes
    let char_width = 7.5_f32; // approximate monospace character width at 12pt
    let line_height = 15.0_f32;
    let header_height = 22.0_f32;
    let padding = 8.0_f32;
    let min_node_width = 200.0_f32;
    let max_node_width = 450.0_f32;

    // Pre-compute formatted instructions and node sizes
    struct NodeInfo {
        instructions: Vec<(u64, String)>,
        width: f32,
        height: f32,
    }

    let _ = ui; // ui was passed for potential font measurements; using constants instead

    let mut node_infos: HashMap<u64, NodeInfo> = HashMap::new();

    for block in &func.blocks {
        let instructions: Vec<(u64, String)> = block
            .instructions
            .iter()
            .map(|insn| {
                let text = if insn.operands.is_empty() {
                    insn.mnemonic.clone()
                } else {
                    format!("{} {}", insn.mnemonic, insn.operands)
                };
                (insn.address, text)
            })
            .collect();

        let max_text_len = instructions
            .iter()
            .map(|(addr, text)| format!("{:08x}: {}", addr, text).len())
            .max()
            .unwrap_or(20);

        let width = (max_text_len as f32 * char_width + padding * 2.0)
            .clamp(min_node_width, max_node_width);
        let height = header_height + instructions.len() as f32 * line_height + padding;

        node_infos.insert(
            block.start_addr,
            NodeInfo {
                instructions,
                width,
                height,
            },
        );
    }

    // Position nodes: each layer has a Y based on cumulative height, centered horizontally
    let layer_gap = 60.0_f32;
    let node_gap = 40.0_f32;

    let mut layer_y_positions: Vec<f32> = Vec::new();
    let mut y_cursor = 20.0_f32;

    for layer in &layer_blocks {
        layer_y_positions.push(y_cursor);
        let max_height = layer
            .iter()
            .filter_map(|addr| node_infos.get(addr))
            .map(|info| info.height)
            .fold(0.0_f32, f32::max);
        y_cursor += max_height + layer_gap;
    }

    // Calculate total width per layer and center nodes
    let mut node_positions: HashMap<u64, (f32, f32)> = HashMap::new();

    for (layer_idx, layer) in layer_blocks.iter().enumerate() {
        let total_width: f32 = layer
            .iter()
            .filter_map(|addr| node_infos.get(addr))
            .map(|info| info.width)
            .sum::<f32>()
            + (layer.len().saturating_sub(1)) as f32 * node_gap;

        let mut x_cursor = -total_width / 2.0;

        for &addr in layer {
            if let Some(info) = node_infos.get(&addr) {
                node_positions.insert(addr, (x_cursor, layer_y_positions[layer_idx]));
                x_cursor += info.width + node_gap;
            }
        }
    }

    // Build node layouts
    let mut nodes: Vec<NodeLayout> = Vec::new();

    for block in &func.blocks {
        if let (Some(info), Some(&(x, y))) = (
            node_infos.get(&block.start_addr),
            node_positions.get(&block.start_addr),
        ) {
            nodes.push(NodeLayout {
                block_addr: block.start_addr,
                rect: Rect::from_min_size(Pos2::new(x, y), Vec2::new(info.width, info.height)),
                instructions: info.instructions.clone(),
            });
        }
    }

    // Build edge layouts
    let mut edges: Vec<EdgeLayout> = Vec::new();

    for block in &func.blocks {
        if block.successors.is_empty() {
            continue;
        }

        let from_addr = block.start_addr;
        let from_info = match node_infos.get(&from_addr) {
            Some(i) => i,
            None => continue,
        };
        let &(from_x, from_y) = match node_positions.get(&from_addr) {
            Some(p) => p,
            None => continue,
        };

        // Determine edge types based on last instruction
        let last_insn = match block.instructions.last() {
            Some(insn) => insn,
            None => continue,
        };

        let is_branch = is_branch_mnemonic(&last_insn.mnemonic);
        let is_cond = is_branch && is_conditional_branch(&last_insn.mnemonic);

        // For conditional branches: first successor is branch target (true), second is fallthrough (false)
        // The analysis builds successors with branch target first, then fallthrough
        let branch_target = if is_branch {
            kiln_core::analysis::parse_target_address(&last_insn.operands)
        } else {
            None
        };

        let from_bottom = Pos2::new(from_x + from_info.width / 2.0, from_y + from_info.height);

        let num_successors = block.successors.len();
        for (succ_idx, &succ_addr) in block.successors.iter().enumerate() {
            let to_info = match node_infos.get(&succ_addr) {
                Some(i) => i,
                None => continue,
            };
            let &(to_x, to_y) = match node_positions.get(&succ_addr) {
                Some(p) => p,
                None => continue,
            };

            let to_top = Pos2::new(to_x + to_info.width / 2.0, to_y);

            let edge_type = if is_cond {
                // Conditional branch: match the branch target
                if branch_target == Some(succ_addr) {
                    EdgeType::TrueBranch
                } else {
                    EdgeType::FalseBranch
                }
            } else {
                EdgeType::Unconditional
            };

            let color = match edge_type {
                EdgeType::TrueBranch => Color32::from_rgb(80, 200, 80),
                EdgeType::FalseBranch => Color32::from_rgb(220, 80, 80),
                EdgeType::Unconditional => Color32::from_rgb(80, 140, 220),
            };

            // Simple edge routing: offset departure points for multiple successors
            let x_offset = if num_successors > 1 {
                let spread = from_info.width * 0.3;
                let t = if num_successors > 1 {
                    succ_idx as f32 / (num_successors - 1) as f32 - 0.5
                } else {
                    0.0
                };
                t * spread
            } else {
                0.0
            };

            let from_pt = Pos2::new(from_bottom.x + x_offset, from_bottom.y);
            let points = vec![from_pt, to_top];

            edges.push(EdgeLayout { points, color });
        }
    }

    GraphLayout { nodes, edges }
}
