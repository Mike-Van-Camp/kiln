//! Control flow graph view: renders function CFGs with interactive pan/zoom.
//!
//! Supports three modes: CFG (default), call graph, and dominance tree.
//! Features: Sugiyama layout, quadratic bezier edge routing, minimap, SVG export.

use std::collections::HashMap;

use egui::{Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use kiln_core::analysis::{is_branch_mnemonic, parse_target_address, AnalysisDatabase};
use kiln_core::graph;
use kiln_core::model::Function;

/// Which graph to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphMode {
    Cfg,
    CallGraph,
    DominanceTree,
}

/// Edge type for graph edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeType {
    TrueBranch,
    FalseBranch,
    Unconditional,
}

/// Layout information for a single node.
struct NodeLayout {
    block_addr: u64,
    rect: Rect,
    instructions: Vec<(u64, String)>,
}

/// Layout information for an edge between nodes.
struct EdgeLayout {
    /// Control points for a quadratic bezier (3 points) or a straight line (2 points).
    points: Vec<Pos2>,
    color: Color32,
}

/// Cached graph layout.
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
    /// Current display mode.
    mode: GraphMode,
    /// Show minimap overlay.
    show_minimap: bool,
    /// Address to navigate to when a node is clicked.
    pending_navigation: Option<u64>,
    /// Block address for right-click context menu.
    context_menu_node: Option<u64>,
}

impl Default for GraphView {
    fn default() -> Self {
        Self {
            selected_function: None,
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            layout: None,
            dragging: false,
            mode: GraphMode::Cfg,
            show_minimap: true,
            pending_navigation: None,
            context_menu_node: None,
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

    /// Take the pending navigation address, if any.
    pub fn take_pending_navigation(&mut self) -> Option<u64> {
        self.pending_navigation.take()
    }

    /// Render the graph view.
    pub fn render(&mut self, ui: &mut Ui, analysis: &AnalysisDatabase) {
        // ── toolbar ──────────────────────────────────────────────────
        ui.horizontal(|ui| {
            let was_cfg = self.mode == GraphMode::Cfg;
            let was_call = self.mode == GraphMode::CallGraph;
            let was_dom = self.mode == GraphMode::DominanceTree;

            if ui.selectable_label(was_cfg, "CFG").clicked() && !was_cfg {
                self.mode = GraphMode::Cfg;
                self.layout = None;
            }
            if ui.selectable_label(was_call, "Call Graph").clicked() && !was_call {
                self.mode = GraphMode::CallGraph;
                self.layout = None;
            }
            if ui.selectable_label(was_dom, "Dominance Tree").clicked() && !was_dom {
                self.mode = GraphMode::DominanceTree;
                self.layout = None;
            }

            ui.separator();

            if ui.selectable_label(self.show_minimap, "Minimap").clicked() {
                self.show_minimap = !self.show_minimap;
            }

            ui.separator();

            if ui.button("Export SVG").clicked() {
                let svg = self.export_svg(analysis);
                ui.ctx().copy_text(svg);
            }
        });

        ui.separator();

        // ── content ──────────────────────────────────────────────────
        if analysis.functions.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No functions detected");
            });
            return;
        }

        match self.mode {
            GraphMode::CallGraph => self.render_call_graph(ui, analysis),
            GraphMode::Cfg | GraphMode::DominanceTree => self.render_cfg_or_dom(ui, analysis),
        }
    }

    // ── call graph mode ──────────────────────────────────────────────

    fn render_call_graph(&mut self, ui: &mut Ui, analysis: &AnalysisDatabase) {
        if self.layout.is_none() {
            self.layout = Some(build_call_graph_layout(analysis));
        }
        self.render_graph(ui);
    }

    // ── CFG / dominance tree mode ────────────────────────────────────

    fn render_cfg_or_dom(&mut self, ui: &mut Ui, analysis: &AnalysisDatabase) {
        let func_addr = match self.selected_function {
            Some(a) => a,
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

        if self.layout.is_none() {
            self.layout = Some(match self.mode {
                GraphMode::Cfg => build_cfg_layout(func),
                GraphMode::DominanceTree => build_dominance_layout(func),
                GraphMode::CallGraph => unreachable!(),
            });
        }
        self.render_graph(ui);
    }

    // ── common graph renderer ────────────────────────────────────────

    fn render_graph(&mut self, ui: &mut Ui) {
        let layout = match self.layout.as_ref() {
            Some(l) => l,
            None => return,
        };

        // allocate interactive region
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

        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            let old_zoom = self.zoom;
            let zoom_factor = 1.0 + scroll_delta * 0.002;
            let new_zoom = (self.zoom * zoom_factor).clamp(0.2, 5.0);
            self.zoom = new_zoom;

            // Adjust pan so the point under the cursor stays fixed.
            if let Some(cursor) = response.hover_pos() {
                let origin = response.rect.min.to_vec2();
                // World coordinate under cursor before zoom change:
                let cursor_world =
                    (cursor.to_vec2() - origin - self.pan_offset) / old_zoom;
                // New pan_offset so cursor_world maps back to cursor:
                self.pan_offset = cursor.to_vec2() - origin - cursor_world * new_zoom;
            }
        }

        // Clip drawing to the canvas area so nodes don't overlap the toolbar.
        let painter = ui.painter_at(response.rect);
        let canvas_origin = response.rect.min.to_vec2() + self.pan_offset;

        // ── edges (bezier or straight) ───────────────────────────────
        for edge in &layout.edges {
            let mapped: Vec<Pos2> = edge
                .points
                .iter()
                .map(|p| {
                    Pos2::new(
                        p.x * self.zoom + canvas_origin.x,
                        p.y * self.zoom + canvas_origin.y,
                    )
                })
                .collect();

            if mapped.len() == 3 {
                // quadratic bezier
                let stroke = Stroke::new(1.5 * self.zoom, edge.color);
                let steps = 16_usize;
                let mut prev = mapped[0];
                for i in 1..=steps {
                    let t = i as f32 / steps as f32;
                    let inv = 1.0 - t;
                    let x =
                        inv * inv * mapped[0].x + 2.0 * inv * t * mapped[1].x + t * t * mapped[2].x;
                    let y =
                        inv * inv * mapped[0].y + 2.0 * inv * t * mapped[1].y + t * t * mapped[2].y;
                    let cur = Pos2::new(x, y);
                    painter.line_segment([prev, cur], stroke);
                    prev = cur;
                }
                draw_arrowhead(&painter, mapped[1], mapped[2], edge.color, self.zoom);
            } else if mapped.len() >= 2 {
                let stroke = Stroke::new(1.5 * self.zoom, edge.color);
                for w in mapped.windows(2) {
                    painter.line_segment([w[0], w[1]], stroke);
                }
                draw_arrowhead(
                    &painter,
                    mapped[mapped.len() - 2],
                    mapped[mapped.len() - 1],
                    edge.color,
                    self.zoom,
                );
            }
        }

        // ── nodes ─────────────────────────────────────────────────────
        let mono_font = FontId::monospace((12.0 * self.zoom).max(6.0));
        let header_font = FontId::monospace((13.0 * self.zoom).max(7.0));
        let hover_pos = response.hover_pos();

        let mut hovered_node: Option<u64> = None;

        for node in &layout.nodes {
            let rect = Rect::from_min_size(
                Pos2::new(
                    node.rect.min.x * self.zoom + canvas_origin.x,
                    node.rect.min.y * self.zoom + canvas_origin.y,
                ),
                node.rect.size() * self.zoom,
            );

            let is_hovered = hover_pos.is_some_and(|pos| rect.contains(pos));
            if is_hovered {
                hovered_node = Some(node.block_addr);
            }

            painter.rect_filled(rect, 4.0 * self.zoom, Color32::from_rgb(30, 30, 40));

            let outline_color = if is_hovered {
                Color32::from_rgb(140, 180, 255)
            } else {
                Color32::from_rgb(80, 80, 100)
            };
            let outline_width = if is_hovered {
                2.0 * self.zoom
            } else {
                1.0 * self.zoom
            };
            painter.rect_stroke(
                rect,
                4.0 * self.zoom,
                Stroke::new(outline_width, outline_color),
                StrokeKind::Outside,
            );

            let header_pos = Pos2::new(rect.min.x + 4.0 * self.zoom, rect.min.y + 2.0 * self.zoom);

            let header_text = format!("0x{:08x}", node.block_addr);

            painter.text(
                header_pos,
                egui::Align2::LEFT_TOP,
                &header_text,
                header_font.clone(),
                Color32::from_rgb(100, 180, 255),
            );

            let sep_y = rect.min.y + 18.0 * self.zoom;
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + 2.0 * self.zoom, sep_y),
                    Pos2::new(rect.max.x - 2.0 * self.zoom, sep_y),
                ],
                Stroke::new(0.5 * self.zoom, Color32::from_rgb(60, 60, 80)),
            );

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

        // Handle left-click on a node (only if not dragging)
        if response.clicked() && !self.dragging {
            if let Some(addr) = hovered_node {
                self.pending_navigation = Some(addr);
            }
        }

        // Track which node was right-clicked for context menu
        if response.secondary_clicked() {
            self.context_menu_node = hovered_node;
        }

        // Right-click context menu
        if let Some(node_addr) = self.context_menu_node {
            response.context_menu(|ui| {
                if ui.button("Copy Address").clicked() {
                    ui.ctx().copy_text(format!("0x{:08x}", node_addr));
                    ui.close_menu();
                }
                if ui.button("Go to Disassembly").clicked() {
                    self.pending_navigation = Some(node_addr);
                    ui.close_menu();
                }
            });
        }

        // ── zoom indicator ───────────────────────────────────────────
        painter.text(
            Pos2::new(response.rect.max.x - 10.0, response.rect.min.y + 10.0),
            egui::Align2::RIGHT_TOP,
            format!("{:.0}%", self.zoom * 100.0),
            FontId::proportional(12.0),
            Color32::from_rgb(150, 150, 150),
        );

        // ── minimap ──────────────────────────────────────────────────
        if self.show_minimap && !layout.nodes.is_empty() {
            draw_minimap(&painter, layout, &response.rect, self.pan_offset, self.zoom);
        }
    }

    // ── SVG export ───────────────────────────────────────────────────

    fn export_svg(&self, analysis: &AnalysisDatabase) -> String {
        let layout = match self.layout.as_ref() {
            Some(l) => l,
            None => return String::from("<svg/>"),
        };

        let svg_nodes: Vec<graph::SvgNode> = layout
            .nodes
            .iter()
            .map(|n| graph::SvgNode {
                x: n.rect.min.x,
                y: n.rect.min.y,
                width: n.rect.width(),
                height: n.rect.height(),
                label: match self.mode {
                    GraphMode::CallGraph => {
                        if let Some(f) = analysis.functions.get(&n.block_addr) {
                            f.name.clone()
                        } else {
                            format!("0x{:08x}", n.block_addr)
                        }
                    }
                    _ => format!("0x{:08x}", n.block_addr),
                },
                sub_lines: n
                    .instructions
                    .iter()
                    .map(|(a, t)| format!("{:08x}: {}", a, t))
                    .collect(),
            })
            .collect();

        let svg_edges: Vec<graph::SvgEdge> = layout
            .edges
            .iter()
            .map(|e| graph::SvgEdge {
                points: e.points.iter().map(|p| (p.x, p.y)).collect(),
                color: format!("#{:02x}{:02x}{:02x}", e.color.r(), e.color.g(), e.color.b()),
            })
            .collect();

        graph::export_svg(&graph::SvgGraph {
            nodes: svg_nodes,
            edges: svg_edges,
        })
    }
}

fn draw_arrowhead(painter: &egui::Painter, from: Pos2, to: Pos2, color: Color32, zoom: f32) {
    let dir = (to - from).normalized();
    if !dir.x.is_finite() || !dir.y.is_finite() {
        return;
    }
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

fn draw_minimap(
    painter: &egui::Painter,
    layout: &GraphLayout,
    canvas_rect: &Rect,
    pan_offset: Vec2,
    zoom: f32,
) {
    // Compute graph extent
    let mut gmin_x = f32::MAX;
    let mut gmin_y = f32::MAX;
    let mut gmax_x = f32::MIN;
    let mut gmax_y = f32::MIN;
    for n in &layout.nodes {
        gmin_x = gmin_x.min(n.rect.min.x);
        gmin_y = gmin_y.min(n.rect.min.y);
        gmax_x = gmax_x.max(n.rect.max.x);
        gmax_y = gmax_y.max(n.rect.max.y);
    }
    let gw = (gmax_x - gmin_x).max(1.0);
    let gh = (gmax_y - gmin_y).max(1.0);

    let mm_w = 140.0_f32;
    let mm_h = 100.0_f32;
    let margin = 8.0;
    let mm_origin = Pos2::new(
        canvas_rect.max.x - mm_w - margin,
        canvas_rect.max.y - mm_h - margin,
    );
    let mm_rect = Rect::from_min_size(mm_origin, Vec2::new(mm_w, mm_h));

    // Background
    painter.rect_filled(
        mm_rect,
        2.0,
        Color32::from_rgba_premultiplied(20, 20, 30, 200),
    );
    painter.rect_stroke(
        mm_rect,
        2.0,
        Stroke::new(1.0, Color32::from_rgb(80, 80, 100)),
        StrokeKind::Outside,
    );

    let scale_x = mm_w / gw;
    let scale_y = mm_h / gh;
    let scale = scale_x.min(scale_y) * 0.9;

    // Draw node dots
    for n in &layout.nodes {
        let cx = mm_origin.x + (n.rect.center().x - gmin_x) * scale + (mm_w - gw * scale) * 0.5;
        let cy = mm_origin.y + (n.rect.center().y - gmin_y) * scale + (mm_h - gh * scale) * 0.5;
        painter.circle_filled(Pos2::new(cx, cy), 2.0, Color32::from_rgb(100, 180, 255));
    }

    // Viewport rectangle
    let off_x = (mm_w - gw * scale) * 0.5;
    let off_y = (mm_h - gh * scale) * 0.5;

    // Compute visible area in graph coordinates
    let vis_left = -pan_offset.x / zoom;
    let vis_top = -pan_offset.y / zoom;
    let vis_w = canvas_rect.width() / zoom;
    let vis_h = canvas_rect.height() / zoom;

    let vr = Rect::from_min_size(
        Pos2::new(
            mm_origin.x + (vis_left - gmin_x) * scale + off_x,
            mm_origin.y + (vis_top - gmin_y) * scale + off_y,
        ),
        Vec2::new(vis_w * scale, vis_h * scale),
    )
    .intersect(mm_rect);

    if vr.is_positive() {
        painter.rect_stroke(
            vr,
            1.0,
            Stroke::new(1.0, Color32::from_rgb(200, 200, 100)),
            StrokeKind::Outside,
        );
    }
}

fn is_conditional_branch(m: &str) -> bool {
    let lower = m.to_ascii_lowercase();
    if matches!(lower.as_str(), "jmp" | "b" | "bx") {
        return false;
    }
    is_branch_mnemonic(&lower)
}

fn build_cfg_layout(func: &Function) -> GraphLayout {
    if func.blocks.is_empty() {
        return GraphLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    }

    // Sugiyama layering
    let layers = graph::longest_path_layering(func.entry_addr, &func.blocks);

    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut layer_blocks: Vec<Vec<u64>> = vec![Vec::new(); max_layer + 1];
    for (&addr, &layer) in &layers {
        layer_blocks[layer].push(addr);
    }
    for l in &mut layer_blocks {
        l.sort();
    }

    // Crossing minimization (2 passes)
    graph::minimize_crossings(&mut layer_blocks, &func.blocks, 2);

    // ── node sizing ──────────────────────────────────────────────────
    let char_width = 7.5_f32;
    let line_height = 15.0_f32;
    let header_height = 22.0_f32;
    let padding = 8.0_f32;
    let min_node_width = 200.0_f32;
    let max_node_width = 450.0_f32;

    struct NodeInfo {
        instructions: Vec<(u64, String)>,
        width: f32,
        height: f32,
    }

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

    // ── positioning ──────────────────────────────────────────────────
    let layer_gap = 60.0_f32;
    let node_gap = 40.0_f32;

    let mut layer_y: Vec<f32> = Vec::new();
    let mut y_cursor = 20.0_f32;
    for layer in &layer_blocks {
        layer_y.push(y_cursor);
        let max_h = layer
            .iter()
            .filter_map(|a| node_infos.get(a))
            .map(|i| i.height)
            .fold(0.0_f32, f32::max);
        y_cursor += max_h + layer_gap;
    }

    let mut node_positions: HashMap<u64, (f32, f32)> = HashMap::new();
    for (li, layer) in layer_blocks.iter().enumerate() {
        let total_w: f32 = layer
            .iter()
            .filter_map(|a| node_infos.get(a))
            .map(|i| i.width)
            .sum::<f32>()
            + layer.len().saturating_sub(1) as f32 * node_gap;
        let mut x = -total_w / 2.0;
        for &addr in layer {
            if let Some(info) = node_infos.get(&addr) {
                node_positions.insert(addr, (x, layer_y[li]));
                x += info.width + node_gap;
            }
        }
    }

    // ── build node layouts ───────────────────────────────────────────
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

    // ── build edge layouts (quadratic bezier) ────────────────────────
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

        let last_insn = match block.instructions.last() {
            Some(insn) => insn,
            None => continue,
        };
        let is_branch = is_branch_mnemonic(&last_insn.mnemonic);
        let is_cond = is_branch && is_conditional_branch(&last_insn.mnemonic);
        let branch_target = if is_branch {
            parse_target_address(&last_insn.operands)
        } else {
            None
        };

        let num_succs = block.successors.len();
        for (si, &succ_addr) in block.successors.iter().enumerate() {
            let to_info = match node_infos.get(&succ_addr) {
                Some(i) => i,
                None => continue,
            };
            let &(to_x, to_y) = match node_positions.get(&succ_addr) {
                Some(p) => p,
                None => continue,
            };

            let edge_type = if is_cond {
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

            // Offset departure for multiple successors
            let x_offset = if num_succs > 1 {
                let spread = from_info.width * 0.3;
                let t = si as f32 / (num_succs - 1) as f32 - 0.5;
                t * spread
            } else {
                0.0
            };

            let from_pt = Pos2::new(
                from_x + from_info.width / 2.0 + x_offset,
                from_y + from_info.height,
            );
            let to_pt = Pos2::new(to_x + to_info.width / 2.0, to_y);

            // Use a quadratic bezier with control point at the midpoint horizontally,
            // biased toward the departure side vertically.
            let ctrl = Pos2::new(
                (from_pt.x + to_pt.x) / 2.0 + x_offset * 0.5,
                (from_pt.y + to_pt.y) / 2.0,
            );
            let points = vec![from_pt, ctrl, to_pt];

            edges.push(EdgeLayout { points, color });
        }
    }

    GraphLayout { nodes, edges }
}

fn build_dominance_layout(func: &Function) -> GraphLayout {
    if func.blocks.is_empty() {
        return GraphLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    }

    let idom = graph::compute_dominators(func.entry_addr, &func.blocks);
    let tree = graph::build_dominator_tree(&idom);

    // BFS from entry to assign layers
    let mut layer_map: HashMap<u64, usize> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(func.entry_addr);
    layer_map.insert(func.entry_addr, 0);

    while let Some(addr) = queue.pop_front() {
        let layer = layer_map[&addr];
        if let Some(children) = tree.get(&addr) {
            for &c in children {
                layer_map.entry(c).or_insert_with(|| {
                    queue.push_back(c);
                    layer + 1
                });
            }
        }
    }

    // Add orphans (blocks not in the tree at all)
    let max_layer = layer_map.values().copied().max().unwrap_or(0);
    for b in &func.blocks {
        layer_map.entry(b.start_addr).or_insert(max_layer + 1);
    }

    // Group by layer
    let ml = layer_map.values().copied().max().unwrap_or(0);
    let mut layer_nodes: Vec<Vec<u64>> = vec![Vec::new(); ml + 1];
    for (&a, &l) in &layer_map {
        layer_nodes[l].push(a);
    }
    for l in &mut layer_nodes {
        l.sort();
    }

    // Build node infos (simpler: just address label)
    let char_width = 7.5_f32;
    let header_height = 22.0_f32;
    let padding = 8.0_f32;
    let line_height = 15.0_f32;
    let min_w = 160.0_f32;

    struct NInfo {
        w: f32,
        h: f32,
        insns: Vec<(u64, String)>,
    }

    let block_map: HashMap<u64, &kiln_core::model::BasicBlock> =
        func.blocks.iter().map(|b| (b.start_addr, b)).collect();

    let mut ninfos: HashMap<u64, NInfo> = HashMap::new();
    for &addr in layer_map.keys() {
        let insns: Vec<(u64, String)> = if let Some(blk) = block_map.get(&addr) {
            blk.instructions
                .iter()
                .map(|i| {
                    let t = if i.operands.is_empty() {
                        i.mnemonic.clone()
                    } else {
                        format!("{} {}", i.mnemonic, i.operands)
                    };
                    (i.address, t)
                })
                .collect()
        } else {
            vec![]
        };
        let max_len = insns
            .iter()
            .map(|(a, t)| format!("{:08x}: {}", a, t).len())
            .max()
            .unwrap_or(16);
        let w = (max_len as f32 * char_width + padding * 2.0).max(min_w);
        let h = header_height + insns.len() as f32 * line_height + padding;
        ninfos.insert(addr, NInfo { w, h, insns });
    }

    // Position
    let layer_gap = 60.0_f32;
    let node_gap = 40.0_f32;
    let mut layer_y: Vec<f32> = Vec::new();
    let mut yc = 20.0_f32;
    for layer in &layer_nodes {
        layer_y.push(yc);
        let mh = layer
            .iter()
            .filter_map(|a| ninfos.get(a))
            .map(|i| i.h)
            .fold(0.0_f32, f32::max);
        yc += mh + layer_gap;
    }

    let mut positions: HashMap<u64, (f32, f32)> = HashMap::new();
    for (li, layer) in layer_nodes.iter().enumerate() {
        let tw: f32 = layer
            .iter()
            .filter_map(|a| ninfos.get(a))
            .map(|i| i.w)
            .sum::<f32>()
            + layer.len().saturating_sub(1) as f32 * node_gap;
        let mut x = -tw / 2.0;
        for &a in layer {
            if let Some(ni) = ninfos.get(&a) {
                positions.insert(a, (x, layer_y[li]));
                x += ni.w + node_gap;
            }
        }
    }

    let mut nodes = Vec::new();
    for (&a, ni) in &ninfos {
        if let Some(&(x, y)) = positions.get(&a) {
            nodes.push(NodeLayout {
                block_addr: a,
                rect: Rect::from_min_size(Pos2::new(x, y), Vec2::new(ni.w, ni.h)),
                instructions: ni.insns.clone(),
            });
        }
    }

    // Edges from idom
    let dom_color = Color32::from_rgb(180, 120, 255);
    let mut edges = Vec::new();
    for (&child, &parent) in &idom {
        let pni = match ninfos.get(&parent) {
            Some(n) => n,
            None => continue,
        };
        let cni = match ninfos.get(&child) {
            Some(n) => n,
            None => continue,
        };
        let &(px, py) = match positions.get(&parent) {
            Some(p) => p,
            None => continue,
        };
        let &(cx, cy) = match positions.get(&child) {
            Some(p) => p,
            None => continue,
        };
        let from_pt = Pos2::new(px + pni.w / 2.0, py + pni.h);
        let to_pt = Pos2::new(cx + cni.w / 2.0, cy);
        let ctrl = Pos2::new((from_pt.x + to_pt.x) / 2.0, (from_pt.y + to_pt.y) / 2.0);
        edges.push(EdgeLayout {
            points: vec![from_pt, ctrl, to_pt],
            color: dom_color,
        });
    }

    GraphLayout { nodes, edges }
}

fn build_call_graph_layout(analysis: &AnalysisDatabase) -> GraphLayout {
    let (cg_nodes, cg_edges) = graph::extract_call_graph(&analysis.functions, &analysis.xrefs);
    let positions = graph::layout_call_graph(&cg_nodes, &cg_edges);

    let node_w = 180.0_f32;
    let node_h = 40.0_f32;

    let nodes: Vec<NodeLayout> = cg_nodes
        .iter()
        .filter_map(|n| {
            let &(x, y) = positions.get(&n.addr)?;
            Some(NodeLayout {
                block_addr: n.addr,
                rect: Rect::from_min_size(Pos2::new(x, y), Vec2::new(node_w, node_h)),
                instructions: vec![(n.addr, n.name.clone())],
            })
        })
        .collect();

    let call_color = Color32::from_rgb(200, 160, 60);
    let edges: Vec<EdgeLayout> = cg_edges
        .iter()
        .filter_map(|e| {
            let &(fx, fy) = positions.get(&e.from)?;
            let &(tx, ty) = positions.get(&e.to)?;
            let from_pt = Pos2::new(fx + node_w / 2.0, fy + node_h);
            let to_pt = Pos2::new(tx + node_w / 2.0, ty);
            let ctrl = Pos2::new((from_pt.x + to_pt.x) / 2.0, (from_pt.y + to_pt.y) / 2.0);
            Some(EdgeLayout {
                points: vec![from_pt, ctrl, to_pt],
                color: call_color,
            })
        })
        .collect();

    GraphLayout { nodes, edges }
}
