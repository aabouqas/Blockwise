//! The View tab: widget palette, the phone you drop widgets on, and the properties panel.

use crate::blocks::COLORS;
use crate::model::*;
use crate::phone::{self, c32, Mode, PhoneOut, PRIMARY};
use egui::*;
#[allow(unused_imports)]
use crate::model::{Event, Widget};
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
pub enum WDrag {
    New(WKind),
    Move(String),
}

#[derive(Default)]
pub struct DesignState {
    pub selected: Option<String>,
    pub drag: Option<WDrag>,
    rename: String,
    rename_for: String,
    rename_err: String,
    /// The picture picker was asked for, for the selected image.
    /// "Add a picture…" / "Add a font…" was clicked: the file dialog opens at the end of the frame.
    pick_file: Option<FileKind>,
    file_err: String,
    /// The play button was pressed (true), or stop (false): the editor
    /// starts or ends the app on the design phone.
    pub want_play: Option<bool>,
}

/// The app running on the design phone, while play is on: its blocks run and
/// taps go to it, and widgets can't be added, moved or changed. The editor
/// ticks it.
pub struct Play<'a> {
    pub rt: &'a mut crate::runtime::Runtime,
}

fn kind_icon(k: WKind) -> &'static str {
    match k {
        WKind::LinearH => crate::icons::COLUMNS,
        WKind::LinearV => crate::icons::ROWS,
        WKind::ScrollH => crate::icons::MOVE_H,
        WKind::ScrollV => crate::icons::MOVE_V,
        WKind::Button => crate::icons::POINTER_CLICK,
        WKind::TextView => crate::icons::TYPE,
        WKind::EditText => crate::icons::TEXT_INPUT,
        WKind::ImageView => crate::icons::IMAGE,
        WKind::WebView => crate::icons::GLOBE,
        WKind::ProgressBar => crate::icons::LOADER,
        WKind::ListView => crate::icons::LIST,
        WKind::Spinner => crate::icons::CHEVRONS_UP_DOWN,
        WKind::CheckBox => crate::icons::SQUARE_CHECK,
        WKind::Switch => crate::icons::TOGGLE,
        WKind::SeekBar => crate::icons::SLIDERS,
        WKind::CalendarView => crate::icons::CALENDAR_DAYS,
        WKind::VideoView => crate::icons::VIDEO,
    }
}

/// Put a widget (new or existing) into `parent` at sibling position `idx`.
pub fn put_widget(s: &mut Screen, what: &WDrag, parent: &str, idx: usize) -> Option<String> {
    let w = match what {
        WDrag::New(k) => Widget::new(s.next_widget_id(*k), *k, parent.to_string()),
        WDrag::Move(id) => {
            if parent == id || s.is_descendant(parent, id) {
                return None;
            }
            let pos = s.widgets.iter().position(|w| &w.id == id)?;
            let mut w = s.widgets.remove(pos);
            w.parent = parent.to_string();
            w
        }
    };
    let id = w.id.clone();
    let sib = s.children(parent);
    let at = if idx < sib.len() { sib[idx] } else { sib.last().map(|i| i + 1).unwrap_or(s.widgets.len()) };
    s.widgets.insert(at.min(s.widgets.len()), w);
    Some(id)
}

pub fn rename_widget(s: &mut Screen, old: &str, new: &str) {
    for w in s.widgets.iter_mut() {
        if w.id == old {
            w.id = new.to_string();
        }
        if w.parent == old {
            w.parent = new.to_string();
        }
    }
    fn walk(b: &mut Block, old: &str, new: &str) {
        let menus: Vec<usize> = crate::blocks::def(&b.op)
            .map(|d| crate::blocks::parse_spec(d.spec).into_iter().filter_map(|t| if let crate::blocks::Token::Param(i, crate::blocks::Param::Menu(_)) = t { Some(i) } else { None }).collect())
            .unwrap_or_default();
        for (i, a) in b.args.iter_mut().enumerate() {
            match a {
                Arg::Text(t) if menus.contains(&i) && t == old => *t = new.to_string(),
                Arg::Block(c) => walk(c, old, new),
                _ => {}
            }
        }
        for c in b.sub1.iter_mut().chain(b.sub2.iter_mut()) {
            walk(c, old, new);
        }
    }
    for e in s.events.iter_mut() {
        if e.target == old {
            e.target = new.to_string();
        }
        for b in e.blocks.iter_mut() {
            walk(b, old, new);
        }
        for l in e.loose.iter_mut() {
            for b in l.blocks.iter_mut() {
                walk(b, old, new);
            }
        }
    }
}

pub fn show(ui: &mut Ui, st: &mut DesignState, project: &mut Project, si: usize, mut play: Option<Play>) -> bool {
    let mut changed = false;
    let playing = play.is_some();
    if playing {
        st.drag = None;
    }
    let app_name = project.app_name.clone();
    let files: std::collections::HashMap<String, std::path::PathBuf> = project.file_paths().into_iter().map(|(f, p)| (f.name.clone(), p)).collect();
    let images = project.file_names(FileKind::Image);
    let fonts = project.file_names(FileKind::Font);
    let videos = project.file_names(FileKind::Video);
    // a bottom sheet is drawn over the first screen, which usually opens it
    let sheet = project.sheet_of(&project.screens[si]);
    let below = (sheet != Sheet::No).then(|| project.screens[0].clone());
    let screen = &mut project.screens[si];
    if let Some(sel) = &st.selected {
        if screen.widget(sel).is_none() {
            st.selected = None;
        }
    }

    // ----- left: insert palette, and the outline of the screen below it
    SidePanel::left("widget_palette").exact_width(250.0).resizable(false).frame(Frame::none().fill(crate::theme::surface()).inner_margin(Margin::same(0.0))).show_inside(ui, |ui| {
        TopBottomPanel::bottom("outline").resizable(true).default_height(260.0).height_range(120.0..=600.0).frame(Frame::none().fill(crate::theme::surface()).inner_margin(Margin::symmetric(12.0, 10.0))).show_inside(ui, |ui| {
            panel_title(ui, "Outline");
            ScrollArea::vertical().id_salt("outline_scroll").auto_shrink([false, false]).show(ui, |ui| {
                outline(ui, st, screen);
            });
        });
        CentralPanel::default().frame(Frame::none().inner_margin(Margin::symmetric(12.0, 10.0))).show_inside(ui, |ui| {
            if playing {
                ui.disable();
            }
            panel_title(ui, "Insert");
            ScrollArea::vertical().id_salt("palette_scroll").auto_shrink([false, false]).show(ui, |ui| {
                for (title, kinds) in [("LAYOUTS", &WKind::LAYOUTS[..]), ("WIDGETS", &WKind::WIDGETS[..])] {
                    ui.label(RichText::new(title).size(11.0).strong().color(crate::theme::gray(130)));
                    ui.add_space(4.0);
                    let tile_w = ((ui.available_width() - 6.0) / 2.0).floor();
                    for pair in kinds.chunks(2) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            for k in pair {
                                let (rect, resp) = ui.allocate_exact_size(vec2(tile_w, 30.0), Sense::click_and_drag());
                                let hover = resp.hovered() || st.drag == Some(WDrag::New(*k));
                                ui.painter().rect_filled(rect, 5.0, match (hover, crate::theme::dark()) {
                                    (true, false) => Color32::from_rgb(0xE3, 0xF2, 0xFB),
                                    (false, false) => Color32::from_rgb(0xF4, 0xF6, 0xFA),
                                    (true, true) => Color32::from_rgb(0x1E, 0x3A, 0x4E),
                                    (false, true) => Color32::from_rgb(0x2B, 0x30, 0x3A),
                                });
                                ui.painter().text(rect.left_center() + vec2(13.0, 0.0), Align2::CENTER_CENTER, kind_icon(*k), FontId::proportional(13.0), PRIMARY);
                                let p = ui.painter_at(rect.shrink2(vec2(2.0, 0.0)));
                                p.text(rect.left_center() + vec2(26.0, 0.0), Align2::LEFT_CENTER, k.palette_name(), FontId::proportional(12.5), crate::theme::gray(40));
                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                                }
                                if resp.drag_started() {
                                    st.drag = Some(WDrag::New(*k));
                                } else if resp.clicked() {
                                    // click: add to the selected layout, or the screen
                                    let parent = st.selected.as_ref().and_then(|s| screen.widget(s)).filter(|w| w.kind.is_container()).map(|w| w.id.clone()).unwrap_or_else(|| "root".into());
                                    if let Some(id) = put_widget(screen, &WDrag::New(*k), &parent, usize::MAX) {
                                        st.selected = Some(id);
                                        changed = true;
                                    }
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                    ui.add_space(8.0);
                }
                ui.label(RichText::new("Drag onto the phone, or click to add to the selected layout.").size(11.5).color(crate::theme::gray(140)));
            });
        });
    });

    // ----- right: inspector
    SidePanel::right("widget_props").exact_width(310.0).resizable(false).frame(Frame::none().fill(crate::theme::surface()).inner_margin(Margin { left: 18.0, right: 8.0, top: 12.0, bottom: 12.0 })).show_inside(ui, |ui| {
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // keep clear of the scroll bar
            ui.set_max_width(ui.available_width() - 12.0);
            if playing {
                ui.label(RichText::new(format!("{}  The app is running on the phone. Press Stop to edit again.", crate::icons::PLAY)).size(12.5).color(crate::theme::gray(120)));
                ui.add_space(8.0);
                ui.disable();
            }
            changed |= properties(ui, st, screen, si == 0, &images, &fonts, &videos);
        });
    });

    // ----- middle: the canvas, with the phone to design on
    let mut out = PhoneOut::default();
    let mut states = HashMap::new();
    let mut phone_t = emath::TSTransform::IDENTITY;
    let mut phone_content = Rect::NOTHING;
    CentralPanel::default().frame(Frame::none().fill(crate::theme::bg())).show_inside(ui, |ui| {
        TopBottomPanel::top("canvas_bar").exact_height(38.0).frame(Frame::none().fill(crate::theme::bg()).inner_margin(Margin::symmetric(14.0, 0.0))).show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let iphone = phone::preview_iphone();
                ui.label(RichText::new(screen.display_name()).strong());
                ui.label(RichText::new(format!("{} widgets", screen.widgets.len())).color(crate::theme::gray(130)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if crate::device::ios_tools() {
                        if let Some(pick) = segmented(ui, &["Android", "iOS"], if iphone { 1 } else { 0 }) {
                            phone::set_preview_iphone(pick == 1);
                        }
                        ui.label(RichText::new("Preview").color(crate::theme::gray(130)));
                        ui.add_space(10.0);
                    }
                    // play: the design phone runs the app, to try it in place
                    let tap = |ui: &mut Ui, text: String, fill: Color32, ink: Color32| ui.add(Button::new(RichText::new(text).color(ink).strong()).fill(fill).min_size(vec2(0.0, 28.0)).rounding(6.0));
                    if playing {
                        if tap(ui, format!("{}  Stop", crate::icons::STOP), Color32::from_rgb(0xD9, 0x3C, 0x3C), Color32::WHITE).on_hover_text("Stop the app and go back to designing  (Esc)").clicked() {
                            st.want_play = Some(false);
                        }
                    } else if tap(ui, format!("{}  Play", crate::icons::PLAY), PRIMARY, Color32::WHITE).on_hover_text("Run the app right here on the phone: tap buttons, type, scroll. Widgets can't be moved while it plays.").clicked() {
                        st.want_play = Some(true);
                    }
                });
            });
        });
        if let Some(pl) = play.as_mut() {
            let now = ui.input(|i| i.time);
            let out = crate::app::run_phone(ui, pl.rt, now, "design_phone_area");
            crate::app::run_phone_after(ui.ctx(), pl.rt, out, now);
            return;
        }
        // the phone, scaled down when the window is too small for it
        let canvas = ui.available_rect_before_wrap();
        let total = phone::shell_size(ui.ctx());
        let title = if screen.title.is_empty() { app_name.clone() } else { screen.title.clone() };
        let dragging_id = match &st.drag {
            Some(WDrag::Move(id)) => Some(id.clone()),
            _ => None,
        };
        let (_, t) = phone::fit_area(ui.ctx(), "design_phone_area", canvas, total, |ui| {
            // the first screen opens the app; the others open from it, with a back button
            let mut shell = match &below {
                Some(b) => {
                    let t = if b.title.is_empty() { app_name.clone() } else { b.title.clone() };
                    let mut sh = phone::shell_begin(ui, &t, &b.app_bar, false);
                    phone::draw_still(ui, sh.content, b, &files);
                    phone::sheet_begin(ui, &mut sh, sheet);
                    sh
                }
                None => phone::shell_begin(ui, &title, &screen.app_bar, si > 0),
            };
            shell.bottom = phone::bottom_color(screen, &states);
            let content = shell.content;
            phone_content = content;
            {
                let mut cx = phone::Ctx { screen, states: &mut states, mode: Mode::Design, selected: st.selected.as_deref(), dragging: dragging_id.as_deref(), out: &mut out, files: &files };
                phone::draw_screen(ui, content, &mut cx);
            }
            if screen.widgets.is_empty() && st.drag.is_none() {
                ui.painter().text(content.center(), Align2::CENTER_CENTER, "Drag widgets here", FontId::proportional(14.0), Color32::from_gray(170));
            }
            phone::shell_end(ui, &shell);
            ui.ctx().data_mut(|d| d.insert_temp(Id::new("design_phone_outer"), shell.outer));
            let resp = ui.interact(content, Id::new("design_phone"), Sense::click_and_drag());
            if resp.clicked() {
                st.selected = resp.interact_pointer_pos().and_then(|p| out.widget_at(p));
            }
            if resp.drag_started() && st.drag.is_none() {
                if let Some(id) = resp.interact_pointer_pos().and_then(|p| out.widget_at(p)) {
                    st.selected = Some(id.clone());
                    st.drag = Some(WDrag::Move(id));
                }
            }
        });
        phone_t = t;
        // dragging a widget near the top or bottom of the screen scrolls it,
        // to drop below (or above) what fits on the phone
        if st.drag.is_some() {
            if let Some(p) = ui.ctx().pointer_latest_pos().map(|p| t.inverse() * p) {
                let edge = 48.0;
                let speed: f32 = 9.0;
                let dy = if p.y < phone_content.top() + edge && p.y > phone_content.top() - edge {
                    speed
                } else if p.y > phone_content.bottom() - edge && p.y < phone_content.bottom() + edge {
                    -speed
                } else {
                    0.0
                };
                if dy != 0.0 && p.x > phone_content.left() && p.x < phone_content.right() {
                    ui.ctx().data_mut(|d| d.insert_temp(Id::new(phone::DESIGN_SCROLL_BY), dy));
                    ui.ctx().request_repaint();
                }
            }
        }
        // where the phone sits in the window (MCP's preview_screenshot crops to it)
        if let Some(outer) = ui.ctx().data(|d| d.get_temp::<Rect>(Id::new("design_phone_outer"))) {
            ui.ctx().data_mut(|d| d.insert_temp(Id::new("design_phone_on_window"), t.mul_rect(outer)));
        }
        let zoom = (t.scaling * 100.0).round();
        if zoom < 100.0 {
            ui.painter().text(canvas.right_bottom() - vec2(12.0, 10.0), Align2::RIGHT_BOTTOM, format!("{zoom}%"), FontId::proportional(12.0), crate::theme::gray(140));
        }
    });

    // ----- drag in progress
    if let Some(d) = st.drag.clone() {
        let ctx = ui.ctx().clone();
        let screen_pointer = ctx.pointer_latest_pos().unwrap_or(Pos2::ZERO);
        // the phone may be drawn scaled: its rects are in its own coordinates
        let pointer = phone_t.inverse() * screen_pointer;
        let label = match &d {
            WDrag::New(k) => k.palette_name().to_string(),
            WDrag::Move(id) => id.clone(),
        };
        Area::new(Id::new("widget_ghost")).order(Order::Tooltip).fixed_pos(screen_pointer + vec2(10.0, 6.0)).interactable(false).show(&ctx, |ui| {
            Frame::none().fill(PRIMARY).rounding(4.0).inner_margin(Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
                ui.label(RichText::new(label).color(Color32::WHITE));
            });
        });
        // find the layout under the pointer
        let skip = |id: &str| match &d {
            WDrag::Move(m) => id == m || screen.is_descendant(id, m),
            _ => false,
        };
        let target = out
            .containers
            .iter()
            .filter(|c| c.rect.contains(pointer) && !(c.id != "root" && skip(&c.id)))
            .max_by_key(|c| if c.id == "root" { 0 } else { out.rects.iter().find(|r| r.0 == c.id).map(|r| r.2).unwrap_or(0) });
        let mut drop_at = None;
        if let Some(c) = target {
            let kids: Vec<&(String, Rect)> = c.children.iter().filter(|(id, _)| !skip(id)).collect();
            let idx = kids.iter().filter(|(_, r)| if c.horizontal { r.center().x < pointer.x } else { r.center().y < pointer.y }).count();
            let lp = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("wdrop")));
            lp.rect_stroke(phone_t * c.rect, 0.0, Stroke::new(2.0_f32, PRIMARY));
            let line = if c.horizontal {
                let x = if idx < kids.len() { kids[idx].1.left() } else { kids.last().map_or(c.rect.left() + 4.0, |k| k.1.right()) };
                [pos2(x, c.rect.top() + 2.0), pos2(x, c.rect.bottom() - 2.0)]
            } else {
                let y = if idx < kids.len() { kids[idx].1.top() } else { kids.last().map_or(c.rect.top() + 4.0, |k| k.1.bottom()) };
                [pos2(c.rect.left() + 2.0, y), pos2(c.rect.right() - 2.0, y)]
            };
            lp.line_segment([phone_t * line[0], phone_t * line[1]], Stroke::new(3.0_f32, Color32::from_rgb(0xFF, 0x98, 0x00)));
            drop_at = Some((c.id.clone(), idx));
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            st.drag = None;
            if let Some((parent, idx)) = drop_at {
                if let Some(id) = put_widget(screen, &d, &parent, idx) {
                    st.selected = Some(id);
                    changed = true;
                }
            }
        }
        ctx.request_repaint();
    }
    if !playing {
        changed |= shortcuts(ui, st, screen);
    }
    // a picture or font picked for the selected widget: copied into the project
    if let Some(kind) = st.pick_file.take() {
        if let Some(path) = rfd::FileDialog::new().set_title(format!("Pick a {}", kind.one())).add_filter(kind.label(), kind.exts()).pick_file() {
            match project.add_file(&path) {
                Ok(name) => {
                    if let Some(w) = st.selected.as_deref().and_then(|id| project.screens[si].widget_mut(id)) {
                        match kind {
                            FileKind::Font => w.font = name,
                            FileKind::Video => w.video = name,
                            _ => w.image = name,
                        }
                    }
                    st.file_err.clear();
                    changed = true;
                }
                Err(e) => st.file_err = e,
            }
        }
    }
    changed
}

/// Marks widgets put on the system clipboard, so they paste back as widgets.
const WCLIP_TAG: &str = "blockwise-widgets:";

/// A widget and everything inside it, in the screen's order.
fn subtree(s: &Screen, id: &str) -> Vec<Widget> {
    s.widgets.iter().filter(|w| s.is_descendant(&w.id, id)).cloned().collect()
}

/// Puts copies of widgets (a widget and what's inside it) into `parent` at
/// sibling position `idx`, with new ids. Returns the new top widget's id.
fn paste_widgets(s: &mut Screen, ws: Vec<Widget>, parent: &str, idx: usize) -> Option<String> {
    let ids: Vec<String> = ws.iter().map(|w| w.id.clone()).collect();
    // exactly one widget sits outside the others: the top one
    ws.iter().find(|w| !ids.contains(&w.parent))?;
    let mut rename: HashMap<String, String> = HashMap::new();
    for w in &ws {
        let p = w.kind.id_prefix();
        let mut n = 1;
        let new = loop {
            let id = format!("{p}{n}");
            if !s.name_taken(&id) && !rename.values().any(|v| *v == id) {
                break id;
            }
            n += 1;
        };
        rename.insert(w.id.clone(), new);
    }
    let mut top = None;
    let mut rest = vec![];
    for mut w in ws {
        w.id = rename[&w.id].clone();
        if let Some(p) = rename.get(&w.parent) {
            w.parent = p.clone();
            rest.push(w);
        } else {
            w.parent = parent.to_string();
            top = Some(w);
        }
    }
    let top = top?;
    let new_root = top.id.clone();
    let sib = s.children(parent);
    let at = if idx < sib.len() { sib[idx] } else { sib.last().map(|i| i + 1).unwrap_or(s.widgets.len()) };
    s.widgets.insert(at.min(s.widgets.len()), top);
    s.widgets.extend(rest);
    Some(new_root)
}

/// Keyboard shortcuts on the Design tab (not while typing in a box):
/// Delete key deletes, ⌘D duplicate, ⌘C / ⌘X / ⌘V copy, cut and paste,
/// Alt ↑ / Alt ↓ move among its neighbours, ↑ / ↓ pick the widget above or below.
fn shortcuts(ui: &mut Ui, st: &mut DesignState, s: &mut Screen) -> bool {
    if st.drag.is_some() || ui.ctx().memory(|m| m.focused().is_some() || m.any_popup_open()) {
        return false;
    }
    let sel = st.selected.clone().filter(|id| s.widget(id).is_some());
    let mut changed = false;
    let (mut del, mut dup, mut move_up, mut move_down, mut prev, mut next) = (false, false, false, false, false, false);
    let (mut copy, mut cut, mut paste) = (false, false, None::<String>);
    ui.input_mut(|i| {
        dup = i.consume_key(Modifiers::COMMAND, Key::D);
        del = i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace);
        move_up = i.consume_key(Modifiers::ALT, Key::ArrowUp);
        move_down = i.consume_key(Modifiers::ALT, Key::ArrowDown);
        prev = i.consume_key(Modifiers::NONE, Key::ArrowUp);
        next = i.consume_key(Modifiers::NONE, Key::ArrowDown);
        for e in &i.events {
            match e {
                egui::Event::Copy => copy = true,
                egui::Event::Cut => cut = true,
                egui::Event::Paste(t) => paste = Some(t.clone()),
                _ => {}
            }
        }
    });
    if prev || next {
        let n = s.widgets.len();
        if n > 0 {
            let cur = sel.as_ref().and_then(|id| s.widgets.iter().position(|w| &w.id == id));
            let i = match (cur, next) {
                (None, _) => 0,
                (Some(c), true) => (c + 1).min(n - 1),
                (Some(c), false) => c.saturating_sub(1),
            };
            st.selected = Some(s.widgets[i].id.clone());
        }
    }
    let Some(sel) = sel else {
        // nothing picked: paste onto the screen
        if let Some(ws) = paste.as_deref().and_then(|t| serde_json::from_str::<Vec<Widget>>(t.trim().strip_prefix(WCLIP_TAG)?).ok()) {
            let n = s.children("root").len();
            if let Some(id) = paste_widgets(s, ws, "root", n) {
                st.selected = Some(id);
                changed = true;
            }
        }
        return changed;
    };
    let parent = s.widget(&sel).map(|w| w.parent.clone()).unwrap_or_else(|| "root".into());
    let sib = s.children(&parent);
    let pos = sib.iter().position(|&i| s.widgets[i].id == sel).unwrap_or(0);
    if move_up && pos > 0 {
        s.widgets.swap(sib[pos], sib[pos - 1]);
        changed = true;
    }
    if move_down && pos + 1 < sib.len() {
        s.widgets.swap(sib[pos], sib[pos + 1]);
        changed = true;
    }
    if copy || cut {
        let text = format!("{WCLIP_TAG}{}", serde_json::to_string(&subtree(s, &sel)).unwrap_or_default());
        ui.ctx().copy_text(text);
    }
    if dup {
        if let Some(id) = paste_widgets(s, subtree(s, &sel), &parent, pos + 1) {
            st.selected = Some(id);
            changed = true;
        }
    }
    if let Some(ws) = paste.as_deref().and_then(|t| serde_json::from_str::<Vec<Widget>>(t.trim().strip_prefix(WCLIP_TAG)?).ok()) {
        // into a picked layout, else next to the picked widget
        let into = s.widget(&sel).is_some_and(|w| w.kind.is_container());
        let (p, at) = if into { (sel.clone(), s.children(&sel).len()) } else { (parent.clone(), pos + 1) };
        if let Some(id) = paste_widgets(s, ws, &p, at) {
            st.selected = Some(id);
            changed = true;
        }
    }
    if (del || cut) && s.widget(&sel).is_some() {
        s.remove_widget(&sel);
        st.selected = None;
        changed = true;
    }
    changed
}

/// A small grey heading at the top of a side panel.
pub fn panel_title(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).size(13.0).strong().color(crate::theme::gray(70)));
    ui.add_space(6.0);
}

/// Buttons side by side, one picked (like a segmented switch). Returns a new pick.
pub fn segmented(ui: &mut Ui, items: &[&str], picked: usize) -> Option<usize> {
    let mut out = None;
    // laid out right to left by callers too, so draw into one fixed box
    let font = FontId::proportional(13.0);
    let widths: Vec<f32> = items.iter().map(|t| ui.painter().layout_no_wrap(t.to_string(), font.clone(), Color32::WHITE).size().x + 22.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 28.0), Sense::hover());
    let dark = crate::theme::dark();
    ui.painter().rect_filled(rect, 7.0, if dark { Color32::from_rgb(0x2B, 0x30, 0x3A) } else { Color32::from_rgb(0xE4, 0xE8, 0xF0) });
    let mut x = rect.left() + 2.0;
    for (i, (t, w)) in items.iter().zip(widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(w, rect.height() - 4.0));
        let resp = ui.interact(r, ui.id().with(("seg", t, i, rect.top() as i32)), Sense::click());
        if i == picked {
            ui.painter().rect_filled(r, 6.0, crate::theme::surface());
            ui.painter().rect_stroke(r, 6.0, Stroke::new(0.5_f32, crate::theme::gray(200)));
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 6.0, crate::theme::surface_hover());
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *t, font.clone(), if i == picked { crate::theme::ink() } else { crate::theme::gray(110) });
        if resp.clicked() && i != picked {
            out = Some(i);
        }
        x += w;
    }
    out
}

/// The screen's widgets as an indented tree; a click selects one.
fn outline(ui: &mut Ui, st: &mut DesignState, screen: &Screen) {
    let rows = tree_rows(screen);
    if rows.is_empty() {
        ui.label(RichText::new("No widgets yet").color(crate::theme::gray(140)));
    }
    for (id, kind, d) in rows {
        let sel = st.selected.as_deref() == Some(id.as_str());
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        if sel {
            ui.painter().rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 4.0, crate::theme::surface_hover());
        }
        let x = rect.left() + 6.0 + d as f32 * 14.0;
        ui.painter().text(pos2(x + 7.0, rect.center().y), Align2::CENTER_CENTER, kind_icon(kind), FontId::proportional(12.0), PRIMARY);
        ui.painter().text(pos2(x + 18.0, rect.center().y), Align2::LEFT_CENTER, &id, FontId::proportional(13.0), crate::theme::ink());
        ui.painter().text(rect.right_center() - vec2(6.0, 0.0), Align2::RIGHT_CENTER, kind.palette_name(), FontId::proportional(11.5), crate::theme::gray(140));
        if resp.clicked() {
            st.selected = Some(id.clone());
        }
    }
}

fn size_edit(ui: &mut Ui, label: &str, s: &mut Size) -> bool {
    let mut ch = false;
    ui.label(label.trim());
    ui.horizontal(|ui| {
        let cur = match s {
            Size::Wrap => "wrap_content",
            Size::Match => "match_parent",
            Size::Px(_) => "fixed",
        };
        ComboBox::from_id_salt(("size", label)).selected_text(cur).width(120.0).show_ui(ui, |ui| {
            ch |= ui.selectable_value(s, Size::Wrap, "wrap_content").changed();
            ch |= ui.selectable_value(s, Size::Match, "match_parent").changed();
            if ui.selectable_label(matches!(s, Size::Px(_)), "fixed").clicked() && !matches!(s, Size::Px(_)) {
                *s = Size::Px(100);
                ch = true;
            }
        });
        if let Size::Px(p) = s {
            ch |= ui.add(DragValue::new(p).range(1..=2000).suffix(" dp")).changed();
        }
    });
    ui.end_row();
    ch
}

/// A drop-down of plain choices; the new pick, if one was made.
fn pick(ui: &mut Ui, id: &str, items: &[&str], cur: usize) -> Option<usize> {
    let mut out = None;
    ComboBox::from_id_salt(id).selected_text(items[cur]).width(120.0).show_ui(ui, |ui| {
        for (i, it) in items.iter().enumerate() {
            if ui.selectable_label(i == cur, *it).clicked() && i != cur {
                out = Some(i);
            }
        }
    });
    out
}

/// A colour swatch; transparent shows as a crossed-out box.
fn swatch(p: &Painter, r: Rect, v: u32, round: f32) {
    if v >> 24 == 0 {
        p.rect_filled(r, round, Color32::WHITE);
        p.rect_stroke(r, round, Stroke::new(1.0_f32, Color32::GRAY));
        p.line_segment([r.left_bottom() + vec2(2.0, -2.0), r.right_top() + vec2(-2.0, 2.0)], Stroke::new(1.2_f32, Color32::from_rgb(200, 60, 60)));
    } else {
        p.rect_filled(r, round, c32(v));
        p.rect_stroke(r, round, Stroke::new(0.5_f32, Color32::GRAY));
    }
}

/// One colour property: a swatch with its hex value that opens a popup of
/// the preset colours and a full picker.
fn color_edit(ui: &mut Ui, label: &str, c: &mut u32) -> bool {
    color_edit_or(ui, label, c, "none")
}

/// `color_edit` whose empty colour reads `empty` ("default" for the app bar).
fn color_edit_or(ui: &mut Ui, label: &str, c: &mut u32, empty: &str) -> bool {
    let mut ch = false;
    ui.label(label);
    let text = if *c >> 24 == 0 { empty.to_string() } else if *c >> 24 == 0xFF { format!("#{:06X}", *c & 0xFF_FFFF) } else { format!("#{:08X}", *c) };
    let resp = ui.add(Button::new(RichText::new(format!("      {text}")).monospace().size(12.5)).min_size(vec2(120.0, 24.0)));
    swatch(ui.painter(), Rect::from_min_size(resp.rect.left_center() + vec2(6.0, -7.0), vec2(22.0, 14.0)), *c, 3.0);
    let pid = ui.make_persistent_id(("colour_popup", label));
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pid));
    }
    popup_below_widget(ui, pid, &resp, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(228.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for row in COLORS.chunks(7) {
            ui.horizontal(|ui| {
                for (name, v) in row {
                    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                    swatch(ui.painter(), r, *v, 5.0);
                    if *c == *v {
                        ui.painter().rect_stroke(r.expand(2.0), 6.0, Stroke::new(2.0_f32, PRIMARY));
                    }
                    if resp.on_hover_text(format!("{name}  #{:08X}", v)).clicked() {
                        *c = *v;
                        ch = true;
                    }
                }
            });
        }
        ui.separator();
        let mut col = c32(*c);
        if color_picker::color_picker_color32(ui, &mut col, color_picker::Alpha::BlendOrAdditive) {
            *c = phone::argb(col);
            ch = true;
        }
    });
    ui.end_row();
    ch
}

/// The app bar's icon buttons: pick an icon, give it a label, or remove it.
/// Each one's "when clicked" event is in the Logic tab's Add event.
fn bar_actions(ui: &mut Ui, screen: &mut Screen) -> bool {
    let mut changed = false;
    prop_section(ui, "App bar buttons");
    let mut remove = None;
    for i in 0..screen.app_bar.actions.len() {
        let a = screen.app_bar.actions[i].clone();
        ui.horizontal(|ui| {
            let (glyph, _) = bar_icon(&a.icon);
            let mut icon = a.icon.clone();
            ComboBox::from_id_salt(("bar_icon", i)).width(46.0).selected_text(RichText::new(glyph.to_string()).size(16.0)).show_ui(ui, |ui| {
                ui.set_min_width(170.0);
                for (name, c, _) in BAR_ICONS {
                    ui.selectable_value(&mut icon, name.to_string(), format!("{c}   {name}"));
                }
            });
            if icon != a.icon {
                // an untouched button takes its new icon's name
                let has_event = screen.events.iter().any(|e| e.target == a.id);
                let act = &mut screen.app_bar.actions[i];
                act.icon = icon.clone();
                if !has_event {
                    act.id = String::new();
                    let id = screen.next_action_id(&icon);
                    screen.app_bar.actions[i].id = id;
                }
                changed = true;
            }
            let act = &mut screen.app_bar.actions[i];
            changed |= ui.add(TextEdit::singleline(&mut act.label).hint_text("Label").desired_width(ui.available_width() - 30.0)).on_hover_text(format!("{}: read aloud by screen readers; Android shows it on a long press", act.id)).changed();
            if ui.add(Button::new(crate::icons::X).frame(false)).on_hover_text("Remove this button and its event").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        let id = screen.app_bar.actions.remove(i).id;
        screen.events.retain(|e| e.target != id);
        changed = true;
    }
    if ui.button(format!("{}  Add button", crate::icons::PLUS)).on_hover_text("An icon button on the right of the bar. Program it in Logic: Add event › When … is clicked").clicked() {
        let id = screen.next_action_id("search");
        screen.app_bar.actions.push(BarAction { id, icon: "search".into(), label: "Search".into() });
        changed = true;
    }
    changed
}

/// A section heading in the inspector.
fn prop_section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.separator();
    ui.label(RichText::new(title).size(13.0).strong().color(crate::theme::gray(70)));
    ui.add_space(2.0);
}

fn prop_grid(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    Grid::new(id).num_columns(2).min_col_width(70.0).spacing(vec2(8.0, 8.0)).show(ui, add);
}

fn properties(ui: &mut Ui, st: &mut DesignState, screen: &mut Screen, first: bool, images: &[String], fonts: &[String], videos: &[String]) -> bool {
    let mut changed = false;
    let Some(sel) = st.selected.clone() else {
        panel_title(ui, "Screen");
        prop_grid(ui, "screen_props", |ui| {
            ui.label("Title");
            if ui.add(TextEdit::singleline(&mut screen.title).hint_text("App name").desired_width(f32::INFINITY)).changed() {
                changed = true;
            }
            ui.end_row();
            ui.label("Opens as");
            let name = |sh: Sheet| match sh {
                Sheet::No => "Full screen",
                Sheet::Half => "Bottom sheet, half",
                Sheet::Full => "Bottom sheet, tall",
            };
            if first {
                ui.label(RichText::new("Full screen (the app starts here)").color(crate::theme::gray(130)));
            } else {
                ComboBox::from_id_salt("opens_as").selected_text(name(screen.sheet)).show_ui(ui, |ui| {
                    for sh in [Sheet::No, Sheet::Half, Sheet::Full] {
                        changed |= ui.selectable_value(&mut screen.sheet, sh, name(sh)).changed();
                    }
                }).response.on_hover_text("A bottom sheet slides up over the screen that opens it (open screen block). Close this screen, or a tap above it, closes it.");
            }
            ui.end_row();
            // a sheet has no app bar
            if first || screen.sheet == Sheet::No {
            ui.label("App bar");
            let mut show = !screen.app_bar.hidden;
            if ui.checkbox(&mut show, "Show").changed() {
                screen.app_bar.hidden = !show;
                changed = true;
            }
            ui.end_row();
            if !screen.app_bar.hidden {
                changed |= color_edit_or(ui, "Bar colour", &mut screen.app_bar.color, "default");
                changed |= color_edit_or(ui, "Title colour", &mut screen.app_bar.title_color, "default");
                ui.label("Back");
                let mut back = !screen.app_bar.no_back;
                if ui.checkbox(&mut back, "Back button").on_hover_text("On a screen opened from another, the bar has a back arrow").changed() {
                    screen.app_bar.no_back = !back;
                    changed = true;
                }
                ui.end_row();
            }
            }
            ui.label("Widgets");
            ui.label(format!("{}", screen.widgets.len()));
            ui.end_row();
            ui.label("Events");
            ui.label(format!("{}", screen.events.len()));
            ui.end_row();
        });
        if !screen.app_bar.hidden && (first || screen.sheet == Sheet::No) {
            changed |= bar_actions(ui, screen);
        }
        ui.add_space(14.0);
        ui.label(RichText::new("Select a widget on the phone or in the outline to change how it looks.").color(crate::theme::gray(130)));
        return changed;
    };
    if st.rename_for != sel {
        st.rename_for = sel.clone();
        st.rename = sel.clone();
        st.rename_err.clear();
    }
    let kind = screen.widget(&sel).map(|w| w.kind).unwrap_or(WKind::TextView);
    ui.horizontal(|ui| {
        ui.label(RichText::new(kind_icon(kind)).size(16.0).color(PRIMARY));
        ui.label(RichText::new(kind.palette_name()).size(16.0).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(Button::new(crate::icons::X).frame(false)).on_hover_text("Deselect").clicked() {
                st.selected = None;
            }
        });
    });
    ui.add_space(6.0);
    prop_grid(ui, "id_grid", |ui| {
        ui.label("ID");
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut st.rename).desired_width(130.0));
            if st.rename != sel && ui.button("Rename").clicked() {
                let n = st.rename.trim().to_string();
                match crate::logic::name_problem(screen, &n) {
                    Some(e) => st.rename_err = e,
                    None => {
                        rename_widget(screen, &sel, &n);
                        st.selected = Some(n.clone());
                        st.rename_for = n;
                        st.rename_err.clear();
                        changed = true;
                    }
                }
            }
        });
        ui.end_row();
    });
    if !st.rename_err.is_empty() {
        ui.colored_label(Color32::from_rgb(200, 40, 40), &st.rename_err);
    }
    let sel = st.selected.clone().unwrap_or(sel);
    let Some(w) = screen.widget_mut(&sel) else { return changed };
    prop_section(ui, "Layout");
    prop_grid(ui, "layout_grid", |ui| {
        changed |= size_edit(ui, "Width", &mut w.width);
        changed |= size_edit(ui, "Height", &mut w.height);
        ui.label("Padding");
        changed |= ui.add(DragValue::new(&mut w.padding).range(0..=200).suffix(" dp")).changed();
        ui.end_row();
        ui.label("Margin");
        changed |= ui.add(DragValue::new(&mut w.margin).range(0..=200).suffix(" dp")).changed();
        ui.end_row();
        // the combined value that is stored
        let flags = gravity_flags(w.gravity, w.vgravity);
        let tip = match gravity_xml(w.gravity, w.vgravity) {
            Some(x) => format!("Android gravity: {x} (0x{flags:02X})"),
            None => "No gravity: things sit at the top left".to_string(),
        };
        ui.label("Gravity").on_hover_text(&tip);
        let cur = [Gravity::None, Gravity::Left, Gravity::Center, Gravity::Right].iter().position(|g| *g == w.gravity).unwrap_or(0);
        if let Some(i) = pick(ui, "gravity_h", &["none", "left", "center", "right"], cur) {
            w.gravity = [Gravity::None, Gravity::Left, Gravity::Center, Gravity::Right][i];
            changed = true;
        }
        ui.end_row();
        // vertical gravity, picked on its own
        ui.label("Vertical").on_hover_text(&tip);
        let vs = [VGravity::None, VGravity::Top, VGravity::Center, VGravity::Bottom];
        let cur = vs.iter().position(|g| *g == w.vgravity).unwrap_or(0);
        if let Some(i) = pick(ui, "gravity_v", &["none", "top", "center", "bottom"], cur) {
            // stored and read back as one number
            (w.gravity, w.vgravity) = gravity_from_flags(gravity_flags(w.gravity, vs[i]));
            changed = true;
        }
        ui.end_row();
        ui.label("Weight");
        changed |= ui.add(DragValue::new(&mut w.weight).range(0.0..=10.0).speed(0.1)).changed();
        ui.end_row();
        changed |= color_edit(ui, "Background", &mut w.bg);
    });
    prop_section(ui, "Shape");
    prop_grid(ui, "shape_grid", |ui| {
        ui.label("Corners");
        changed |= ui.add(DragValue::new(&mut w.radius).range(0..=200).suffix(" dp")).on_hover_text("Rounded corners").changed();
        ui.end_row();
        ui.label("Shadow");
        changed |= ui.add(DragValue::new(&mut w.elevation).range(0..=24).suffix(" dp")).on_hover_text("How far it lifts off the screen. Needs a background colour.").changed();
        ui.end_row();
        ui.label("Border");
        changed |= ui.add(DragValue::new(&mut w.border).range(0..=20).suffix(" dp")).on_hover_text("An outline; 0 for none").changed();
        ui.end_row();
        if w.border > 0 {
            changed |= color_edit(ui, "Border colour", &mut w.border_color);
        }
    });
    if w.kind == WKind::ImageView {
        prop_section(ui, "Picture");
        prop_grid(ui, "picture_grid", |ui| {
            ui.label("Picture");
            let shown = if w.image.is_empty() { "none".to_string() } else { w.image.clone() };
            ComboBox::from_id_salt("picture").selected_text(shown).width(150.0).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut w.image, String::new(), "none").changed();
                for n in images {
                    changed |= ui.selectable_value(&mut w.image, n.clone(), n).changed();
                }
            });
            ui.end_row();
            ui.label("");
            if ui.button(crate::icons::lbl(crate::icons::IMAGE_PLUS, "Add a picture…")).on_hover_text("PNG or JPEG. It goes into the app; 1 pixel shows as 1 dp.").clicked() {
                st.pick_file = Some(FileKind::Image);
            }
            ui.end_row();
            ui.label("Fit");
            let cur = if w.crop { 1 } else { 0 };
            if let Some(i) = pick(ui, "picture_fit", &["whole picture", "fill and cut"], cur) {
                w.crop = i == 1;
                changed = true;
            }
            ui.end_row();
        });
        if !st.file_err.is_empty() {
            ui.colored_label(Color32::from_rgb(200, 40, 40), &st.file_err);
        }
    }
    if w.kind == WKind::VideoView {
        prop_section(ui, "Video");
        prop_grid(ui, "video_grid", |ui| {
            ui.label("Video");
            let shown = if w.video.is_empty() { "none".to_string() } else { w.video.clone() };
            ComboBox::from_id_salt("video").selected_text(shown).width(150.0).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut w.video, String::new(), "none").changed();
                for n in videos {
                    changed |= ui.selectable_value(&mut w.video, n.clone(), n).changed();
                }
            });
            ui.end_row();
            ui.label("");
            if ui.button(crate::icons::lbl(crate::icons::PLUS, "Add a video…")).on_hover_text("MP4 (H.264) plays on every phone; M4V and MOV work too.").clicked() {
                st.pick_file = Some(FileKind::Video);
            }
            ui.end_row();
            ui.label("Playing");
            ui.vertical(|ui| {
                changed |= ui.checkbox(&mut w.autoplay, "Starts by itself").changed();
                changed |= ui.checkbox(&mut w.looping, "Again and again").changed();
                changed |= ui.checkbox(&mut w.controls, "Play/pause controls").changed();
            });
            ui.end_row();
        });
        if !st.file_err.is_empty() {
            ui.colored_label(Color32::from_rgb(200, 40, 40), &st.file_err);
        }
    }
    if w.kind.has_text() {
        prop_section(ui, "Text");
        changed |= ui.add(TextEdit::multiline(&mut w.text).desired_rows(2).desired_width(f32::INFINITY)).changed();
        ui.add_space(4.0);
        prop_grid(ui, "text_grid", |ui| {
            ui.label("Size");
            ui.horizontal(|ui| {
                changed |= ui.add(DragValue::new(&mut w.text_size).range(4.0..=96.0).suffix(" sp")).changed();
                changed |= ui.checkbox(&mut w.bold, "Bold").changed();
            });
            ui.end_row();
            changed |= color_edit(ui, "Colour", &mut w.text_color);
            ui.label("Font");
            let shown = if w.font.is_empty() { "phone's own".to_string() } else { w.font.clone() };
            ComboBox::from_id_salt("font").selected_text(shown).width(150.0).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut w.font, String::new(), "phone's own").changed();
                for n in fonts {
                    let fam = crate::phone::project_font(ui.ctx(), n);
                    changed |= ui.selectable_value(&mut w.font, n.clone(), RichText::new(n).family(fam)).changed();
                }
            });
            ui.end_row();
            ui.label("");
            if ui.button(crate::icons::lbl(crate::icons::PLUS, "Add a font…")).on_hover_text("A TTF or OTF font file. It goes into the app.").clicked() {
                st.pick_file = Some(FileKind::Font);
            }
            ui.end_row();
            if w.kind == WKind::EditText {
                ui.label("Hint");
                changed |= ui.text_edit_singleline(&mut w.hint).changed();
                ui.end_row();
                changed |= color_edit(ui, "Hint colour", &mut w.hint_color);
                ui.label("Input").on_hover_text("What the field takes: the keyboard it brings up, and dots with an eye for a password");
                let cur = w.input_kind();
                ComboBox::from_id_salt("input_kind").selected_text(cur.label()).show_ui(ui, |ui| {
                    for k in InputKind::ALL {
                        if ui.selectable_label(cur == k, k.label()).clicked() && cur != k {
                            w.input = k;
                            w.password = k == InputKind::Password;
                            changed = true;
                        }
                    }
                });
                ui.end_row();
            }
        });
        if !st.file_err.is_empty() {
            ui.colored_label(Color32::from_rgb(200, 40, 40), &st.file_err);
        }
    }
    if w.kind.is_compound() || matches!(w.kind, WKind::SeekBar | WKind::ProgressBar) {
        prop_section(ui, "Value");
        prop_grid(ui, "value_grid", |ui| {
            if w.kind.is_compound() {
                ui.label("Checked");
                changed |= ui.checkbox(&mut w.checked, "").changed();
                ui.end_row();
            }
            if matches!(w.kind, WKind::SeekBar | WKind::ProgressBar) {
                ui.label("Progress");
                changed |= ui.add(DragValue::new(&mut w.progress).range(0..=w.max.max(0))).changed();
                ui.end_row();
                ui.label("Max");
                changed |= ui.add(DragValue::new(&mut w.max).range(0..=100000)).changed();
                ui.end_row();
            }
        });
    }
    prop_section(ui, "State");
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut w.enabled, "Enabled").changed();
        ui.add_space(12.0);
        changed |= ui.checkbox(&mut w.visible, "Visible").changed();
    });
    prop_section(ui, "Arrange");
    let parent = w.parent.clone();
    ui.horizontal(|ui| {
        let sib = screen.children(&parent);
        let pos = sib.iter().position(|&i| screen.widgets[i].id == sel).unwrap_or(0);
        if ui.add_enabled(pos > 0, Button::new(crate::icons::lbl("\u{e04e}", "Up"))).on_hover_text("Move up (Alt ↑)").clicked() {
            screen.widgets.swap(sib[pos], sib[pos - 1]);
            changed = true;
        }
        if ui.add_enabled(pos + 1 < sib.len(), Button::new(crate::icons::lbl("\u{e046}", "Down"))).on_hover_text("Move down (Alt ↓)").clicked() {
            screen.widgets.swap(sib[pos], sib[pos + 1]);
            changed = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(RichText::new(crate::icons::lbl(crate::icons::TRASH, "Delete")).color(Color32::from_rgb(200, 40, 40))).on_hover_text("Delete (Delete key)").clicked() {
                screen.remove_widget(&sel);
                st.selected = None;
                changed = true;
            }
        });
    });
    changed
}

/// The widgets in tree order: each layout followed by what it holds, in the
/// order the phone shows them, so the indents read as the real nesting.
/// Anything whose parent is gone comes last, at the top level.
fn tree_rows(s: &Screen) -> Vec<(String, WKind, usize)> {
    fn walk(s: &Screen, parent: &str, d: usize, out: &mut Vec<(String, WKind, usize)>, seen: &mut std::collections::HashSet<String>) {
        for i in s.children(parent) {
            let w = &s.widgets[i];
            if d < 50 && seen.insert(w.id.clone()) {
                out.push((w.id.clone(), w.kind, d));
                walk(s, &w.id, d + 1, out, seen);
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    walk(s, "root", 0, &mut out, &mut seen);
    for w in &s.widgets {
        if !seen.contains(&w.id) {
            out.push((w.id.clone(), w.kind, 0));
        }
    }
    out
}

#[allow(dead_code)]
fn depth(s: &Screen, id: &str) -> usize {
    let mut d = 0;
    let mut cur = s.widget(id).map(|w| w.parent.clone()).unwrap_or_default();
    while cur != "root" && d < 50 {
        d += 1;
        cur = s.widget(&cur).map(|w| w.parent.clone()).unwrap_or_else(|| "root".into());
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tap on the design phone while play is on reaches the app's blocks,
    /// and the design itself doesn't change.
    #[test]
    fn play_runs_the_app_on_the_design_phone() {
        let mut project = crate::app::sample_project(601);
        let before = crate::model::project_snapshot(&project);
        let mut rt = crate::runtime::Runtime::new(project.clone(), 0.0);
        let mut st = DesignState::default();
        let ctx = Context::default();
        let mut frame = |events: Vec<egui::Event>, t: f64| {
            let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1100.0))), max_texture_side: Some(4096), events, time: Some(t), ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    show(ui, &mut st, &mut project, 0, Some(Play { rt: &mut rt }));
                });
            });
        };
        frame(vec![], 0.0);
        let rects: Vec<(String, Rect)> = ctx.data(|d| d.get_temp(Id::new("design_phone_area").with("rects"))).unwrap();
        let at = rects.iter().find(|(id, _)| id == "button1").unwrap().1.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        frame(vec![egui::Event::PointerMoved(at), press(true)], 0.1);
        frame(vec![press(false)], 0.2);
        frame(vec![], 0.3);
        assert_eq!(rt.widgets["textview1"].text, "1");
        assert_eq!(crate::model::project_snapshot(&project), before);
        assert!(st.drag.is_none() && st.selected.is_none());
    }

    /// A screen taller than the phone scrolls while designing: the wheel
    /// brings the lower widgets up to be picked, and dragging a widget to the
    /// bottom edge scrolls on, so it can be dropped below what fitted.
    #[test]
    fn a_long_screen_scrolls_while_designing() {
        let mut project = crate::app::sample_project(601);
        for i in 0..20 {
            let mut b = Widget::new(format!("b{i}"), WKind::Button, "linear1".into());
            b.text = format!("B{i}");
            b.width = Size::Match;
            project.screens[0].widgets.push(b);
        }
        let mut st = DesignState::default();
        let ctx = Context::default();
        let t = std::cell::Cell::new(0.0);
        let frame = |events: Vec<egui::Event>, st: &mut DesignState, project: &mut Project| {
            t.set(t.get() + 0.05);
            let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1100.0))), max_texture_side: Some(4096), events, time: Some(t.get()), ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    show(ui, st, project, 0, None);
                });
            });
        };
        frame(vec![], &mut st, &mut project);
        frame(vec![], &mut st, &mut project);
        let phone: Rect = ctx.data(|d| d.get_temp(Id::new("design_phone_on_window"))).unwrap();
        // a spot low on the phone's screen
        let at = pos2(phone.center().x, phone.bottom() - phone.height() * 0.1);
        let press = |pressed, at| egui::Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let number = |id: &Option<String>| id.as_deref().and_then(|i| i.strip_prefix('b')).and_then(|n| n.parse::<usize>().ok());
        let pick = |st: &mut DesignState, project: &mut Project| {
            frame(vec![egui::Event::PointerMoved(at)], st, project);
            frame(vec![press(true, at)], st, project);
            frame(vec![press(false, at)], st, project);
            st.selected.clone()
        };
        let before = number(&pick(&mut st, &mut project));
        assert!(before.is_some_and(|n| n < 15), "{before:?}");
        for _ in 0..10 {
            frame(vec![egui::Event::PointerMoved(at), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -300.0), modifiers: Modifiers::NONE }], &mut st, &mut project);
        }
        frame(vec![], &mut st, &mut project);
        let after = number(&pick(&mut st, &mut project));
        assert!(after.is_some_and(|n| n >= 15), "scrolled to the end: {after:?} (was {before:?})");
        // back to the top, then drag the first button to the bottom edge and hold it there
        for _ in 0..10 {
            frame(vec![egui::Event::PointerMoved(at), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, 300.0), modifiers: Modifiers::NONE }], &mut st, &mut project);
        }
        frame(vec![], &mut st, &mut project);
        let top = number(&pick(&mut st, &mut project));
        assert_eq!(top, before, "back at the top");
        // find b0 by picking down the phone from the top
        let mut grab = None;
        for k in 1..40 {
            let p = pos2(phone.center().x, phone.top() + phone.height() * k as f32 / 40.0);
            frame(vec![egui::Event::PointerMoved(p)], &mut st, &mut project);
            frame(vec![press(true, p)], &mut st, &mut project);
            frame(vec![press(false, p)], &mut st, &mut project);
            if st.selected.as_deref() == Some("b0") {
                grab = Some(p);
                break;
            }
        }
        let grab = grab.expect("b0 is on the phone");
        let edge = pos2(phone.center().x, phone.bottom() - phone.height() * 0.09);
        frame(vec![egui::Event::PointerMoved(grab), press(true, grab)], &mut st, &mut project);
        for k in 1..=40 {
            frame(vec![egui::Event::PointerMoved(grab + (edge - grab) * k as f32 / 40.0)], &mut st, &mut project);
        }
        for _ in 0..120 {
            frame(vec![egui::Event::PointerMoved(edge)], &mut st, &mut project);
        }
        frame(vec![press(false, edge)], &mut st, &mut project);
        frame(vec![], &mut st, &mut project);
        // b0 went to the end: the last of the column, or just after it
        let s = &project.screens[0];
        let parent = s.widget("b0").unwrap().parent.clone();
        let order: Vec<String> = s.children(&parent).iter().map(|i| s.widgets[*i].id.clone()).collect();
        let at = order.iter().position(|id| id == "b0").unwrap();
        let near_end = if parent == "linear1" { at + 4 >= order.len() } else { parent == "root" && order.get(at.wrapping_sub(1)).map(String::as_str) == Some("linear1") };
        assert!(near_end, "b0 dropped near the end after scrolling: in {parent} at {at} of {order:?}");
    }

    /// Playing a scrolling column that fills the screen and holds a centred,
    /// full-height column taller than the phone: the content starts at the
    /// top, keeps its own heights, and the column scrolls in all of its
    /// height (it used to stop at 400 points, leaving the rest blank).
    #[test]
    fn a_full_scrolling_column_plays_without_a_gap() {
        let mut project = crate::app::sample_project(601);
        crate::app::scroll_column_screen(&mut project);
        let mut rt = crate::runtime::Runtime::new(project.clone(), 0.0);
        let mut st = DesignState::default();
        let ctx = Context::default();
        for k in 0..6 {
            let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1100.0))), max_texture_side: Some(4096), time: Some(k as f64 * 0.1), ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    show(ui, &mut st, &mut project, 0, Some(Play { rt: &mut rt }));
                });
            });
        }
        let rects: Vec<(String, Rect)> = ctx.data(|d| d.get_temp(Id::new("design_phone_area").with("rects"))).unwrap();
        let r = |id: &str| rects.iter().find(|(i, _)| i == id).unwrap().1;
        let (scroll, cal, video) = (r("vscroll2"), r("calendarview1"), r("videoview1"));
        assert!(cal.top() >= scroll.top() && cal.height() < 300.0, "the calendar is at the top, its own height: {cal:?} in {scroll:?}");
        // one after another, no gaps
        let order = ["calendarview1", "textview1", "seekbar1", "btn", "webview1", "videoview1"];
        for pair in order.windows(2) {
            assert!((r(pair[1]).top() - r(pair[0]).bottom()).abs() < 1.0, "{pair:?}");
        }
        // the column is taller than the scroll area, and the area fills the screen
        assert!(video.bottom() > scroll.bottom(), "{video:?} {scroll:?}");
        assert!(scroll.height() > 600.0, "{scroll:?}");
    }

    #[test]
    fn outline_follows_the_nesting() {
        let mut s = crate::app::sample_project(601).screens[0].clone();
        // a text listed after the row in the widget list, though it belongs
        // to the column before it
        let mut t = s.widget("textview1").unwrap().clone();
        t.id = "late".into();
        s.widgets.push(t);
        let rows: Vec<(String, usize)> = tree_rows(&s).into_iter().map(|(id, _, d)| (id, d)).collect();
        let ids: Vec<&str> = rows.iter().map(|(i, _)| i.as_str()).collect();
        assert_eq!(ids, ["linear1", "textview1", "textview2", "linear2", "button1", "button2", "checkbox1", "late"]);
        assert_eq!(rows[4].1, 2);
        assert_eq!(rows[7].1, 1);
    }

    #[test]
    fn duplicate_a_layout_with_its_widgets() {
        let mut s = crate::app::sample_project(601).screens[0].clone();
        let before = s.widgets.len();
        // linear2 holds button1 and button2
        let copy = subtree(&s, "linear2");
        assert_eq!(copy.len(), 3);
        let n = s.children("linear1").len();
        let id = paste_widgets(&mut s, copy, "linear1", n).unwrap();
        assert_eq!(s.widgets.len(), before + 3);
        assert_eq!(id, "linear3");
        let kids: Vec<String> = s.children(&id).iter().map(|&i| s.widgets[i].id.clone()).collect();
        assert_eq!(kids, vec!["button3", "button4"]);
        // the copies keep their looks
        assert_eq!(s.widget("button3").unwrap().text, "Add one");
    }
}
