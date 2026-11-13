//! The Assets page's viewer: a picture, sound, video, font or data file
//! shown and played inside Blockwise, over the page (no other app).

use crate::icons as ic;
use crate::model::{FileKind, ProjFile};
use crate::player::Media;
use egui::*;
use std::path::PathBuf;

pub struct AssetView {
    pub name: String,
    ext: String,
    kind: FileKind,
    path: PathBuf,
    media: Option<Media>,
    /// The sound or video could not be opened.
    failed: bool,
    /// Picture: None fits it in the window, Some(k) shows it k times its size.
    zoom: Option<f32>,
    volume: f32,
    text: Option<String>,
}

impl AssetView {
    pub fn new(f: &ProjFile, path: PathBuf) -> AssetView {
        let mut v = AssetView { name: f.name.clone(), ext: f.ext.clone(), kind: f.kind, path, media: None, failed: false, zoom: None, volume: 1.0, text: None };
        match f.kind {
            FileKind::Sound | FileKind::Video => {
                v.media = Media::open(&v.path, f.kind == FileKind::Video);
                v.failed = v.media.is_none();
                if let Some(m) = &mut v.media {
                    m.play();
                }
            }
            FileKind::Data => {
                // the start of a big file is enough to read
                v.text = std::fs::read(&v.path).ok().map(|b| {
                    let cut = b.len().min(400_000);
                    let mut t = String::from_utf8_lossy(&b[..cut]).into_owned();
                    if t.starts_with('\u{FEFF}') {
                        t.remove(0);
                    }
                    if cut < b.len() {
                        t.push_str("\n…");
                    }
                    t
                });
            }
            _ => {}
        }
        v
    }
}

/// m:ss, whole seconds gone (0:00 at the start, as players show it); with
/// tenths when the whole file is shorter than 10 seconds.
fn time(secs: f64, length: f64) -> String {
    if length > 0.0 && length < 10.0 {
        let t = (secs.max(0.0) * 10.0).floor() / 10.0;
        return format!("0:{:04.1}", t);
    }
    let s = secs.max(0.0).floor() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Draws the viewer over the page. False once it is closed.
pub fn show(ctx: &Context, v: &mut AssetView) -> bool {
    let mut open = true;
    if let Some(m) = &mut v.media {
        m.tick(ctx);
    }
    let screen = ctx.screen_rect();
    // the page behind, dimmed; a click on it closes the viewer
    Area::new(Id::new("asset_view_dim")).order(Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let r = ui.allocate_rect(screen, Sense::click());
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(140));
        if r.clicked() {
            open = false;
        }
    });
    let size = vec2((screen.width() - 80.0).clamp(320.0, 980.0), (screen.height() - 80.0).clamp(300.0, 740.0));
    Area::new(Id::new("asset_view")).order(Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        Frame::popup(ui.style()).fill(crate::theme::surface()).rounding(10.0).inner_margin(Margin::same(14.0)).show(ui, |ui| {
            // its own size, whatever is inside
            ui.set_width(size.x - 28.0);
            ui.set_max_width(size.x - 28.0);
            ui.set_height(size.y - 28.0);
            // name, what it is, and the close button
            ui.horizontal(|ui| {
                ui.label(RichText::new(crate::app::file_icon(v.kind)).size(18.0));
                ui.label(RichText::new(&v.name).strong().size(16.0));
                ui.label(RichText::new(format!(".{}", v.ext)).color(Color32::GRAY));
                if let Some(n) = std::fs::metadata(&v.path).ok().map(|m| m.len()) {
                    ui.label(RichText::new(format!("  ·  {}", crate::app::human_size(n))).small().color(Color32::GRAY));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(Button::new(RichText::new(ic::X).size(16.0)).frame(false)).on_hover_text("Close (Esc)").clicked() {
                        open = false;
                    }
                });
            });
            ui.add_space(8.0);
            let controls = if matches!(v.kind, FileKind::Sound | FileKind::Video | FileKind::Image) { 40.0 } else { 0.0 };
            let body = vec2(ui.available_width(), ui.available_height() - controls);
            match v.kind {
                FileKind::Image => picture(ui, v, body),
                FileKind::Video => video(ui, v, body),
                FileKind::Sound => sound(ui, v, body),
                FileKind::Font => font(ui, v, body),
                FileKind::Data => text(ui, v, body),
            }
            if controls > 0.0 {
                ui.add_space(6.0);
                if v.kind == FileKind::Image {
                    zoom_bar(ui, v);
                } else {
                    play_bar(ui, v);
                }
            }
        });
    });
    // Esc closes; space plays and pauses
    if ctx.memory(|m| m.focused().is_none()) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            open = false;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
            if let Some(m) = &mut v.media {
                m.toggle();
            }
        }
    }
    open
}

/// A checked background, so the clear parts of a picture show.
fn checks(p: &Painter, r: Rect) {
    p.rect_filled(r, 6.0, Color32::from_gray(236));
    let s = 12.0;
    let (nx, ny) = ((r.width() / s).ceil() as i32, (r.height() / s).ceil() as i32);
    for y in 0..ny {
        for x in 0..nx {
            if (x + y) % 2 == 0 {
                let c = Rect::from_min_size(r.min + vec2(x as f32 * s, y as f32 * s), vec2(s, s)).intersect(r);
                p.rect_filled(c, 0.0, Color32::from_gray(222));
            }
        }
    }
}

fn picture(ui: &mut Ui, v: &mut AssetView, body: Vec2) {
    let Some(t) = crate::phone::picture(ui.ctx(), &v.path) else {
        missing(ui, body, "This picture can't be read");
        return;
    };
    let n = t.size_vec2();
    let fit = (body.x / n.x).min(body.y / n.y).min(4.0);
    let k = v.zoom.unwrap_or(fit);
    let shown = n * k;
    let (outer, _) = ui.allocate_exact_size(body, Sense::hover());
    ui.painter().rect_filled(outer, 6.0, crate::theme::bg());
    let mut child = ui.new_child(UiBuilder::new().max_rect(outer));
    ScrollArea::both().id_salt("asset_view_pic").auto_shrink([false, false]).show(&mut child, |ui| {
        let space = vec2(shown.x.max(outer.width()), shown.y.max(outer.height()));
        let (r, resp) = ui.allocate_exact_size(space, Sense::click());
        let img = Rect::from_center_size(r.center(), shown);
        checks(ui.painter(), img);
        ui.painter().image(t.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        // a double click goes between fitting and its own size
        if resp.double_clicked() {
            v.zoom = if v.zoom.is_some() { None } else { Some(1.0) };
        }
    });
}

fn zoom_bar(ui: &mut Ui, v: &mut AssetView) {
    let n = crate::phone::picture(ui.ctx(), &v.path).map(|t| t.size_vec2());
    ui.horizontal(|ui| {
        if ui.selectable_label(v.zoom.is_none(), "Fit").clicked() {
            v.zoom = None;
        }
        if ui.selectable_label(v.zoom == Some(1.0), "100%").clicked() {
            v.zoom = Some(1.0);
        }
        let now = v.zoom.unwrap_or(1.0);
        if ui.button("−").on_hover_text("Smaller").clicked() {
            v.zoom = Some((now / 1.5).max(0.05));
        }
        if ui.button("+").on_hover_text("Bigger").clicked() {
            v.zoom = Some((now * 1.5).min(16.0));
        }
        if let Some(k) = v.zoom {
            ui.label(RichText::new(format!("{:.0}%", k * 100.0)).color(Color32::GRAY));
        }
        if let Some(n) = n {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{} × {} pixels (dp on the phones)", n.x, n.y)).color(Color32::GRAY));
            });
        }
    });
}

fn missing(ui: &mut Ui, body: Vec2, why: &str) {
    let (r, _) = ui.allocate_exact_size(body, Sense::hover());
    ui.painter().rect_filled(r, 6.0, crate::theme::bg());
    ui.painter().text(r.center(), Align2::CENTER_CENTER, why, FontId::proportional(15.0), Color32::GRAY);
}

fn video(ui: &mut Ui, v: &mut AssetView, body: Vec2) {
    let (r, resp) = ui.allocate_exact_size(body, Sense::click());
    ui.painter().rect_filled(r, 6.0, Color32::BLACK);
    let Some(m) = &mut v.media else {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "This video can't be played here", FontId::proportional(15.0), Color32::GRAY);
        return;
    };
    match m.picture() {
        Some(t) => {
            let n = t.size_vec2();
            let k = (r.width() / n.x).min(r.height() / n.y);
            let img = Rect::from_center_size(r.center(), n * k);
            ui.painter().image(t.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "Loading…", FontId::proportional(15.0), Color32::GRAY);
        }
    }
    // a big play sign while it is stopped
    if !m.is_playing() && m.picture().is_some() {
        ui.painter().circle_filled(r.center(), 34.0, Color32::from_black_alpha(140));
        ui.painter().text(r.center() + vec2(3.0, 0.0), Align2::CENTER_CENTER, ic::PLAY, FontId::proportional(30.0), Color32::WHITE);
    }
    if resp.clicked() {
        m.toggle();
    }
}

fn sound(ui: &mut Ui, v: &mut AssetView, body: Vec2) {
    let (r, resp) = ui.allocate_exact_size(body, Sense::click());
    ui.painter().rect_filled(r, 6.0, crate::theme::bg());
    let Some(m) = &mut v.media else {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "This sound can't be played here", FontId::proportional(15.0), Color32::GRAY);
        return;
    };
    let playing = m.is_playing();
    let c = r.center();
    // rings that breathe while it plays
    if playing {
        let t = ui.input(|i| i.time) as f32;
        for k in 0..3 {
            let phase = (t * 0.8 + k as f32 / 3.0).fract();
            ui.painter().circle_stroke(c, 60.0 + phase * 70.0, Stroke::new(2.0_f32, crate::phone::PRIMARY.gamma_multiply(1.0 - phase)));
        }
    }
    ui.painter().circle_filled(c, 56.0, crate::phone::PRIMARY);
    ui.painter().text(c, Align2::CENTER_CENTER, if playing { ic::PAUSE } else { ic::MUSIC }, FontId::proportional(40.0), Color32::WHITE);
    if resp.clicked() {
        m.toggle();
    }
}

fn play_bar(ui: &mut Ui, v: &mut AssetView) {
    let Some(m) = &mut v.media else {
        return;
    };
    ui.horizontal(|ui| {
        let playing = m.is_playing();
        let b = Button::new(RichText::new(if playing { ic::PAUSE } else { ic::PLAY }).size(16.0).color(Color32::WHITE)).fill(crate::phone::PRIMARY).rounding(16.0).min_size(vec2(36.0, 30.0));
        if ui.add(b).on_hover_text(if playing { "Pause (space)" } else { "Play (space)" }).clicked() {
            m.toggle();
        }
        let d = m.duration();
        let mut pos = m.position().min(d.max(0.0));
        ui.label(RichText::new(time(pos, d)).monospace());
        // the rest of the row: where it is, then length, loop and volume
        let right = 250.0;
        ui.spacing_mut().slider_width = (ui.available_width() - right).max(60.0);
        let slider = ui.add_enabled(d > 0.0, Slider::new(&mut pos, 0.0..=d.max(0.001)).show_value(false));
        if slider.changed() {
            m.seek(pos);
        }
        ui.label(RichText::new(if d > 0.0 { time(d, d) } else { "–:––".into() }).monospace());
        let mut again = m.looping();
        if ui.toggle_value(&mut again, ic::REFRESH).on_hover_text("Again and again").changed() {
            m.set_looping(again);
        }
        ui.label(RichText::new(if v.volume == 0.0 { "🔇" } else { "🔊" }).color(Color32::GRAY));
        ui.spacing_mut().slider_width = 90.0;
        if ui.add(Slider::new(&mut v.volume, 0.0..=1.0).show_value(false)).changed() {
            m.set_volume(v.volume);
        }
    });
}

fn font(ui: &mut Ui, v: &mut AssetView, body: Vec2) {
    let fam = crate::phone::project_font(ui.ctx(), &v.name);
    ScrollArea::vertical().id_salt("asset_view_font").max_height(body.y).auto_shrink([false, false]).show(ui, |ui| {
        ui.label(RichText::new("ABCDEFGHIJKLMNOPQRSTUVWXYZ\nabcdefghijklmnopqrstuvwxyz\n0123456789 !?&@#%()[]{}.,;:'\"").family(fam.clone()).size(26.0));
        ui.add_space(12.0);
        for size in [12.0, 16.0, 20.0, 28.0, 40.0, 56.0] {
            ui.label(RichText::new(format!("{size:.0} points")).small().color(Color32::GRAY));
            ui.add(Label::new(RichText::new("The quick brown fox jumps over the lazy dog").family(fam.clone()).size(size)).wrap());
            ui.add_space(6.0);
        }
    });
}

fn text(ui: &mut Ui, v: &mut AssetView, body: Vec2) {
    Frame::none().fill(crate::theme::bg()).rounding(6.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
        ScrollArea::both().id_salt("asset_view_text").max_height(body.y - 20.0).auto_shrink([false, false]).show(ui, |ui| match &v.text {
            Some(t) if !t.is_empty() => {
                ui.add(Label::new(RichText::new(t).monospace().size(12.5)).wrap_mode(TextWrapMode::Extend));
            }
            Some(_) => {
                ui.label(RichText::new("The file is empty").color(Color32::GRAY));
            }
            None => {
                ui.label(RichText::new("This file can't be read").color(Color32::GRAY));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn times_count_whole_seconds() {
        assert_eq!(super::time(0.0, 75.0), "0:00");
        assert_eq!(super::time(0.99, 75.0), "0:00");
        assert_eq!(super::time(6.0, 75.0), "0:06");
        assert_eq!(super::time(75.4, 75.0), "1:15");
        // short ones: tenths
        assert_eq!(super::time(0.0, 0.5), "0:00.0");
        assert_eq!(super::time(0.47, 0.5), "0:00.4");
        assert_eq!(super::time(6.0, 6.0), "0:06.0");
    }
}
