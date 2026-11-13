//! The blocks editor: category chips, palette, and a canvas where blocks snap together.

use crate::blocks::*;
use crate::model::*;
use crate::phone::c32;
use egui::*;
use egui::Shape;
use crate::blocks::Shape as BS;
#[allow(unused_imports)]
use crate::model::{Event, Widget};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub enum StackLoc {
    Top,
    Loose(usize),
    Sub1(u64),
    Sub2(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Stack(StackLoc, usize),
    Arg(u64, usize),
}

pub struct Drag {
    /// The grabbed block (and those under it): it can snap into place.
    pub blocks: Vec<Block>,
    pub grab: Vec2,
    /// The rest of a multi-selection: each keeps its offset from the grabbed
    /// block and lands loose where it is dropped, so the layout survives.
    pub rest: Vec<(Vec2, Vec<Block>)>,
}

enum LDialog {
    AddVar { name: String, ty: VarType },
    AddList { name: String, ty: ListType },
    RemoveVars,
    /// Make a more block, or change one (`editing`: its name now). Each input
    /// remembers which old input it was, so blocks using it keep their values.
    MoreBlock { name: String, params: Vec<(String, VarType, Option<usize>)>, editing: Option<String> },
}

pub struct LogicState {
    pub drag: Option<Drag>,
    rects: HashMap<u64, Rect>,
    pan: Vec2,
    pub cat: usize,
    ghost: Rect,
    dialog: Option<LDialog>,
    error: String,
    /// Blocks the pretend phone pauses before (right-click a block to set one).
    pub breakpoints: HashSet<u64>,
    /// The category whose blocks show under it (false: all folded).
    pub cat_open: bool,
    /// Text typed in the palette's search box.
    pub search: String,
    /// Selected blocks (click, shift/⌘-click, or drag a box on the canvas).
    pub selected: HashSet<u64>,
    /// Where a selection box started (screen position), while dragging one.
    marquee: Option<Pos2>,
    /// The canvas is being moved with Space + drag.
    panning: bool,
    /// The project's Supabase link (copied in each frame), the form that
    /// changes it, and a change to save into the project.
    supabase: Option<SupabaseLink>,
    supa_form: Option<SupabaseLink>,
    supa_save: Option<Option<SupabaseLink>>,
    /// The sound file picker was asked for.
    pick_sound: bool,
}

impl LogicState {
    pub fn with_cat(cat: usize) -> Self {
        LogicState { cat, ..Default::default() }
    }
}

impl Default for LogicState {
    fn default() -> Self {
        LogicState { drag: None, rects: HashMap::new(), pan: Vec2::ZERO, cat: 0, ghost: Rect::NOTHING, dialog: None, error: String::new(), breakpoints: HashSet::new(), cat_open: true, search: String::new(), selected: HashSet::new(), marquee: None, panning: false, supabase: None, supa_form: None, supa_save: None, pick_sound: false }
    }
}

/// What a render pass needs to know about the screen (copied, so blocks can be edited).
struct Info {
    menus: HashMap<String, Vec<String>>,
    var_types: HashMap<String, VarType>,
    /// Each more block's call spec, by name.
    more: HashMap<String, String>,
}

impl Info {
    fn new(s: &Screen, screens: &crate::blocks::Choices) -> Self {
        let mut menus = HashMap::new();
        for k in ["view", "textview", "edittext", "checkbox", "seekbar", "listspn", "webview", "imageview", "image", "sound", "font", "asset", "videoview", "video", "player", "varBool", "varNum", "varStr", "varMap", "listNum", "listStr", "listMap", "list", "timer", "dialog", "intent", "file", "vibrator", "calendar", "web", "camera", "mediaKind", "method", "screen", "visible", "color"] {
            menus.insert(k.to_string(), menu_choices(k, s, screens));
        }
        let more = s.events.iter().filter(|e| e.target == "moreBlock").map(|e| (e.name.clone(), more_spec(&e.params))).collect();
        Info { menus, var_types: s.vars.iter().map(|v| (v.name.clone(), v.ty)).collect(), more }
    }
    fn shape(&self, b: &Block) -> BS {
        if b.op == "getVar" {
            if let Some(Arg::Text(n)) = b.args.first() {
                return match self.var_types.get(n) {
                    Some(VarType::Bool) => BS::Bool,
                    Some(VarType::Num) => BS::Num,
                    _ => BS::Str,
                };
            }
        }
        def(&b.op).map(|d| d.shape).unwrap_or(BS::Stmt)
    }
}

struct R<'a> {
    info: &'a Info,
    interactive: bool,
    prev: &'a HashMap<u64, Rect>,
    rects: HashMap<u64, Rect>,
    targets: Vec<(Rect, Target, Option<Param>)>,
    take: Option<(u64, Vec2)>,
    clip: Rect,
    brk: &'a HashSet<u64>,
    toggle_brk: Option<u64>,
    /// Selected blocks get an outline.
    sel: &'a HashSet<u64>,
    /// A block clicked this frame.
    click: Option<u64>,
    /// A block right-clicked this frame.
    right: Option<u64>,
    /// A choice made in a block's right-click menu.
    menu: Option<BlockAct>,
}

/// What the block menu and the keyboard shortcuts do to the selection.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BlockAct {
    Duplicate,
    Copy,
    Cut,
    Delete,
    SelectAll,
    Breakpoint(u64),
}

/// Outline of selected blocks.
const SEL_COLOR: Color32 = Color32::from_rgb(0xFF, 0xB3, 0x00);

/// Marks blocks put on the system clipboard, so they paste back as blocks.
const CLIP_TAG: &str = "blockwise-blocks:";

fn ctx_item(ui: &mut Ui, icon: &str, text: &str, keys: &str) -> bool {
    let r = ui.add(Button::new(crate::icons::lbl(icon, text)).shortcut_text(keys).min_size(vec2(200.0, 0.0)));
    if r.clicked() {
        ui.close_menu();
        return true;
    }
    false
}

/// The right-click menu of a block.
fn block_menu(ui: &mut Ui, id: u64, statement: bool, out: &mut Option<BlockAct>) {
    use crate::icons as ic;
    if ctx_item(ui, ic::COPY_PLUS, "Duplicate", "⌘D") {
        *out = Some(BlockAct::Duplicate);
    }
    if ctx_item(ui, ic::COPY, "Copy", "⌘C") {
        *out = Some(BlockAct::Copy);
    }
    if ctx_item(ui, ic::SCISSORS, "Cut", "⌘X") {
        *out = Some(BlockAct::Cut);
    }
    ui.separator();
    if statement && ctx_item(ui, ic::CIRCLE_PAUSE, "Pause here (breakpoint)", "") {
        *out = Some(BlockAct::Breakpoint(id));
    }
    if ctx_item(ui, ic::TRASH, "Delete", "Del") {
        *out = Some(BlockAct::Delete);
    }
}

const LABEL: f32 = 13.0;

fn label(ui: &mut Ui, t: &str) {
    ui.add(Label::new(RichText::new(t).color(Color32::WHITE).size(LABEL)).selectable(false));
}

fn darker(c: Color32) -> Color32 {
    Color32::from_rgb((c.r() as f32 * 0.78) as u8, (c.g() as f32 * 0.78) as u8, (c.b() as f32 * 0.78) as u8)
}

fn default_arg(op: &str, i: usize) -> String {
    if let Some(d) = def(op) {
        for t in parse_spec(d.spec) {
            if let Token::Param(j, p) = t {
                if j == i {
                    return if p == Param::Num { "0".into() } else { String::new() };
                }
            }
        }
    }
    String::new()
}

fn accepts(p: &Param, s: BS) -> bool {
    matches!((p, s), (Param::Text, BS::Str) | (Param::Num, BS::Num) | (Param::Bool, BS::Bool))
}

fn text_slot(ui: &mut Ui, t: &mut String, id: Id, round: f32, interactive: bool) -> Rect {
    let f = FontId::proportional(12.0);
    let w = ui.painter().layout_no_wrap(t.clone(), f.clone(), Color32::BLACK).size().x.max(14.0) + 6.0;
    let bg = ui.painter().add(Shape::Noop);
    let rect = if interactive {
        ui.add(TextEdit::singleline(t).id(id).frame(false).font(f).text_color(Color32::BLACK).desired_width(w).margin(vec2(5.0, 2.0))).rect
    } else {
        let (r, _) = ui.allocate_exact_size(vec2(w + 10.0, 19.0), Sense::hover());
        ui.painter().text(r.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, t.as_str(), f, Color32::BLACK);
        r
    };
    ui.painter().set(bg, Shape::rect_filled(rect, round, Color32::WHITE));
    rect
}

fn menu_slot(ui: &mut Ui, t: &mut String, kind: &str, id: Id, choices: &[String], interactive: bool, block_color: Color32) -> Rect {
    let shown = if t.is_empty() { " ".to_string() } else { menu_label(kind, t) };
    let resp = ui.add(Button::new(RichText::new(format!("{shown}  {}", crate::icons::CHEVRON_DOWN)).color(Color32::WHITE).size(12.0)).fill(darker(block_color)).stroke(Stroke::NONE).rounding(3.0).sense(if interactive { Sense::click() } else { Sense::hover() }));
    if interactive {
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(id));
        }
        popup::popup_below_widget(ui, id, &resp, PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(100.0);
            if choices.is_empty() {
                ui.label(RichText::new("Nothing to choose yet").italics());
            }
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for c in choices {
                    if ui.selectable_label(c == t, menu_label(kind, c)).clicked() {
                        *t = c.clone();
                    }
                }
            });
        });
    }
    resp.rect
}

fn tokens(ui: &mut Ui, b: &mut Block, spec: &str, color: Color32, r: &mut R) {
    let id = b.id;
    let op = b.op.clone();
    for tok in parse_spec(spec) {
        match tok {
            Token::Label(s) => label(ui, &s),
            Token::ArgLabel(n) => {
                let t = match b.args.get(n) {
                    Some(Arg::Text(t)) => t.clone(),
                    _ => String::new(),
                };
                label(ui, &t);
            }
            Token::Param(i, p) => {
                while b.args.len() <= i {
                    let d = if op == "moreBlock" {
                        if p == Param::Num && b.args.len() == i { "0".to_string() } else { String::new() }
                    } else {
                        default_arg(&op, b.args.len())
                    };
                    b.args.push(Arg::Text(d));
                }
                match &mut b.args[i] {
                    Arg::Block(child) => {
                        block(ui, child, r);
                    }
                    Arg::Text(t) => {
                        let rect = match &p {
                            Param::Text => text_slot(ui, t, Id::new(("arg", id, i)), 3.0, r.interactive),
                            Param::Num => text_slot(ui, t, Id::new(("arg", id, i)), 9.0, r.interactive),
                            Param::Bool => {
                                let (rect, _) = ui.allocate_exact_size(vec2(30.0, 18.0), Sense::hover());
                                ui.painter().add(hexagon(rect, darker(color)));
                                rect
                            }
                            Param::Menu(k) => {
                                let empty = vec![];
                                let ch = r.info.menus.get(k).unwrap_or(&empty);
                                menu_slot(ui, t, k, Id::new(("menu", id, i)), ch, r.interactive, color)
                            }
                        };
                        if r.interactive && !matches!(p, Param::Menu(_)) {
                            r.targets.push((rect, Target::Arg(id, i), Some(p.clone())));
                        }
                    }
                }
            }
        }
        ui.add_space(4.0);
    }
}

fn hexagon(r: Rect, c: Color32) -> Shape {
    let h = r.height() / 2.0;
    Shape::convex_polygon(
        vec![pos2(r.left(), r.center().y), pos2(r.left() + h, r.top()), pos2(r.right() - h, r.top()), pos2(r.right(), r.center().y), pos2(r.right() - h, r.bottom()), pos2(r.left() + h, r.bottom())],
        c,
        Stroke::NONE,
    )
}

/// Draw one block (and what's inside it). Returns its rect.
fn block(ui: &mut Ui, b: &mut Block, r: &mut R) -> Rect {
    let d = def(&b.op);
    let shape = r.info.shape(b);
    let color = if b.op == "getVar" { c32(0xFF00_0000 | CATEGORIES[0].color) } else { c32(0xFF00_0000 | color_of(b)) };
    // a more block call shows its inputs, from the definition
    let more_spec = if b.op == "moreBlock" {
        match b.args.first() {
            Some(Arg::Text(n)) => r.info.more.get(n).cloned(),
            _ => None,
        }
    } else {
        None
    };
    let spec = more_spec.as_deref().unwrap_or_else(|| d.map(|d| d.spec).unwrap_or("unknown block"));
    let spec2 = d.map(|d| d.spec2).unwrap_or("");
    if r.interactive {
        if let Some(prev) = r.prev.get(&b.id).copied() {
            if prev.intersects(r.clip) {
                let resp = ui.interact(prev, Id::new(("blk", b.id)), Sense::click_and_drag());
                if resp.drag_started_by(PointerButton::Primary) {
                    let p = resp.interact_pointer_pos().unwrap_or(prev.min);
                    r.take = Some((b.id, p - prev.min));
                }
                if resp.clicked() {
                    r.click = Some(b.id);
                }
                if resp.secondary_clicked() {
                    r.right = Some(b.id);
                }
                let (id, statement) = (b.id, !shape.is_reporter());
                let menu = &mut r.menu;
                resp.context_menu(|ui| block_menu(ui, id, statement, menu));
            }
        }
    }
    let bg = ui.painter().add(Shape::Noop);
    let reporter = shape.is_reporter();
    let mut header = Rect::NOTHING;
    let mut arms: Vec<(Rect, Rect)> = vec![];
    let resp = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let margin = if reporter { Margin { left: 7.0, right: 3.0, top: 2.0, bottom: 2.0 } } else { Margin { left: 8.0, right: 6.0, top: 5.0, bottom: 7.0 } };
        let h = Frame::none().inner_margin(margin).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if !reporter {
                    ui.set_min_height(20.0);
                }
                // minimum widths of each block shape
                let min_w = match shape {
                    BS::Bool | BS::Num | BS::Str => 30.0 - 10.0,
                    BS::C | BS::E => 90.0 - 14.0,
                    _ => 50.0 - 14.0,
                };
                ui.set_min_width(min_w);
                tokens(ui, b, spec, color, r);
            });
        });
        header = h.response.rect;
        if matches!(shape, BS::C | BS::E) {
            let id = b.id;
            let m1 = mouth(ui, &mut b.sub1, StackLoc::Sub1(id), r);
            if shape == BS::E {
                let eh = Frame::none().inner_margin(Margin { left: 8.0, right: 6.0, top: 3.0, bottom: 5.0 }).show(ui, |ui| {
                    ui.horizontal(|ui| label(ui, spec2));
                });
                let m2 = mouth(ui, &mut b.sub2, StackLoc::Sub2(id), r);
                arms.push((m1, eh.response.rect));
                arms.push((m2, Rect::NOTHING));
            } else {
                arms.push((m1, Rect::NOTHING));
            }
            ui.allocate_exact_size(vec2(header.width().max(60.0), 14.0), Sense::hover());
        }
    });
    let rect = resp.response.rect;
    let p = ui.painter();
    let outline = Stroke::new(1.0_f32, darker(color));
    let shapes = if reporter {
        let rr = rect;
        // the shape says what a block gives back, matching the slots it fits:
        // true/false a hexagon, a number a round pill, text a rectangle
        match shape {
            BS::Bool => vec![hexagon(rr, color)],
            BS::Str => vec![Shape::rect_filled(rr, 3.0, color), Shape::rect_stroke(rr, 3.0, outline)],
            _ => vec![Shape::rect_filled(rr, rr.height() / 2.0, color), Shape::rect_stroke(rr, rr.height() / 2.0, outline)],
        }
    } else if arms.is_empty() {
        let mut v = vec![Shape::rect_filled(rect, 3.0, color)];
        if shape != BS::Cap {
            v.push(Shape::rect_filled(Rect::from_min_size(rect.left_bottom() + vec2(12.0, 0.0), vec2(16.0, 3.0)), 1.0, color));
        }
        v
    } else {
        let w = header.width().max(60.0);
        let mut v = vec![Shape::rect_filled(Rect::from_min_size(rect.min, vec2(w, header.height())), 3.0, color)];
        for (m, after) in &arms {
            v.push(Shape::rect_filled(Rect::from_min_max(pos2(rect.left(), m.top() - 1.0), pos2(rect.left() + 14.0, m.bottom() + 1.0)), 0.0, color));
            if *after != Rect::NOTHING {
                v.push(Shape::rect_filled(Rect::from_min_size(pos2(rect.left(), after.top()), vec2(w, after.height())), 2.0, color));
            }
        }
        let last = arms.last().map(|a| a.0.bottom()).unwrap_or(rect.bottom() - 14.0);
        v.push(Shape::rect_filled(Rect::from_min_max(pos2(rect.left(), last), pos2(rect.left() + w, rect.bottom())), 3.0, color));
        v.push(Shape::rect_filled(Rect::from_min_size(rect.left_bottom() + vec2(12.0, 0.0), vec2(16.0, 3.0)), 1.0, color));
        v
    };
    p.set(bg, Shape::Vec(shapes));
    if r.sel.contains(&b.id) {
        let round = if reporter && shape != BS::Str { rect.height() / 2.0 + 2.0 } else { 5.0 };
        p.rect_stroke(rect.expand(2.0), round, Stroke::new(2.5_f32, SEL_COLOR));
    }
    if r.brk.contains(&b.id) {
        // breakpoint: a red dot on the block's left edge
        let c = pos2(rect.left() - 1.0, header.center().y);
        p.circle_filled(c, 6.0, Color32::from_rgb(0xE5, 0x39, 0x35));
        p.circle_stroke(c, 6.0, Stroke::new(1.5_f32, Color32::WHITE));
    }
    r.rects.insert(b.id, rect);
    rect
}

fn mouth(ui: &mut Ui, blocks: &mut Vec<Block>, loc: StackLoc, r: &mut R) -> Rect {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            stack(ui, blocks, loc, r);
        });
    })
    .response
    .rect
}

fn stack(ui: &mut Ui, blocks: &mut Vec<Block>, loc: StackLoc, r: &mut R) {
    if blocks.is_empty() {
        let (rect, _) = ui.allocate_exact_size(vec2(60.0, 22.0), Sense::hover());
        if r.interactive {
            r.targets.push((rect.expand2(vec2(0.0, 4.0)), Target::Stack(loc, 0), None));
        }
        return;
    }
    let n = blocks.len();
    for (i, b) in blocks.iter_mut().enumerate() {
        let top = ui.cursor().min;
        if r.interactive {
            r.targets.push((Rect::from_min_size(top - vec2(12.0, 12.0), vec2(200.0, 24.0)), Target::Stack(loc.clone(), i), None));
        }
        block(ui, b, r);
        if i + 1 == n && r.interactive && def(&b.op).map(|d| d.shape) != Some(BS::Cap) {
            let bottom = ui.cursor().min;
            r.targets.push((Rect::from_min_size(bottom - vec2(12.0, 12.0), vec2(200.0, 30.0)), Target::Stack(loc.clone(), n), None));
        }
    }
}

fn hat(ui: &mut Ui, title: &str, color: Color32) -> Rect {
    let bg = ui.painter().add(Shape::Noop);
    let fr = Frame::none().inner_margin(Margin { left: 10.0, right: 14.0, top: 12.0, bottom: 8.0 }).show(ui, |ui| {
        ui.set_min_width(90.0);
        label(ui, title);
    });
    let r = fr.response.rect;
    ui.painter().set(
        bg,
        Shape::Vec(vec![
            Shape::rect_filled(r, Rounding { nw: 14.0, ne: 14.0, sw: 3.0, se: 3.0 }, color),
            Shape::rect_filled(Rect::from_min_size(r.left_bottom() + vec2(12.0, 0.0), vec2(16.0, 3.0)), 1.0, color),
        ]),
    );
    r
}

// ---------- tree edits

fn find_in_vec(v: &mut [Block], id: u64) -> Option<&mut Block> {
    for b in v.iter_mut() {
        if b.contains_id(id) {
            return find_in_block(b, id);
        }
    }
    None
}

fn find_in_block(b: &mut Block, id: u64) -> Option<&mut Block> {
    if b.id == id {
        return Some(b);
    }
    for a in b.args.iter_mut() {
        if let Arg::Block(c) = a {
            if c.contains_id(id) {
                return find_in_block(c, id);
            }
        }
    }
    if b.sub1.iter().any(|x| x.contains_id(id)) {
        return find_in_vec(&mut b.sub1, id);
    }
    find_in_vec(&mut b.sub2, id)
}

fn find_block(ev: &mut Event, id: u64) -> Option<&mut Block> {
    if ev.blocks.iter().any(|b| b.contains_id(id)) {
        return find_in_vec(&mut ev.blocks, id);
    }
    for l in ev.loose.iter_mut() {
        if l.blocks.iter().any(|b| b.contains_id(id)) {
            return find_in_vec(&mut l.blocks, id);
        }
    }
    None
}

fn take_from_vec(v: &mut Vec<Block>, id: u64) -> Option<Vec<Block>> {
    if let Some(i) = v.iter().position(|b| b.id == id) {
        return Some(v.split_off(i));
    }
    for b in v.iter_mut() {
        if b.contains_id(id) {
            return take_from_block(b, id);
        }
    }
    None
}

fn take_from_block(b: &mut Block, id: u64) -> Option<Vec<Block>> {
    let op = b.op.clone();
    for (i, a) in b.args.iter_mut().enumerate() {
        let hit = matches!(a, Arg::Block(c) if c.id == id);
        if hit {
            let old = std::mem::replace(a, Arg::Text(default_arg(&op, i)));
            if let Arg::Block(bx) = old {
                return Some(vec![*bx]);
            }
        }
        if let Arg::Block(c) = a {
            if c.contains_id(id) {
                return take_from_block(c, id);
            }
        }
    }
    if b.sub1.iter().any(|x| x.contains_id(id)) {
        return take_from_vec(&mut b.sub1, id);
    }
    take_from_vec(&mut b.sub2, id)
}

fn take_one_from_vec(v: &mut Vec<Block>, id: u64) -> Option<Block> {
    if let Some(i) = v.iter().position(|b| b.id == id) {
        return Some(v.remove(i));
    }
    for b in v.iter_mut() {
        if b.contains_id(id) {
            return take_one_from_block(b, id);
        }
    }
    None
}

fn take_one_from_block(b: &mut Block, id: u64) -> Option<Block> {
    let op = b.op.clone();
    for (i, a) in b.args.iter_mut().enumerate() {
        if matches!(a, Arg::Block(c) if c.id == id) {
            if let Arg::Block(bx) = std::mem::replace(a, Arg::Text(default_arg(&op, i))) {
                return Some(*bx);
            }
        }
        if let Arg::Block(c) = a {
            if c.contains_id(id) {
                return take_one_from_block(c, id);
            }
        }
    }
    if b.sub1.iter().any(|x| x.contains_id(id)) {
        return take_one_from_vec(&mut b.sub1, id);
    }
    take_one_from_vec(&mut b.sub2, id)
}

/// Takes out just one block (with what's inside it), leaving the blocks
/// under it where they are.
pub fn take_one(ev: &mut Event, id: u64) -> Option<Block> {
    let mut got = None;
    if ev.blocks.iter().any(|b| b.contains_id(id)) {
        got = take_one_from_vec(&mut ev.blocks, id);
    } else {
        for l in ev.loose.iter_mut() {
            if l.blocks.iter().any(|b| b.contains_id(id)) {
                got = take_one_from_vec(&mut l.blocks, id);
                break;
            }
        }
    }
    ev.loose.retain(|l| !l.blocks.is_empty());
    got
}

fn find_ref(v: &[Block], id: u64) -> Option<&Block> {
    for b in v {
        if b.id == id {
            return Some(b);
        }
        if b.contains_id(id) {
            for a in &b.args {
                if let Arg::Block(c) = a {
                    if let Some(f) = find_ref(std::slice::from_ref(c.as_ref()), id) {
                        return Some(f);
                    }
                }
            }
            return find_ref(&b.sub1, id).or_else(|| find_ref(&b.sub2, id));
        }
    }
    None
}

fn find_in_event(ev: &Event, id: u64) -> Option<&Block> {
    find_ref(&ev.blocks, id).or_else(|| ev.loose.iter().find_map(|l| find_ref(&l.blocks, id)))
}

/// Every block of an event in reading order, with the block it sits in (0: none).
fn walk_ids(ev: &Event) -> Vec<(u64, u64)> {
    fn go(b: &Block, parent: u64, out: &mut Vec<(u64, u64)>) {
        out.push((b.id, parent));
        for a in &b.args {
            if let Arg::Block(c) = a {
                go(c, b.id, out);
            }
        }
        for c in b.sub1.iter().chain(b.sub2.iter()) {
            go(c, b.id, out);
        }
    }
    let mut out = vec![];
    for b in ev.blocks.iter().chain(ev.loose.iter().flat_map(|l| l.blocks.iter())) {
        go(b, 0, &mut out);
    }
    out
}

/// The selected blocks that aren't inside another selected block, in reading order.
pub fn selection_roots(ev: &Event, sel: &HashSet<u64>) -> Vec<u64> {
    let all = walk_ids(ev);
    let parent: HashMap<u64, u64> = all.iter().copied().collect();
    let inside_selected = |mut id: u64| {
        for _ in 0..500 {
            match parent.get(&id) {
                Some(&p) if p != 0 => {
                    if sel.contains(&p) {
                        return true;
                    }
                    id = p;
                }
                _ => return false,
            }
        }
        false
    };
    all.iter().map(|x| x.0).filter(|id| sel.contains(id) && !inside_selected(*id)).collect()
}

/// The top blocks of every stack (selecting them selects everything).
fn all_top_ids(ev: &Event) -> HashSet<u64> {
    ev.blocks.iter().chain(ev.loose.iter().flat_map(|l| l.blocks.iter())).map(|b| b.id).collect()
}

/// Puts blocks on the canvas at `at` (canvas coordinates): statements in one
/// stack, each reporter on its own. Returns the new blocks' ids.
fn drop_loose(ev: &mut Event, info: &Info, at: Vec2, blocks: Vec<Block>) -> HashSet<u64> {
    let ids = blocks.iter().map(|b| b.id).collect();
    let (reps, stmts): (Vec<Block>, Vec<Block>) = blocks.into_iter().partition(|b| info.shape(b).is_reporter());
    let mut y = at.y;
    if !stmts.is_empty() {
        ev.loose.push(LooseStack { x: at.x, y, blocks: stmts });
        y += 40.0;
    }
    for (i, b) in reps.into_iter().enumerate() {
        ev.loose.push(LooseStack { x: at.x + 8.0 * i as f32, y: y + 34.0 * i as f32, blocks: vec![b] });
    }
    ids
}

/// The clipboard text for some blocks.
pub fn clip_text(blocks: &[Block]) -> String {
    format!("{CLIP_TAG}{}", serde_json::to_string(blocks).unwrap_or_default())
}

/// Blocks from clipboard text, with new ids (None: not blocks).
pub fn from_clip_text(t: &str) -> Option<Vec<Block>> {
    let v: Vec<Block> = serde_json::from_str(t.trim().strip_prefix(CLIP_TAG)?).ok()?;
    Some(v.iter().map(|b| b.fresh()).collect())
}

pub fn take_block(ev: &mut Event, id: u64) -> Option<Vec<Block>> {
    let mut got = None;
    if ev.blocks.iter().any(|b| b.contains_id(id)) {
        got = take_from_vec(&mut ev.blocks, id);
    } else {
        for l in ev.loose.iter_mut() {
            if l.blocks.iter().any(|b| b.contains_id(id)) {
                got = take_from_vec(&mut l.blocks, id);
                break;
            }
        }
    }
    ev.loose.retain(|l| !l.blocks.is_empty());
    got
}

fn stack_mut<'a>(ev: &'a mut Event, loc: &StackLoc) -> Option<&'a mut Vec<Block>> {
    match loc {
        StackLoc::Top => Some(&mut ev.blocks),
        StackLoc::Loose(i) => ev.loose.get_mut(*i).map(|l| &mut l.blocks),
        StackLoc::Sub1(id) => find_block(ev, *id).map(|b| &mut b.sub1),
        StackLoc::Sub2(id) => find_block(ev, *id).map(|b| &mut b.sub2),
    }
}

/// Put dragged blocks at a target. Gives them back if the target is gone.
pub fn place(ev: &mut Event, t: &Target, blocks: Vec<Block>) -> Result<(), Vec<Block>> {
    match t {
        Target::Stack(loc, i) => match stack_mut(ev, loc) {
            Some(v) => {
                let at = (*i).min(v.len());
                let tail = v.split_off(at);
                v.extend(blocks);
                v.extend(tail);
                Ok(())
            }
            None => Err(blocks),
        },
        Target::Arg(id, i) => match find_block(ev, *id) {
            Some(b) if blocks.len() == 1 && b.args.len() > *i => {
                let mut blocks = blocks;
                b.args[*i] = Arg::Block(Box::new(blocks.remove(0)));
                Ok(())
            }
            _ => Err(blocks),
        },
    }
}

// ---------- more blocks

fn param_type_label(t: VarType) -> &'static str {
    match t {
        VarType::Num => "number",
        VarType::Bool => "true or false",
        _ => "text",
    }
}

/// What's wrong with a more block's inputs, if anything.
pub fn params_problem(params: &[(String, VarType, Option<usize>)]) -> Option<String> {
    for (i, (n, _, _)) in params.iter().enumerate() {
        let n = n.trim();
        if n.is_empty() {
            return Some("Give each input a name".into());
        }
        if !valid_name(n) {
            return Some(format!("\"{n}\": use letters, digits and _ only, starting with a letter"));
        }
        if params[..i].iter().any(|p| p.0.trim() == n) {
            return Some(format!("Two inputs are called \"{n}\""));
        }
    }
    None
}

/// Every block in a stack, inside ones included, in order.
fn walk_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut Block)) {
    for b in blocks.iter_mut() {
        f(b);
        for a in b.args.iter_mut() {
            if let Arg::Block(c) = a {
                walk_mut(std::slice::from_mut(c.as_mut()), f);
            }
        }
        walk_mut(&mut b.sub1, f);
        walk_mut(&mut b.sub2, f);
    }
}

/// Renames a more block and changes its inputs. `from[k]` is the old input
/// the new input k was (None: new), so each use keeps its values and the
/// definition's input blocks follow their renames; an input taken away
/// leaves an empty slot where it was read.
pub fn apply_more_edit(s: &mut Screen, old: &str, name: &str, params: Vec<Var>, from: &[Option<usize>]) {
    let Some(ei) = s.events.iter().position(|e| e.target == "moreBlock" && e.name == old) else { return };
    let old_params = s.events[ei].params.clone();
    // old input name → its new name and type (None: taken away)
    let follow: HashMap<String, Option<(String, VarType)>> = old_params
        .iter()
        .enumerate()
        .map(|(j, p)| (p.name.clone(), from.iter().position(|f| *f == Some(j)).map(|k| (params[k].name.clone(), params[k].ty))))
        .collect();
    let is_param = |op: &str| matches!(op, "paramStr" | "paramNum" | "paramBool");
    let gone = |b: &Block| is_param(&b.op) && matches!(b.args.first(), Some(Arg::Text(n)) if matches!(follow.get(n), Some(None)));
    let ev = &mut s.events[ei];
    let mut fix = |b: &mut Block| {
        for a in b.args.iter_mut() {
            if matches!(a, Arg::Block(c) if gone(c)) {
                *a = Arg::Text(String::new());
            }
        }
        if is_param(&b.op) {
            if let Some(Arg::Text(n)) = b.args.first_mut() {
                if let Some(Some((nn, ty))) = follow.get(n.as_str()) {
                    *n = nn.clone();
                    b.op = param_op(*ty).into();
                }
            }
        }
    };
    walk_mut(&mut ev.blocks, &mut fix);
    for l in ev.loose.iter_mut() {
        l.blocks.retain(|b| !gone(b));
        walk_mut(&mut l.blocks, &mut fix);
    }
    ev.loose.retain(|l| !l.blocks.is_empty());
    ev.name = name.to_string();
    ev.params = params.clone();
    // every use of it, on this screen
    for e in s.events.iter_mut() {
        let mut call = |b: &mut Block| {
            if b.op == "moreBlock" && matches!(b.args.first(), Some(Arg::Text(n)) if n == old) {
                let mut args = vec![Arg::Text(name.to_string())];
                for (k, f) in from.iter().enumerate() {
                    let empty = Arg::Text(if params[k].ty == VarType::Num { "0".into() } else { String::new() });
                    args.push(f.and_then(|j| b.args.get(j + 1).cloned()).unwrap_or(empty));
                }
                b.args = args;
            }
        };
        walk_mut(&mut e.blocks, &mut call);
        for l in e.loose.iter_mut() {
            walk_mut(&mut l.blocks, &mut call);
        }
    }
}

// ---------- palette

enum PItem {
    Header(String),
    Block(Block),
}

fn palette_items(cat: usize, s: &Screen, screens: &crate::blocks::Choices, ev: Option<&Event>) -> Vec<PItem> {
    let mut out = vec![];
    match cat {
        8 => {
            // inside a more block: its inputs, to read in its blocks
            if let Some(e) = ev.filter(|e| e.target == "moreBlock" && !e.params.is_empty()) {
                out.push(PItem::Header(format!("Inputs of {}", e.name)));
                for p in &e.params {
                    out.push(PItem::Block(named_block(param_op(p.ty), &p.name)));
                }
                out.push(PItem::Header("Your blocks".into()));
            }
            for e in s.events.iter().filter(|e| e.target == "moreBlock") {
                out.push(PItem::Block(named_block("moreBlock", &e.name)));
            }
        }
        0 => {
            for v in &s.vars {
                if v.ty != VarType::Map {
                    out.push(PItem::Block(named_block("getVar", &v.name)));
                }
            }
        }
        _ => {}
    }
    for op in palette(cat) {
        if let Some(h) = op.strip_prefix('#') {
            out.push(PItem::Header(h.to_string()));
        } else {
            out.push(PItem::Block(make_block(op, s, screens)));
        }
    }
    out
}

fn valid_name(n: &str) -> bool {
    let mut c = n.chars();
    matches!(c.next(), Some(f) if f.is_ascii_alphabetic() || f == '_') && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Words Kotlin or Swift keep for themselves, and names the generated screen
/// code already uses, so they can't name a variable, widget or component.
const RESERVED_WORDS: &[&str] = &[
    "_activity", "_timer", "_ok", "_code", "_response", "router", "extras", "started", "screenTitle", "toastText", "start", "showToast", "finish", "startActivity", "performClick", "body",
    "as", "break", "case", "class", "continue", "default", "do", "else", "enum", "extension", "false", "for", "fun", "func", "guard", "if", "import", "in", "init", "interface", "is", "let", "nil", "null", "object", "operator", "package", "private", "protocol", "public", "repeat", "return", "self", "static", "struct", "super", "switch", "this", "throw", "true", "try", "typealias", "typeof", "val", "var", "when", "where", "while",
];

pub fn name_problem(s: &Screen, n: &str) -> Option<String> {
    if !valid_name(n) {
        Some("Use letters, digits and _ only, starting with a letter".into())
    } else if RESERVED_WORDS.contains(&n) {
        Some("That word is reserved".into())
    } else if n == "root" {
        // the screen itself is every top-level widget's parent "root"
        Some("\"root\" is the screen itself".into())
    } else if s.name_taken(n) || s.events.iter().any(|e| e.target == "moreBlock" && e.name == n) {
        Some("That name is already used".into())
    } else {
        None
    }
}

// ---------- the editor

/// The whole blocks editor for one event of one screen.
pub fn show(ui: &mut Ui, st: &mut LogicState, project: &mut Project, si: usize, ei: usize) -> bool {
    let mut changed = false;
    st.supabase = project.supabase.clone();
    if let Some(link) = st.supa_save.take() {
        project.supabase = link;
        st.supabase = project.supabase.clone();
        changed = true;
    }
    let screens = screen_choices(project);
    let screen = &mut project.screens[si];
    if ei >= screen.events.len() {
        return false;
    }
    let info = Info::new(screen, &screens);
    let full = ui.available_rect_before_wrap();
    let pal_w = 300.0;
    let canvas_rect = Rect::from_min_max(full.min, pos2(full.right() - pal_w, full.bottom()));
    let pal_rect = Rect::from_min_max(pos2(full.right() - pal_w, full.top()), full.max);
    ui.allocate_rect(full, Sense::hover());

    // ----- canvas
    // empty canvas: drag a box to select; Space + drag, right-drag,
    // middle-drag or scrolling moves around
    let canvas_resp = ui.interact(canvas_rect, Id::new("logic_canvas"), Sense::click_and_drag());
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    let space = !typing && ui.input(|i| i.key_down(Key::Space));
    if st.drag.is_none() {
        if canvas_resp.drag_started_by(PointerButton::Primary) {
            if space {
                st.panning = true;
            } else {
                st.marquee = ui.input(|i| i.pointer.press_origin());
            }
        }
        if canvas_resp.dragged_by(PointerButton::Secondary) || canvas_resp.dragged_by(PointerButton::Middle) || (st.panning && canvas_resp.dragged_by(PointerButton::Primary)) {
            st.pan += canvas_resp.drag_delta();
        }
        // over a block too (blocks sit on top of the canvas and take the
        // hover), but not over a menu or window opened above it
        let over = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| canvas_rect.contains(p) && ui.ctx().layer_id_at(p).map_or(true, |l| l == ui.layer_id()));
        if over {
            let d = ui.input(|i| i.smooth_scroll_delta);
            if d != Vec2::ZERO {
                st.pan += d;
            }
        }
    }
    if !ui.input(|i| i.pointer.any_down()) {
        st.panning = false;
    }
    if space && canvas_resp.hovered() {
        ui.ctx().set_cursor_icon(if st.panning { CursorIcon::Grabbing } else { CursorIcon::Grab });
    }
    {
        let p = ui.painter_at(canvas_rect);
        p.rect_filled(canvas_rect, 0.0, crate::theme::bg());
        let step = 24.0;
        let off = vec2(st.pan.x.rem_euclid(step), st.pan.y.rem_euclid(step));
        let mut y = canvas_rect.top() + off.y;
        while y < canvas_rect.bottom() {
            let mut x = canvas_rect.left() + off.x;
            while x < canvas_rect.right() {
                p.circle_filled(pos2(x, y), 1.0, Color32::from_rgb(0xC9, 0xD1, 0xE3));
                x += step;
            }
            y += step;
        }
    }
    let origin = canvas_rect.min + st.pan;
    let prev = std::mem::take(&mut st.rects);
    // breakpoints belong to the offline preview's debugger; hidden with it
    let brk = st.breakpoints.clone();
    let sel = st.selected.clone();
    let mut r = R { info: &info, interactive: st.drag.is_none(), prev: &prev, rects: HashMap::new(), targets: vec![], take: None, clip: canvas_rect, brk: &brk, toggle_brk: None, sel: &sel, click: None, right: None, menu: None };
    // keep drop targets even while dragging: render interactively but ignore new drags
    r.interactive = true;
    let ev = &mut screen.events[ei];
    let title = ev.title();
    let hat_color = if ev.target == "moreBlock" { c32(0xFF00_0000 | CATEGORIES[8].color) } else { c32(0xFF00_0000 | EVENT_COLOR) };
    {
        let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(origin + vec2(30.0, 30.0), vec2(3000.0, 3000.0))).layout(Layout::top_down(Align::Min)));
        child.set_clip_rect(canvas_rect);
        child.spacing_mut().item_spacing = vec2(0.0, 0.0);
        hat(&mut child, &title, hat_color);
        stack(&mut child, &mut ev.blocks, StackLoc::Top, &mut r);
    }
    for (li, l) in ev.loose.iter_mut().enumerate() {
        let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(origin + vec2(l.x, l.y), vec2(3000.0, 3000.0))).layout(Layout::top_down(Align::Min)));
        child.set_clip_rect(canvas_rect);
        child.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let first_reporter = l.blocks.first().map_or(false, |b| info.shape(b).is_reporter());
        if first_reporter {
            for b in l.blocks.iter_mut() {
                block(&mut child, b, &mut r);
            }
        } else {
            stack(&mut child, &mut l.blocks, StackLoc::Loose(li), &mut r);
        }
    }
    let R { rects, targets, take, toggle_brk, click, right, menu, .. } = r;
    st.rects = rects;
    if let Some(id) = toggle_brk {
        if !st.breakpoints.remove(&id) {
            st.breakpoints.insert(id);
        }
    }
    let mods = ui.input(|i| i.modifiers);
    // clicking picks a block; shift or ⌘ adds it to (or drops it from) the selection
    if let Some(id) = click {
        if mods.shift || mods.command {
            if !st.selected.remove(&id) {
                st.selected.insert(id);
            }
        } else {
            st.selected = HashSet::from([id]);
        }
    } else if canvas_resp.clicked() && !mods.shift && !mods.command {
        st.selected.clear();
    }
    if let Some(id) = right {
        if !st.selected.contains(&id) {
            st.selected = HashSet::from([id]);
        }
    }
    // the selection box
    if let Some(start) = st.marquee {
        let now = ui.ctx().pointer_latest_pos().unwrap_or(start);
        let boxr = Rect::from_two_pos(start, now);
        let lp = ui.painter_at(canvas_rect);
        lp.rect(boxr, 2.0, SEL_COLOR.gamma_multiply(0.12), Stroke::new(1.0_f32, SEL_COLOR));
        let inside: HashSet<u64> = st.rects.iter().filter(|(_, r)| boxr.contains_rect(**r)).map(|(id, _)| *id).collect();
        let inside: HashSet<u64> = selection_roots(ev, &inside).into_iter().collect();
        if !ui.input(|i| i.pointer.primary_down()) {
            if mods.shift || mods.command {
                st.selected.extend(inside);
            } else {
                st.selected = inside;
            }
            st.marquee = None;
        } else {
            // show what the box would pick
            for id in &inside {
                if let Some(r) = st.rects.get(id) {
                    lp.rect_stroke(r.expand(2.0), 5.0, Stroke::new(1.5_f32, SEL_COLOR.gamma_multiply(0.7)));
                }
            }
        }
        ui.ctx().request_repaint();
    }
    // the empty canvas's own menu
    let mut canvas_act = None;
    let has_clip = ui.ctx().memory(|m| m.data.get_temp::<bool>(Id::new("blocks_on_clipboard"))).unwrap_or(false);
    let mut paste_menu = false;
    canvas_resp.context_menu(|ui| {
        use crate::icons as ic;
        if ui.add_enabled(has_clip, Button::new(ic::lbl(ic::PASTE, "Paste")).shortcut_text("⌘V").min_size(vec2(200.0, 0.0))).clicked() {
            paste_menu = true;
            ui.close_menu();
        }
        if ctx_item(ui, ic::SQUARE_CHECK, "Select all", "⌘A") {
            canvas_act = Some(BlockAct::SelectAll);
        }
    });
    // keyboard shortcuts (not while typing in a block or a box)
    let mut act = menu.or(canvas_act);
    let mut paste_text: Option<String> = None;
    if !typing && st.dialog.is_none() && st.drag.is_none() {
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::D) {
                act = Some(BlockAct::Duplicate);
            }
            if i.consume_key(Modifiers::COMMAND, Key::A) {
                act = Some(BlockAct::SelectAll);
            }
            if i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace) {
                act = Some(BlockAct::Delete);
            }
            for e in &i.events {
                match e {
                    egui::Event::Copy => act = Some(BlockAct::Copy),
                    egui::Event::Cut => act = Some(BlockAct::Cut),
                    egui::Event::Paste(t) => paste_text = Some(t.clone()),
                    _ => {}
                }
            }
        });
    }
    if paste_menu {
        paste_text = ui.ctx().memory(|m| m.data.get_temp::<String>(Id::new("blocks_clipboard")));
    }
    if let Some(a) = act {
        let roots = selection_roots(ev, &st.selected);
        let copies = |ev: &Event| -> Vec<Block> { roots.iter().filter_map(|id| find_in_event(ev, *id).cloned()).collect() };
        match a {
            BlockAct::SelectAll => st.selected = all_top_ids(ev),
            BlockAct::Breakpoint(id) => {
                if !st.breakpoints.remove(&id) {
                    st.breakpoints.insert(id);
                }
            }
            BlockAct::Copy | BlockAct::Cut if !roots.is_empty() => {
                let text = clip_text(&copies(ev));
                ui.ctx().copy_text(text.clone());
                ui.ctx().memory_mut(|m| {
                    m.data.insert_temp(Id::new("blocks_on_clipboard"), true);
                    m.data.insert_temp(Id::new("blocks_clipboard"), text);
                });
                if a == BlockAct::Cut {
                    for id in &roots {
                        take_one(ev, *id);
                    }
                    st.selected.clear();
                    changed = true;
                }
            }
            BlockAct::Delete if !roots.is_empty() => {
                for id in &roots {
                    take_one(ev, *id);
                }
                st.selected.clear();
                changed = true;
            }
            BlockAct::Duplicate if !roots.is_empty() => {
                // each copy lands a little below and right of its original,
                // so several blocks keep their layout
                let mut ids = HashSet::new();
                for (n, (id, b)) in roots.iter().zip(copies(ev)).enumerate() {
                    let b = b.fresh();
                    let at = st.rects.get(id).map(|r| r.min - origin).unwrap_or(vec2(40.0, 120.0 + 44.0 * n as f32)) + vec2(28.0, 28.0);
                    ids.insert(b.id);
                    ev.loose.push(LooseStack { x: at.x, y: at.y, blocks: vec![b] });
                }
                st.selected = ids;
                changed = true;
            }
            _ => {}
        }
    }
    if let Some(blocks) = paste_text.as_deref().and_then(from_clip_text) {
        let pointer = ui.ctx().pointer_latest_pos().filter(|p| canvas_rect.contains(*p));
        let at = pointer.map(|p| p - origin).unwrap_or(canvas_rect.center() - origin);
        st.selected = drop_loose(ev, &info, at, blocks);
        changed = true;
    }
    // dragging a block: a selected one carries the whole selection with it;
    // Alt (Option) drags a copy and leaves the blocks where they are
    if take.is_some() && st.drag.is_none() {
        let (id, grab) = take.unwrap();
        let roots = if st.selected.contains(&id) && st.selected.len() > 1 { selection_roots(ev, &st.selected) } else { vec![] };
        let mut rest = vec![];
        let blocks = if roots.len() > 1 && roots.contains(&id) {
            // every selected block moves, each keeping its place relative to
            // the grabbed one, which sits under the pointer
            let lead_at = st.rects.get(&id).map(|r| r.min);
            let grab_one = |ev: &mut Event, i: u64| if mods.alt { find_in_event(ev, i).map(|b| b.fresh()) } else { take_one(ev, i) };
            for (n, r) in roots.iter().copied().filter(|r| *r != id).enumerate() {
                let off = match (lead_at, st.rects.get(&r)) {
                    (Some(a), Some(b)) => b.min - a,
                    _ => vec2(0.0, 44.0 * (n + 1) as f32),
                };
                if let Some(b) = grab_one(ev, r) {
                    rest.push((off, vec![b]));
                }
            }
            grab_one(ev, id).map(|b| vec![b])
        } else if mods.alt {
            // a copy of the block and the ones under it
            let mut copy = ev.clone();
            take_block(&mut copy, id).map(|v| v.iter().map(|b| b.fresh()).collect())
        } else {
            take_block(ev, id)
        };
        if let Some(blocks) = blocks.filter(|v: &Vec<Block>| !v.is_empty()) {
            st.selected = blocks.iter().chain(rest.iter().flat_map(|r| r.1.iter())).map(|b| b.id).collect();
            st.drag = Some(Drag { blocks, grab, rest });
            changed = true;
        }
    }
    // hint when empty
    if ev.blocks.is_empty() && ev.loose.is_empty() && st.drag.is_none() {
        ui.painter_at(canvas_rect).text(origin + vec2(36.0, 90.0), Align2::LEFT_TOP, "Drag blocks from the right and snap them under the event", FontId::proportional(13.0), crate::theme::gray(140));
    }
    ui.painter_at(canvas_rect).text(canvas_rect.left_bottom() + vec2(12.0, -10.0), Align2::LEFT_BOTTOM, "Drag to select  ·  Shift-click adds  ·  Alt-drag copies  ·  Scroll or Space-drag to move  ·  Right-click for more", FontId::proportional(11.0), crate::theme::gray(150));

    // ----- palette
    let pal_changed = palette_panel(ui, st, pal_rect, screen, &screens, &info, ei);
    changed |= pal_changed;
    if std::mem::take(&mut st.pick_sound) {
        if let Some(path) = rfd::FileDialog::new().set_title("Pick a sound").add_filter("Sounds", &SOUND_EXTS).pick_file() {
            match project.add_file(&path) {
                Ok(_) => {
                    st.error.clear();
                    changed = true;
                }
                Err(e) => st.error = e,
            }
        }
    }
    let screen = &mut project.screens[si];

    // ----- dragging
    let pointer = ui.ctx().pointer_latest_pos().unwrap_or(Pos2::ZERO);
    let down = ui.ctx().input(|i| i.pointer.primary_down());
    if let Some(drag) = &mut st.drag {
        let ghost_pos = pointer - drag.grab;
        let mut ghost_blocks = drag.blocks.clone();
        let ghost_prev = HashMap::new();
        let no_brk = HashSet::new();
        let resp = Area::new(Id::new("logic_ghost")).order(Order::Tooltip).fixed_pos(ghost_pos).constrain(false).interactable(false).show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.set_opacity(0.85);
            let mut gr = R { info: &info, interactive: false, prev: &ghost_prev, rects: HashMap::new(), targets: vec![], take: None, clip: Rect::EVERYTHING, brk: &no_brk, toggle_brk: None, sel: &no_brk, click: None, right: None, menu: None };
            ui.vertical(|ui| {
                for b in ghost_blocks.iter_mut() {
                    block(ui, b, &mut gr);
                }
            });
        });
        st.ghost = resp.response.rect;
        for (n, (off, blocks)) in drag.rest.iter().enumerate() {
            let mut ghost = blocks.clone();
            Area::new(Id::new(("logic_ghost_rest", n))).order(Order::Tooltip).fixed_pos(ghost_pos + *off).constrain(false).interactable(false).show(ui.ctx(), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.set_opacity(0.85);
                let mut gr = R { info: &info, interactive: false, prev: &ghost_prev, rects: HashMap::new(), targets: vec![], take: None, clip: Rect::EVERYTHING, brk: &no_brk, toggle_brk: None, sel: &no_brk, click: None, right: None, menu: None };
                ui.vertical(|ui| {
                    for b in ghost.iter_mut() {
                        block(ui, b, &mut gr);
                    }
                });
            });
        }
        let is_rep = drag.blocks.len() == 1 && info.shape(&drag.blocks[0]).is_reporter();
        let tgt = pick(&targets, is_rep, &drag.blocks, &info, st.ghost, pointer, canvas_rect);
        // preview
        let lp = ui.ctx().layer_painter(LayerId::new(Order::Foreground, Id::new("logic_preview")));
        if pal_rect.contains(pointer) {
            lp.rect_filled(pal_rect, 0.0, Color32::from_rgba_unmultiplied(200, 40, 40, 30));
            lp.text(pal_rect.center(), Align2::CENTER_CENTER, "Drop here to delete", FontId::proportional(16.0), Color32::from_rgb(170, 40, 40));
        } else if let Some((rect, t)) = &tgt {
            match t {
                Target::Stack(..) => {
                    let y = rect.center().y;
                    lp.line_segment([pos2(rect.left() + 12.0, y), pos2(rect.left() + 150.0, y)], Stroke::new(4.0_f32, Color32::WHITE));
                    lp.line_segment([pos2(rect.left() + 12.0, y), pos2(rect.left() + 150.0, y)], Stroke::new(2.0_f32, Color32::from_gray(60)));
                }
                Target::Arg(..) => {
                    lp.rect_stroke(rect.expand(2.0), 4.0, Stroke::new(2.0_f32, Color32::WHITE));
                }
            }
        }
        if !down {
            let drag = st.drag.take().unwrap();
            changed = true;
            if pal_rect.contains(pointer) || !full.contains(pointer) {
                // deleted
                st.selected.clear();
            } else {
                let ev = &mut screen.events[ei];
                let placed = match tgt {
                    Some((_, t)) => place(ev, &t, drag.blocks),
                    None => Err(drag.blocks),
                };
                if let Err(blocks) = placed {
                    let at = st.ghost.min - origin;
                    ev.loose.push(LooseStack { x: at.x, y: at.y, blocks });
                }
                for (off, blocks) in drag.rest {
                    let at = st.ghost.min + off - origin;
                    ev.loose.push(LooseStack { x: at.x, y: at.y, blocks });
                }
            }
        }
        ui.ctx().request_repaint();
    }
    changed
}

fn pick(targets: &[(Rect, Target, Option<Param>)], is_rep: bool, blocks: &[Block], info: &Info, ghost: Rect, pointer: Pos2, canvas: Rect) -> Option<(Rect, Target)> {
    if !canvas.contains(pointer) {
        return None;
    }
    if !is_rep && blocks.iter().any(|b| info.shape(b).is_reporter()) {
        return None;
    }
    if is_rep {
        let s = info.shape(&blocks[0]);
        let probe = pos2(ghost.left() + 6.0, ghost.center().y);
        targets
            .iter()
            .filter(|(r, t, p)| matches!(t, Target::Arg(..)) && p.as_ref().map_or(false, |p| accepts(p, s)) && (r.expand(6.0).contains(probe) || r.contains(pointer)))
            .min_by(|a, b| a.0.area().partial_cmp(&b.0.area()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(r, t, _)| (*r, t.clone()))
    } else {
        let probe = ghost.left_top() + vec2(4.0, 2.0);
        // a dragged stack whose last block is a cap can't go above anything
        let ends_cap = blocks.last().map_or(false, |b| info.shape(b) == BS::Cap);
        targets
            .iter()
            .filter(|(r, t, _)| matches!(t, Target::Stack(..)) && r.contains(probe))
            .filter(|(_, t, _)| !ends_cap || matches!(t, Target::Stack(_, _)))
            .min_by(|a, b| (a.0.center().y - probe.y).abs().partial_cmp(&(b.0.center().y - probe.y).abs()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(r, t, _)| (*r, t.clone()))
    }
}

/// Whether a palette block matches every word of a search.
fn block_matches(b: &Block, q: &str) -> bool {
    let text = format!("{} {}", describe(b), b.op).to_lowercase();
    q.split_whitespace().all(|w| text.contains(w))
}

/// The buttons a category shows above its blocks: add variables and lists,
/// make a more block, or add a component in one click.
fn category_tools(ui: &mut Ui, st: &mut LogicState, screen: &mut Screen, cat: usize, ei: usize) -> bool {
    let mut changed = false;
    match cat {
        0 => {
            ui.horizontal(|ui| {
                if ui.button("Add variable").clicked() {
                    st.dialog = Some(LDialog::AddVar { name: String::new(), ty: VarType::Num });
                    st.error.clear();
                }
                if ui.button("Remove variable").clicked() {
                    st.dialog = Some(LDialog::RemoveVars);
                }
            });
        }
        1 => {
            ui.horizontal(|ui| {
                if ui.button("Add list").clicked() {
                    st.dialog = Some(LDialog::AddList { name: String::new(), ty: ListType::Str });
                    st.error.clear();
                }
                if ui.button("Remove list").clicked() {
                    st.dialog = Some(LDialog::RemoveVars);
                }
            });
        }
        7 => {
            // quick add: one click makes a component with a free name
            ui.menu_button("+ Add component", |ui| {
                ui.set_min_width(250.0);
                for k in CompKind::ALL {
                    let r = ui.button(k.label()).on_hover_text(k.description());
                    if r.clicked() {
                        let base = k.default_name();
                        let mut n = base.to_string();
                        let mut i = 2;
                        while screen.name_taken(&n) {
                            n = format!("{base}{i}");
                            i += 1;
                        }
                        screen.components.push(Component { name: n, kind: k });
                        changed = true;
                        ui.close_menu();
                    }
                }
            });
            if ui.button(crate::icons::lbl(crate::icons::MUSIC, "Add a sound file…")).on_hover_text("MP3, WAV or M4A. It goes into the app, for the Sound player blocks.").clicked() {
                st.pick_sound = true;
            }
            if !st.error.is_empty() && st.dialog.is_none() {
                ui.colored_label(Color32::from_rgb(200, 40, 40), &st.error);
            }
            if !screen.components.is_empty() {
                let names: Vec<String> = screen.components.iter().map(|c| format!("{} ({})", c.name, c.kind.label())).collect();
                ui.label(RichText::new(names.join(", ")).small().color(Color32::GRAY));
            }
        }
        8 => {
            if let Some(e) = screen.events.get(ei).filter(|e| e.target == "moreBlock") {
                if ui.button(format!("Change {}'s name or inputs", e.name)).clicked() {
                    let params = e.params.iter().enumerate().map(|(j, p)| (p.name.clone(), p.ty, Some(j))).collect();
                    st.dialog = Some(LDialog::MoreBlock { name: e.name.clone(), params, editing: Some(e.name.clone()) });
                    st.error.clear();
                }
            }
            if ui.button("Create a more block").clicked() {
                st.dialog = Some(LDialog::MoreBlock { name: String::new(), params: vec![], editing: None });
                st.error.clear();
            }
        }
        9 => supabase_link(ui, st),
        _ => {}
    }
    ui.add_space(4.0);
    changed
}

impl LogicState {
    /// Show the Supabase link form, filled in with `link`.
    /// Opens the window that changes a more block's name and inputs.
    pub fn edit_more(&mut self, e: &Event) {
        let params = e.params.iter().enumerate().map(|(j, p)| (p.name.clone(), p.ty, Some(j))).collect();
        self.dialog = Some(LDialog::MoreBlock { name: e.name.clone(), params, editing: Some(e.name.clone()) });
        self.error.clear();
    }

    pub fn open_supabase_form(&mut self, link: SupabaseLink) {
        self.supa_form = Some(link);
    }
}

/// Linking the project to Supabase, right above its blocks.
fn supabase_link(ui: &mut Ui, st: &mut LogicState) {
    let linked = st.supabase.clone().filter(|s| s.is_set());
    let Some(form) = &mut st.supa_form else {
        match &linked {
            Some(l) => {
                ui.horizontal_wrapped(|ui| {
                    let host = l.base().trim_start_matches("https://").to_string();
                    ui.label(RichText::new(format!("{}  Linked to {host}", crate::icons::CHECK)).small().color(c32(0xFF00_0000 | CATEGORIES[9].color)));
                    if ui.small_button("Change").clicked() {
                        st.supa_form = Some(l.clone());
                    }
                });
            }
            None => {
                ui.label(RichText::new("Link your Supabase project to sign people in and keep their data online.").small().color(Color32::GRAY));
                if ui.button(format!("{}  Link Supabase…", crate::icons::DATABASE)).clicked() {
                    st.supa_form = Some(SupabaseLink::default());
                }
            }
        }
        return;
    };
    let mut close = false;
    let mut save = None;
    Frame::none().fill(crate::theme::surface()).rounding(6.0).inner_margin(Margin::same(8.0)).show(ui, |ui| {
        ui.label(RichText::new("Project URL").small().strong());
        ui.add(TextEdit::singleline(&mut form.url).hint_text("https://abcd.supabase.co").desired_width(f32::INFINITY));
        ui.label(RichText::new("Public key (anon or publishable)").small().strong());
        ui.add(TextEdit::singleline(&mut form.key).hint_text("sb_publishable_… or eyJ…").desired_width(f32::INFINITY));
        ui.label(RichText::new("Both are in your Supabase dashboard, under Project Settings → API. The public key is safe in an app; the secret one is not.").small().color(Color32::GRAY));
        let problem = form.problem();
        if let Some(p) = problem {
            if !form.url.trim().is_empty() {
                ui.label(RichText::new(p).small().color(Color32::from_rgb(200, 40, 40)));
            }
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(problem.is_none(), Button::new(RichText::new("Link").color(Color32::WHITE)).fill(c32(0xFF00_0000 | CATEGORIES[9].color))).clicked() {
                save = Some(Some(SupabaseLink { url: form.base(), key: form.key.trim().to_string() }));
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
            if linked.is_some() && ui.button("Unlink").clicked() {
                save = Some(None);
                close = true;
            }
        });
    });
    if close {
        st.supa_form = None;
    }
    if save.is_some() {
        st.supa_save = save;
        ui.ctx().request_repaint();
    }
}

/// Palette blocks to drag onto the canvas.
fn palette_blocks(ui: &mut Ui, st: &mut LogicState, items: Vec<PItem>, cat: usize, info: &Info) {
    ui.spacing_mut().item_spacing = vec2(0.0, 8.0);
    let prev = HashMap::new();
    let no_brk = HashSet::new();
    for (n, it) in items.into_iter().enumerate() {
        match it {
            PItem::Header(h) => {
                ui.add_space(4.0);
                ui.label(RichText::new(h).color(c32(0xFF00_0000 | HEADER_COLOR)).strong().size(12.0));
            }
            PItem::Block(mut b) => {
                let mut r = R { info, interactive: false, prev: &prev, rects: HashMap::new(), targets: vec![], take: None, clip: Rect::EVERYTHING, brk: &no_brk, toggle_brk: None, sel: &no_brk, click: None, right: None, menu: None };
                let br = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    block(ui, &mut b, &mut r)
                });
                let rect = br.inner;
                let resp = ui.interact(rect, Id::new(("palblk", cat, n)), Sense::drag());
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
                if resp.drag_started() && st.drag.is_none() {
                    let p = resp.interact_pointer_pos().unwrap_or(rect.min);
                    st.drag = Some(Drag { blocks: vec![b.fresh()], grab: p - rect.min, rest: vec![] });
                }
            }
        }
    }
    ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
}

fn palette_panel(ui: &mut Ui, st: &mut LogicState, rect: Rect, screen: &mut Screen, screens: &crate::blocks::Choices, info: &Info, ei: usize) -> bool {
    let mut changed = false;
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)));
    child.set_clip_rect(rect);
    child.painter().rect_filled(rect, 0.0, crate::theme::surface());
    child.painter().line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0_f32, crate::theme::gray(215)));
    let ui = &mut child;
    Frame::none().inner_margin(Margin::same(8.0)).show(ui, |ui| {
        ui.set_width(rect.width() - 16.0);
        // search every category at once, for quick access
        ui.horizontal(|ui| {
            let w = ui.available_width() - if st.search.is_empty() { 0.0 } else { 30.0 };
            let te = TextEdit::singleline(&mut st.search).hint_text("Search blocks").desired_width(w);
            ui.add(te);
            if !st.search.is_empty() && ui.small_button(crate::icons::X).on_hover_text("Clear the search").clicked() {
                st.search.clear();
            }
        });
        ui.add_space(6.0);
        let q = st.search.trim().to_lowercase();
        ScrollArea::vertical().id_salt("pal").auto_shrink([false, false]).show(ui, |ui| {
            if !q.is_empty() {
                let mut any = false;
                for (ci, c) in CATEGORIES.iter().enumerate() {
                    let items: Vec<PItem> = palette_items(ci, screen, screens, screen.events.get(ei)).into_iter().filter(|it| matches!(it, PItem::Block(b) if block_matches(b, &q))).collect();
                    if items.is_empty() {
                        continue;
                    }
                    any = true;
                    ui.add_space(2.0);
                    ui.label(RichText::new(c.name).color(c32(0xFF00_0000 | c.color)).strong().size(12.5));
                    ui.add_space(4.0);
                    palette_blocks(ui, st, items, ci, info);
                    ui.add_space(6.0);
                }
                if !any {
                    ui.label(RichText::new("No block matches. Try another word, like \"text\", \"list\" or \"screen\".").italics().color(Color32::GRAY));
                }
            } else {
                // one category per line; the open one shows its blocks right under it
                for (i, c) in CATEGORIES.iter().enumerate() {
                    let col = c32(0xFF00_0000 | c.color);
                    let open = st.cat_open && st.cat == i;
                    let b = Button::new(RichText::new(c.name).color(if open { Color32::WHITE } else { col }).size(12.5).strong()).fill(if open { col } else { crate::theme::surface() }).stroke(Stroke::new(1.5_f32, col)).rounding(12.0).min_size(vec2(ui.available_width(), 26.0));
                    if ui.add(b).on_hover_text(if open { "Click to fold" } else { "Click to see its blocks" }).clicked() {
                        if open {
                            st.cat_open = false;
                        } else {
                            st.cat = i;
                            st.cat_open = true;
                        }
                    }
                    ui.add_space(4.0);
                    if open {
                        let line = col.gamma_multiply(0.6);
                        let r = Frame::none().inner_margin(Margin { left: 10.0, right: 2.0, top: 2.0, bottom: 8.0 }).show(ui, |ui| {
                            changed |= category_tools(ui, st, screen, i, ei);
                            let items = palette_items(i, screen, screens, screen.events.get(ei));
                            if items.is_empty() {
                                ui.label(RichText::new("Nothing here yet").italics().color(Color32::GRAY));
                            }
                            palette_blocks(ui, st, items, i, info);
                        });
                        let rr = r.response.rect;
                        ui.painter().line_segment([pos2(rr.left() + 3.0, rr.top()), pos2(rr.left() + 3.0, rr.bottom() - 6.0)], Stroke::new(2.0_f32, line));
                    }
                }
            }
            ui.add_space(30.0);
        });
    });

    // dialogs
    let ctx = ui.ctx().clone();
    let mut close = false;
    if let Some(d) = &mut st.dialog {
        let title = match d {
            LDialog::AddVar { .. } => "Add variable",
            LDialog::AddList { .. } => "Add list",
            LDialog::RemoveVars => "Remove variables and lists",
            LDialog::MoreBlock { editing: None, .. } => "Create a more block",
            LDialog::MoreBlock { editing: Some(_), .. } => "Change a more block",
        };
        Window::new(title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(&ctx, |ui| {
            ui.set_min_width(300.0);
            match d {
                LDialog::AddVar { name, ty } => {
                    ui.horizontal(|ui| {
                        for t in [VarType::Bool, VarType::Num, VarType::Str, VarType::Map] {
                            ui.radio_value(ty, t, t.label());
                        }
                    });
                    ui.add(TextEdit::singleline(name).hint_text("Name"));
                    if !st.error.is_empty() {
                        ui.colored_label(Color32::from_rgb(200, 40, 40), &st.error);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        if ui.button(RichText::new("Add").strong()).clicked() {
                            match name_problem(screen, name.trim()) {
                                Some(e) => st.error = e,
                                None => {
                                    screen.vars.push(Var { name: name.trim().into(), ty: *ty });
                                    changed = true;
                                    close = true;
                                }
                            }
                        }
                    });
                }
                LDialog::AddList { name, ty } => {
                    ui.horizontal(|ui| {
                        for t in [ListType::Num, ListType::Str, ListType::Map] {
                            ui.radio_value(ty, t, t.label());
                        }
                    });
                    ui.add(TextEdit::singleline(name).hint_text("Name"));
                    if !st.error.is_empty() {
                        ui.colored_label(Color32::from_rgb(200, 40, 40), &st.error);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        if ui.button(RichText::new("Add").strong()).clicked() {
                            match name_problem(screen, name.trim()) {
                                Some(e) => st.error = e,
                                None => {
                                    screen.lists.push(ListVar { name: name.trim().into(), ty: *ty });
                                    changed = true;
                                    close = true;
                                }
                            }
                        }
                    });
                }
                LDialog::RemoveVars => {
                    if screen.vars.is_empty() && screen.lists.is_empty() {
                        ui.label("There are no variables or lists.");
                    }
                    let mut rm_v = None;
                    let mut rm_l = None;
                    for (i, v) in screen.vars.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}  ({})", v.name, v.ty.label()));
                            if ui.small_button("Remove").clicked() {
                                rm_v = Some(i);
                            }
                        });
                    }
                    for (i, l) in screen.lists.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}  ({})", l.name, l.ty.label()));
                            if ui.small_button("Remove").clicked() {
                                rm_l = Some(i);
                            }
                        });
                    }
                    if let Some(i) = rm_v {
                        screen.vars.remove(i);
                        changed = true;
                    }
                    if let Some(i) = rm_l {
                        screen.lists.remove(i);
                        changed = true;
                    }
                    if ui.button("Done").clicked() {
                        close = true;
                    }
                }
                LDialog::MoreBlock { name, params, editing } => {
                    ui.label(if editing.is_some() {
                        "Blocks that already use it keep the values put in them."
                    } else {
                        "A more block is your own block: define it once, use it anywhere on this screen. Inputs let each use give it different values."
                    });
                    ui.add_space(4.0);
                    ui.add(TextEdit::singleline(name).hint_text("Block name").desired_width(f32::INFINITY));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Inputs").strong());
                    if params.is_empty() {
                        ui.label(RichText::new("None yet").small().color(Color32::GRAY));
                    }
                    let mut remove = None;
                    for (i, (pn, ty, _)) in params.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(TextEdit::singleline(pn).hint_text("input name").desired_width(130.0));
                            ComboBox::from_id_salt(("more_param_type", i)).selected_text(param_type_label(*ty)).width(110.0).show_ui(ui, |ui| {
                                for t in [VarType::Str, VarType::Num, VarType::Bool] {
                                    ui.selectable_value(ty, t, param_type_label(t));
                                }
                            });
                            if ui.small_button(crate::icons::X).on_hover_text("Remove this input").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        params.remove(i);
                    }
                    if ui.button(crate::icons::lbl(crate::icons::PLUS, "Add an input")).clicked() {
                        params.push((String::new(), VarType::Str, None));
                    }
                    ui.add_space(6.0);
                    if !st.error.is_empty() {
                        ui.colored_label(Color32::from_rgb(200, 40, 40), &st.error);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        if ui.button(RichText::new(if editing.is_some() { "Save" } else { "Create" }).strong()).clicked() {
                            let n = name.trim().to_string();
                            let problem = if editing.as_deref() == Some(n.as_str()) { None } else { name_problem(screen, &n) };
                            match problem.or_else(|| params_problem(params)) {
                                Some(e) => st.error = e,
                                None => {
                                    let vars: Vec<Var> = params.iter().map(|(pn, ty, _)| Var { name: pn.trim().into(), ty: *ty }).collect();
                                    match editing {
                                        Some(old) => {
                                            let from: Vec<Option<usize>> = params.iter().map(|p| p.2).collect();
                                            apply_more_edit(screen, old, &n, vars, &from);
                                        }
                                        None => screen.events.push(Event { target: "moreBlock".into(), name: n, blocks: vec![], loose: vec![], params: vars }),
                                    }
                                    changed = true;
                                    close = true;
                                }
                            }
                        }
                    });
                }
            }
        });
    }
    if close {
        st.dialog = None;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stack taller than the window follows the pointer while dragged and
    /// lands where it is dropped, not pinned to the top of the window.
    #[test]
    fn a_tall_stack_drags_freely() {
        let mut p = crate::app::sample_project(602);
        let one = p.screens[0].events[1].blocks[0].clone();
        let tall: Vec<Block> = (0..80).map(|_| one.fresh()).collect();
        let mut st = LogicState::default();
        st.drag = Some(Drag { blocks: tall, grab: vec2(10.0, 10.0), rest: vec![] });
        let ctx = egui::Context::default();
        let at = pos2(500.0, 400.0);
        let frame = |events: Vec<egui::Event>, st: &mut LogicState, p: &mut Project| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 800.0))), events, ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, st, p, 0, 1);
                });
            });
        };
        let press = |pressed| egui::Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        frame(vec![egui::Event::PointerMoved(at), press(true)], &mut st, &mut p);
        frame(vec![egui::Event::PointerMoved(at)], &mut st, &mut p);
        assert!(st.ghost.height() > 800.0, "the stack is taller than the window");
        assert!((st.ghost.min - (at - vec2(10.0, 10.0))).length() < 1.0, "ghost at {:?}", st.ghost.min);
        frame(vec![press(false)], &mut st, &mut p);
        assert!(st.drag.is_none());
        let l = p.screens[0].events[1].loose.last().expect("dropped loose");
        assert_eq!(l.blocks.len(), 80);
        assert!(l.y > 300.0, "landed at y {}", l.y);
    }

    /// Scrolling moves around the canvas with the pointer on a block too.
    #[test]
    fn scrolling_over_a_block_moves_the_canvas() {
        let mut p = crate::app::sample_project(602);
        let mut st = LogicState::default();
        let ctx = egui::Context::default();
        let frame = |events: Vec<egui::Event>, st: &mut LogicState, p: &mut Project| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 800.0))), events, ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, st, p, 0, 1);
                });
            });
        };
        frame(vec![], &mut st, &mut p);
        let on = st.rects.values().next().expect("a block on the canvas").center();
        frame(vec![egui::Event::PointerMoved(on)], &mut st, &mut p);
        let before = st.pan;
        let wheel = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -120.0), modifiers: Modifiers::NONE };
        frame(vec![wheel], &mut st, &mut p);
        for _ in 0..30 {
            frame(vec![], &mut st, &mut p);
        }
        assert!(st.pan.y < before.y - 50.0, "pan went from {before:?} to {:?}", st.pan);
    }

    #[test]
    fn take_and_place_blocks() {
        let p = crate::app::sample_project(601);
        let mut ev = p.screens[0].events[1].clone();
        let if_id = ev.blocks[0].id;
        let inner = ev.blocks[0].sub2[0].id;
        // take the inner "increase" block out of the else branch
        let got = take_block(&mut ev, inner).unwrap();
        assert_eq!(got.len(), 1);
        assert!(ev.blocks[0].sub2.is_empty());
        // put it at the top of the event
        place(&mut ev, &Target::Stack(StackLoc::Top, 0), got).unwrap();
        assert_eq!(ev.blocks[0].op, "increaseInt");
        // taking a reporter leaves an empty slot behind
        let cond = match &ev.blocks[1].args[0] { Arg::Block(b) => b.id, _ => panic!() };
        let r = take_block(&mut ev, cond).unwrap();
        assert_eq!(ev.blocks[1].args[0], Arg::Text(String::new()));
        place(&mut ev, &Target::Arg(if_id, 0), r).unwrap();
        assert!(matches!(ev.blocks[1].args[0], Arg::Block(_)));
        // taking a statement takes everything under it too
        let first = ev.blocks[0].id;
        let all = take_block(&mut ev, first).unwrap();
        assert_eq!(all.len(), 3);
        assert!(ev.blocks.is_empty());
        // loose stacks disappear when emptied
        ev.loose.push(LooseStack { x: 0.0, y: 0.0, blocks: all });
        take_block(&mut ev, first).unwrap();
        assert!(ev.loose.is_empty());
    }

    #[test]
    fn select_take_one_and_copy_blocks() {
        let p = crate::app::sample_project(601);
        let mut ev = p.screens[0].events[1].clone();
        let (if_id, set_text) = (ev.blocks[0].id, ev.blocks[1].id);
        let inner = ev.blocks[0].sub1[0].id;
        // a block inside a selected block isn't a root of its own
        let sel = HashSet::from([if_id, inner, set_text]);
        assert_eq!(selection_roots(&ev, &sel), vec![if_id, set_text]);
        // taking one block leaves the ones under it in place
        let first = take_one(&mut ev, if_id).unwrap();
        assert_eq!(first.op, "ifElse");
        assert_eq!(ev.blocks.len(), 1);
        assert_eq!(ev.blocks[0].id, set_text);
        // copy and paste keeps the blocks but gives them new ids
        let text = clip_text(&[first.clone()]);
        let back = from_clip_text(&text).unwrap();
        assert_eq!(back[0].op, "ifElse");
        assert_ne!(back[0].id, first.id);
        assert_eq!(back[0].sub1.len(), first.sub1.len());
        assert!(from_clip_text("just some text").is_none());
        // everything selectable at once: the top block of each stack
        ev.loose.push(LooseStack { x: 0.0, y: 0.0, blocks: back });
        assert_eq!(all_top_ids(&ev).len(), 2);
    }
}
