//! Draws a screen's widgets as a phone would show them: in the designer (static, selectable)
//! and in the pretend phone (live, clickable).

use crate::model::*;
use crate::runtime::{Val, WState};
use egui::*;
#[allow(unused_imports)]
use crate::model::{Event, Widget};
use std::collections::HashMap;

pub const PHONE_W: f32 = 360.0;
pub const PHONE_H: f32 = 640.0;
pub const PRIMARY: Color32 = Color32::from_rgb(0x5B, 0x5B, 0xD6);
pub const PRIMARY_DARK: Color32 = Color32::from_rgb(0x4A, 0x4A, 0xC4);

pub fn c32(argb: u32) -> Color32 {
    let a = (argb >> 24) as u8;
    Color32::from_rgba_unmultiplied((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, a)
}

pub fn argb(c: Color32) -> u32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Design,
    Run,
}

pub struct ContainerHit {
    pub id: String,
    pub rect: Rect,
    pub horizontal: bool,
    pub children: Vec<(String, Rect)>,
}

#[derive(Default)]
pub struct PhoneOut {
    /// Every drawn widget: id, rect, depth.
    pub rects: Vec<(String, Rect, usize)>,
    pub containers: Vec<ContainerHit>,
    /// Events to fire: target, event name, event values.
    pub events: Vec<(String, String, HashMap<String, Val>)>,
}

impl PhoneOut {
    /// Deepest widget under a point.
    pub fn widget_at(&self, p: Pos2) -> Option<String> {
        self.rects.iter().filter(|(_, r, _)| r.contains(p)).max_by_key(|(_, _, d)| *d).map(|(id, _, _)| id.clone())
    }
}

/// The phone body with status bar and title bar. Returns the content rect.
pub fn phone_frame(ui: &mut Ui, title: &str, size: Vec2) -> (Rect, Response) {
    let (outer, resp) = ui.allocate_exact_size(size + vec2(24.0, 60.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(outer, 28.0, Color32::from_rgb(0x26, 0x2A, 0x33));
    let screen = Rect::from_min_size(outer.min + vec2(12.0, 30.0), size);
    p.rect_filled(screen, 2.0, Color32::WHITE);
    // speaker and home dot
    p.rect_filled(Rect::from_center_size(pos2(outer.center().x, outer.top() + 15.0), vec2(50.0, 5.0)), 3.0, Color32::from_gray(80));
    p.circle_stroke(pos2(outer.center().x, outer.bottom() - 15.0), 8.0, Stroke::new(1.5_f32, Color32::from_gray(90)));
    // status bar
    let status = Rect::from_min_size(screen.min, vec2(size.x, 22.0));
    p.rect_filled(status, 0.0, PRIMARY_DARK);
    p.text(status.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, "12:00", FontId::proportional(11.0), Color32::WHITE);
    p.rect_filled(Rect::from_min_size(status.right_center() - vec2(56.0, 4.0), vec2(14.0, 8.0)), 1.0, Color32::WHITE);
    // title bar
    let bar = Rect::from_min_size(status.left_bottom(), vec2(size.x, 50.0));
    p.rect_filled(bar, 0.0, PRIMARY);
    p.text(bar.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(18.0), Color32::WHITE);
    let content = Rect::from_min_max(bar.left_bottom(), screen.max);
    (content, resp)
}

// ----- which phone the designer and the pretend phone show

static PREVIEW_IPHONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_preview_iphone(on: bool) {
    PREVIEW_IPHONE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn preview_iphone() -> bool {
    PREVIEW_IPHONE.load(std::sync::atomic::Ordering::Relaxed) && crate::device::ios_tools()
}

/// The iPhone picture: its texture, its size in pixels, where its screen is,
/// and a mask in the exact shape of that screen.
#[derive(Clone)]
struct FrameArt {
    tex: TextureHandle,
    size: Vec2,
    hole: Rect,
    /// White where the screen shows, see-through elsewhere; covers `hole`.
    mask: TextureHandle,
}

/// The iPhone picture, drawn here rather than shipped as an image: 1350×2760
/// pixels with a 1206×2622 screen (a 402×874 pt screen at 3×), a titanium
/// band, a black bezel, the side buttons and the camera island. The screen
/// and everything outside the phone are see-through.
fn drawn_iphone() -> (usize, usize, Vec<u8>) {
    const W: usize = 1350;
    const H: usize = 2760;
    const SW: f32 = 1206.0;
    const SH: f32 = 2622.0;
    const SCREEN_R: f32 = 165.0;
    let sx = (W as f32 - SW) / 2.0;
    let sy = (H as f32 - SH) / 2.0;
    // how much of a pixel (x, y) a rounded rectangle covers: 0..1, smooth edge
    fn cover(x: f32, y: f32, r: [f32; 4], rad: f32) -> f32 {
        let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        let (hw, hh) = ((r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0);
        let rad = rad.min(hw).min(hh);
        let qx = (x - cx).abs() - (hw - rad);
        let qy = (y - cy).abs() - (hh - rad);
        let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
        (0.5 - d).clamp(0.0, 1.0)
    }
    let side = 9.0;
    let body = [side, 0.0, W as f32 - side, H as f32];
    let body_r = SCREEN_R + sx - side;
    let inset = |r: [f32; 4], k: f32| [r[0] + k, r[1] + k, r[2] - k, r[3] - k];
    let screen = [sx, sy, sx + SW, sy + SH];
    let island = [W as f32 / 2.0 - 186.0, sy + 33.0, W as f32 / 2.0 + 186.0, sy + 33.0 + 110.0];
    // buttons: action and volume on the left, power on the right (as 0..1 of the height)
    let buttons = [(0.0, 0.205, 0.245), (0.0, 0.29, 0.355), (0.0, 0.375, 0.44), (1.0, 0.31, 0.415)];
    // (shape, radius, colour), painted in order, each over the last
    let mut layers: Vec<([f32; 4], f32, [f32; 3])> = vec![];
    for (right, t, b) in buttons {
        let x = if right > 0.5 { [W as f32 - side - 4.0, W as f32] } else { [0.0, side + 4.0] };
        layers.push(([x[0], H as f32 * t, x[1], H as f32 * b], 5.0, [0x8C as f32, 0x8F as f32, 0x94 as f32]));
    }
    for (k, c) in [(0.0, [0x5A, 0x5D, 0x62]), (3.0, [0xB9, 0xBC, 0xC1]), (8.0, [0xA3, 0xA6, 0xAB]), (13.0, [0xCF, 0xD2, 0xD6]), (16.0, [0x0A, 0x0A, 0x0C])] {
        layers.push((inset(body, k), body_r - k, c.map(|v: i32| v as f32)));
    }
    let mut px = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let (mut r, mut g, mut b, mut a) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
            for (shape, rad, c) in &layers {
                let k = cover(fx, fy, *shape, *rad);
                if k > 0.0 {
                    let na = k + a * (1.0 - k);
                    r = (c[0] * k + r * a * (1.0 - k)) / na;
                    g = (c[1] * k + g * a * (1.0 - k)) / na;
                    b = (c[2] * k + b * a * (1.0 - k)) / na;
                    a = na;
                }
            }
            // the screen is cut out of the bezel; the island sits on the screen
            let hole = cover(fx, fy, screen, SCREEN_R);
            let isl = cover(fx, fy, island, 55.0);
            a *= 1.0 - hole * (1.0 - isl);
            if isl > 0.0 {
                r *= 1.0 - isl;
                g *= 1.0 - isl;
                b *= 1.0 - isl;
            }
            let i = (y * W + x) * 4;
            px[i..i + 4].copy_from_slice(&[r as u8, g as u8, b as u8, (a * 255.0).round() as u8]);
        }
    }
    (W, H, px)
}

/// Find the see-through screen of the frame picture: walk out from the middle
/// until the pixels turn solid.
fn find_hole(w: usize, h: usize, rgba: &[u8]) -> Rect {
    let alpha = |x: usize, y: usize| rgba[(y * w + x) * 4 + 3];
    let (cx, cy) = (w / 2, h / 2);
    let mut l = cx;
    while l > 0 && alpha(l - 1, cy) < 128 {
        l -= 1;
    }
    let mut r = cx;
    while r + 1 < w && alpha(r + 1, cy) < 128 {
        r += 1;
    }
    // a column away from the camera cut-out at the top
    let sx = l + (r - l) / 5;
    let mut t = cy;
    while t > 0 && alpha(sx, t - 1) < 128 {
        t -= 1;
    }
    let mut b = cy;
    while b + 1 < h && alpha(sx, b + 1) < 128 {
        b += 1;
    }
    Rect::from_min_max(pos2(l as f32, t as f32), pos2(r as f32 + 1.0, b as f32 + 1.0))
}

/// The screen's exact shape inside `hole`: the see-through pixels reached from
/// the middle, grown by two pixels so it tucks under the bezel's soft edge.
/// The see-through area outside the phone never joins in, so fills can't
/// spill past the round corners.
fn screen_mask(w: usize, rgba: &[u8], hole: Rect) -> ColorImage {
    let (x0, y0) = (hole.min.x as usize, hole.min.y as usize);
    let (mw, mh) = (hole.width() as usize, hole.height() as usize);
    let clear = |x: usize, y: usize| rgba[((y0 + y) * w + x0 + x) * 4 + 3] < 128;
    let mut on = vec![false; mw * mh];
    let mut todo = vec![(mw / 2, mh / 2)];
    while let Some((x, y)) = todo.pop() {
        if on[y * mw + x] || !clear(x, y) {
            continue;
        }
        on[y * mw + x] = true;
        if x > 0 { todo.push((x - 1, y)); }
        if y > 0 { todo.push((x, y - 1)); }
        if x + 1 < mw { todo.push((x + 1, y)); }
        if y + 1 < mh { todo.push((x, y + 1)); }
    }
    for _ in 0..2 {
        let was = on.clone();
        for y in 0..mh {
            for x in 0..mw {
                if !was[y * mw + x] && ((x > 0 && was[y * mw + x - 1]) || (x + 1 < mw && was[y * mw + x + 1]) || (y > 0 && was[(y - 1) * mw + x]) || (y + 1 < mh && was[(y + 1) * mw + x])) {
                    on[y * mw + x] = true;
                }
            }
        }
    }
    ColorImage { size: [mw, mh], pixels: on.iter().map(|&o| if o { Color32::WHITE } else { Color32::TRANSPARENT }).collect() }
}

fn iphone_art(ctx: &Context) -> Option<FrameArt> {
    let id = Id::new("iphone_art");
    if let Some(a) = ctx.data(|d| d.get_temp::<Option<FrameArt>>(id)) {
        return a;
    }
    let art = {
        let (w, h, px) = drawn_iphone();
        let hole = find_hole(w, h, &px);
        let mask = ctx.load_texture("iphone_mask", screen_mask(w, &px, hole), TextureOptions::LINEAR);
        let ci = ColorImage::from_rgba_unmultiplied([w, h], &px);
        let tex = ctx.load_texture("iphone_frame", ci, TextureOptions::LINEAR);
        Some(FrameArt { tex, size: vec2(w as f32, h as f32), hole, mask })
    };
    ctx.data_mut(|d| d.insert_temp(id, art.clone()));
    art
}

/// Paint `part` of the screen (`hole`, in window coordinates) in `color`,
/// following the screen's round corners.
fn fill_screen(p: &Painter, a: &FrameArt, hole: Rect, part: Rect, color: Color32) {
    let part = part.intersect(hole);
    let uv = Rect::from_min_max(((part.min - hole.min) / hole.size()).to_pos2(), ((part.max - hole.min) / hole.size()).to_pos2());
    p.image(a.mask.id(), part, uv, color);
}

/// A phone drawn on screen: where its content goes and what to finish with.
pub struct Shell {
    pub content: Rect,
    pub outer: Rect,
    /// The colour under the home bar: the screen's own background
    /// (see `bottom_color`), white unless the caller says otherwise.
    pub bottom: Color32,
    screen: Rect,
    kind: ShellKind,
}

enum ShellKind {
    Iphone(FrameArt),
    Pixel,
    Plain,
}

/// Status bar and navigation bar heights of the iPhone, in dp at 360 wide
/// (scaled from 62 pt and 44 pt on a 402 pt wide screen; estimated).
const IOS_STATUS: f32 = 54.0;
const IOS_NAV: f32 = 44.0;
const IOS_HOME: f32 = 30.0;

/// The Pixel, drawn with shapes so it stays sharp at any zoom (dp). A 20:9
/// screen, a thin black bezel, a metal rim and two buttons on the right.
mod pixel {
    pub const SCREEN_H: f32 = 800.0;
    pub const SCREEN_R: f32 = 34.0;
    pub const BEZEL: f32 = 9.0;
    pub const RIM: f32 = 4.0;
    pub const BUTTONS: f32 = 3.0;
    pub const STATUS: f32 = 44.0;
    pub const BAR: f32 = 56.0;
    /// The gesture bar strip: as tall as the corner radius (see `shell_end`).
    pub const GESTURE: f32 = SCREEN_R;
    pub const BEZEL_COLOR: egui::Color32 = egui::Color32::from_rgb(0x08, 0x08, 0x0A);
}

/// Size the phone takes on screen, frame included.
pub fn shell_size(ctx: &Context) -> Vec2 {
    if preview_iphone() {
        if let Some(a) = iphone_art(ctx) {
            let k = PHONE_W / a.hole.width();
            return a.size * k;
        }
        return vec2(PHONE_W, PHONE_H) + vec2(24.0, 60.0);
    }
    let edge = pixel::BEZEL + pixel::RIM;
    vec2(PHONE_W + edge * 2.0 + pixel::BUTTONS, pixel::SCREEN_H + edge * 2.0)
}

/// Start drawing the phone the settings pick (the iPhone picture or the
/// Pixel). Draw the screen into `content`, then call `shell_end`.
pub fn shell_begin(ui: &mut Ui, title: &str) -> Shell {
    if preview_iphone() {
        let Some(a) = iphone_art(ui.ctx()) else {
            let (content, resp) = phone_frame(ui, title, vec2(PHONE_W, PHONE_H));
            return Shell { content, outer: resp.rect, bottom: Color32::WHITE, screen: content, kind: ShellKind::Plain };
        };
        let k = PHONE_W / a.hole.width();
        let (outer, _) = ui.allocate_exact_size(a.size * k, Sense::hover());
        let hole = Rect::from_min_size(outer.min + a.hole.min.to_vec2() * k, a.hole.size() * k);
        let p = ui.painter();
        fill_screen(p, &a, hole, hole, Color32::WHITE);
        let nav = Rect::from_min_size(hole.min + vec2(0.0, IOS_STATUS), vec2(hole.width(), IOS_NAV));
        fill_screen(p, &a, hole, Rect::from_min_max(hole.min, nav.max), Color32::from_rgb(0xF9, 0xF9, 0xF9));
        // status bar: time on the left, battery on the right, beside the camera cut-out
        let status = Rect::from_min_size(hole.min, vec2(hole.width(), IOS_STATUS));
        let ink = Color32::from_rgb(0x11, 0x11, 0x11);
        p.text(pos2(status.left() + hole.width() * 0.16, status.center().y + 2.0), Align2::CENTER_CENTER, "9:41", FontId::proportional(15.0), ink);
        let bat = Rect::from_center_size(pos2(status.right() - hole.width() * 0.13, status.center().y + 2.0), vec2(22.0, 11.0));
        p.rect_stroke(bat, 3.0, Stroke::new(1.0_f32, Color32::from_gray(150)));
        p.rect_filled(bat.shrink(2.0), 1.5, ink);
        p.rect_filled(Rect::from_min_size(pos2(bat.right() + 1.0, bat.center().y - 2.0), vec2(1.5, 4.0)), 0.5, Color32::from_gray(150));
        // navigation bar with the title in the middle
        p.text(nav.center(), Align2::CENTER_CENTER, title, FontId::proportional(17.0), ink);
        p.line_segment([nav.left_bottom(), nav.right_bottom()], Stroke::new(0.5_f32, Color32::from_gray(200)));
        let content = Rect::from_min_max(nav.left_bottom() + vec2(0.0, 0.5), pos2(hole.right(), hole.bottom() - IOS_HOME));
        return Shell { content, outer, bottom: Color32::WHITE, screen: hole, kind: ShellKind::Iphone(a) };
    }

    let (outer, _) = ui.allocate_exact_size(shell_size(ui.ctx()), Sense::hover());
    let p = ui.painter();
    let body = Rect::from_min_size(outer.min, outer.size() - vec2(pixel::BUTTONS, 0.0));
    let edge = pixel::BEZEL + pixel::RIM;
    let r_body = pixel::SCREEN_R + edge;
    // buttons first so the rim overlaps their inner edge: power, then volume
    for (top, h) in [(0.24, 0.07), (0.36, 0.14)] {
        let b = Rect::from_min_size(pos2(body.right() - 2.0, body.top() + body.height() * top), vec2(pixel::BUTTONS + 2.0, body.height() * h));
        p.rect_filled(b, 1.5, Color32::from_rgb(0xA9, 0xAD, 0xB3));
    }
    // metal rim: a darker outer line, the satin band and a bright inner edge
    fill_rounded(p, body, [r_body; 4], Color32::from_rgb(0x6E, 0x73, 0x7A));
    fill_rounded(p, body.shrink(0.8), [r_body - 0.8; 4], Color32::from_rgb(0xB4, 0xB9, 0xC0));
    fill_rounded(p, body.shrink(1.8), [r_body - 1.8; 4], Color32::from_rgb(0xD3, 0xD7, 0xDC));
    fill_rounded(p, body.shrink(pixel::RIM - 0.9), [r_body - pixel::RIM + 0.9; 4], Color32::from_rgb(0x9A, 0x9F, 0xA6));
    let glass = body.shrink(pixel::RIM);
    fill_rounded(p, glass, [r_body - pixel::RIM; 4], pixel::BEZEL_COLOR);
    let screen = glass.shrink(pixel::BEZEL);
    let r = pixel::SCREEN_R;
    fill_rounded(p, screen, [r; 4], Color32::WHITE);
    // status bar in the dark theme colour, app bar in the theme colour
    let status = Rect::from_min_size(screen.min, vec2(screen.width(), pixel::STATUS));
    let bar = Rect::from_min_size(status.left_bottom(), vec2(screen.width(), pixel::BAR));
    fill_rounded(p, status, [r, r, 0.0, 0.0], PRIMARY_DARK);
    p.rect_filled(bar, 0.0, PRIMARY);
    let y = status.center().y + 1.0;
    p.text(pos2(status.left() + 24.0, y), Align2::LEFT_CENTER, "12:00", FontId::proportional(13.0), Color32::WHITE);
    let bat = Rect::from_center_size(pos2(status.right() - 28.0, y), vec2(7.5, 13.0));
    p.rect_filled(bat, 1.5, Color32::WHITE);
    p.rect_filled(Rect::from_center_size(pos2(bat.center().x, bat.top() - 1.0), vec2(3.5, 2.0)), 0.5, Color32::WHITE);
    let sig = pos2(bat.left() - 7.0, bat.bottom());
    p.add(Shape::convex_polygon(vec![sig, sig + vec2(-11.0, 0.0), sig + vec2(0.0, -11.0)], Color32::WHITE, Stroke::NONE));
    p.text(bar.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(19.0), Color32::WHITE);
    let content = Rect::from_min_max(bar.left_bottom(), pos2(screen.right(), screen.bottom() - pixel::GESTURE));
    Shell { content, outer, bottom: Color32::WHITE, screen, kind: ShellKind::Pixel }
}

/// A rounded rectangle as a convex outline, each corner with its own radius.
/// Unlike `rect_filled`, a radius may exceed half the height, so a short strip
/// can still follow the screen's big corners.
fn rounded(r: Rect, [nw, ne, se, sw]: [f32; 4]) -> Vec<Pos2> {
    let mut pts = vec![];
    for (rad, centre, a0) in [
        (nw, r.left_top() + vec2(nw, nw), 180.0_f32),
        (ne, r.right_top() + vec2(-ne, ne), 270.0),
        (se, r.right_bottom() + vec2(-se, -se), 0.0),
        (sw, r.left_bottom() + vec2(sw, -sw), 90.0),
    ] {
        let steps = if rad > 0.0 { 14 } else { 0 };
        for i in 0..=steps {
            let a = (a0 + 90.0 * i as f32 / steps.max(1) as f32).to_radians();
            pts.push(centre + vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts
}

fn fill_rounded(p: &Painter, r: Rect, corners: [f32; 4], color: Color32) {
    p.add(Shape::convex_polygon(rounded(r, corners), color, Stroke::NONE));
}

/// The colour a screen shows at its very bottom: the background of a root
/// layout that fills the height, or white.
pub fn bottom_color(screen: &Screen, states: &HashMap<String, WState>) -> Color32 {
    let mut c = Color32::WHITE;
    for i in screen.children("root") {
        let w = &screen.widgets[i];
        let bg = states.get(&w.id).map(|s| s.bg).unwrap_or(w.bg);
        if w.height == Size::Match && bg >> 24 == 0xFF {
            c = c32(bg);
        }
    }
    c
}

/// Fill the home bar's strip with the screen's colour, round the screen's
/// corners, lay the frame on top and draw the home bar.
pub fn shell_end(ui: &Ui, sh: &Shell) {
    let p = ui.painter();
    let strip = Rect::from_min_max(pos2(sh.screen.left(), sh.content.bottom()), sh.screen.max);
    let [r, g, b, _] = sh.bottom.to_array();
    let light = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000 > 140;
    let pill = if light { Color32::from_rgb(0x11, 0x11, 0x11) } else { Color32::from_rgb(0xF2, 0xF2, 0xF2) };
    match &sh.kind {
        ShellKind::Iphone(a) => {
            fill_screen(p, a, sh.screen, strip, sh.bottom);
            p.image(a.tex.id(), sh.outer, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            p.rect_filled(Rect::from_center_size(pos2(sh.screen.center().x, sh.screen.bottom() - 10.0), vec2(sh.screen.width() * 0.34, 5.0)), 2.5, pill);
        }
        ShellKind::Pixel => {
            // the strip is as tall as the corners are round, so the content
            // above it never reaches a curve
            fill_rounded(p, strip, [0.0, 0.0, pixel::SCREEN_R, pixel::SCREEN_R], sh.bottom);
            // the punch-hole camera
            let cam = pos2(sh.screen.center().x, sh.screen.top() + pixel::STATUS / 2.0 + 1.0);
            p.circle_filled(cam, 9.5, Color32::from_rgb(0x02, 0x02, 0x03));
            p.circle_stroke(cam, 9.5, Stroke::new(0.8_f32, Color32::from_rgb(0x2A, 0x2C, 0x33)));
            p.circle_filled(cam, 4.2, Color32::from_rgb(0x12, 0x15, 0x24));
            p.circle_filled(cam + vec2(-1.5, -1.5), 1.2, Color32::from_rgba_unmultiplied(0x9A, 0xA8, 0xFF, 120));
            p.rect_filled(Rect::from_center_size(pos2(sh.screen.center().x, sh.screen.bottom() - pixel::GESTURE / 2.0), vec2(sh.screen.width() * 0.30, 4.0)), 2.0, pill);
        }
        ShellKind::Plain => {}
    }
}

/// Draw `size` worth of content (the phone) scaled down, never up, so it fits in
/// `canvas`, centred. It lives on its own layer whose transform egui also
/// applies to clicks and drags, so widgets inside work as usual. Returns what
/// `add` returns and the transform (layer coordinates → window coordinates).
pub fn fit_area<R>(ctx: &Context, id: &str, canvas: Rect, size: Vec2, add: impl FnOnce(&mut Ui) -> R) -> (Option<R>, emath::TSTransform) {
    let margin = 20.0;
    let scale = ((canvas.width() - margin * 2.0) / size.x).min((canvas.height() - margin * 2.0) / size.y).clamp(0.35, 1.0);
    let at = canvas.center() - size * scale / 2.0;
    let t = emath::TSTransform::new(at.to_vec2(), scale);
    let area_id = Id::new(id);
    ctx.set_transform_layer(LayerId::new(Order::Middle, area_id), t);
    let r = Area::new(area_id).order(Order::Middle).fixed_pos(Pos2::ZERO).constrain(false).show(ctx, |ui| {
        ui.set_clip_rect(Rect::from_min_size(Pos2::ZERO, size + vec2(4.0, 4.0)));
        add(ui)
    });
    (Some(r.inner), t)
}

fn font(ui: &Ui, size_sp: f32, family: &str) -> FontId {
    FontId::new((size_sp * 1.15).clamp(5.0, 110.0), project_font(ui.ctx(), family))
}

/// A project font's family once it is loaded (it loads a frame after
/// `sync_fonts` sees it), else the usual letters.
pub fn project_font(ctx: &Context, name: &str) -> FontFamily {
    if name.is_empty() {
        return FontFamily::Proportional;
    }
    let fam = FontFamily::Name(format!("project:{name}").into());
    if ctx.fonts(|f| f.families().contains(&fam)) {
        fam
    } else {
        FontFamily::Proportional
    }
}

/// Loads the open project's font files into the window's fonts, when they
/// change. A file that isn't a font Blockwise can read is skipped.
pub fn sync_fonts(ctx: &Context, project: &crate::model::Project) {
    let list: Vec<(String, std::path::PathBuf, Option<std::time::SystemTime>)> = project
        .file_paths()
        .into_iter()
        .filter(|(f, _)| f.kind == crate::model::FileKind::Font)
        .map(|(f, p)| (f.name.clone(), p.clone(), std::fs::metadata(&p).and_then(|m| m.modified()).ok()))
        .collect();
    let key = Id::new("project-fonts");
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        list.hash(&mut h);
        h.finish()
    };
    if ctx.data(|d| d.get_temp::<u64>(key)) == Some(hash) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, hash));
    let fonts = list.into_iter().filter_map(|(n, p, _)| std::fs::read(p).ok().filter(|b| crate::model::font_ok(b)).map(|b| (n, b))).collect();
    crate::icons::install_with(ctx, fonts);
}

/// How much wider bold letters are than the plain ones they are drawn from.
fn bold_extra(g: &Galley, bold: bool) -> f32 {
    if bold {
        (g.size().y * 0.045).clamp(0.6, 3.0)
    } else {
        0.0
    }
}

/// A text's size, room for bold included.
fn text_size(g: &Galley, bold: bool) -> Vec2 {
    g.size() + vec2(bold_extra(g, bold), 0.0)
}

/// Draws a text; bold is the plain letters drawn a few times side by side,
/// a hair apart (egui has no bold font, and this thickens any font, the
/// project's own ones too, as the phones do for a font without a bold face).
fn paint_text(p: &Painter, pos: Pos2, g: std::sync::Arc<Galley>, c: Color32, bold: bool) {
    let extra = bold_extra(&g, bold);
    if extra > 0.0 {
        let steps = (extra / 0.3).ceil().max(2.0) as usize;
        for i in 1..=steps {
            p.galley(pos + vec2(extra * i as f32 / steps as f32, 0.0), g.clone(), c);
        }
    }
    p.galley(pos, g, c);
}

fn galley(ui: &Ui, text: &str, f: FontId, c: Color32) -> std::sync::Arc<Galley> {
    ui.painter().layout_no_wrap(text.to_string(), f, c)
}

pub struct Ctx<'a> {
    pub screen: &'a Screen,
    pub states: &'a mut HashMap<String, WState>,
    pub mode: Mode,
    pub selected: Option<&'a str>,
    pub dragging: Option<&'a str>,
    pub out: &'a mut PhoneOut,
    pub time: f64,
    /// The project's pictures, by name.
    pub files: &'a HashMap<String, std::path::PathBuf>,
}

/// A project picture as a texture: loaded once, and again when the file changes.
/// A video's length (ms), read once per file change.
pub fn video_len(ctx: &Context, path: &std::path::Path) -> Option<f64> {
    let when = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let key = Id::new(("video-len", path, when));
    if let Some(v) = ctx.data(|d| d.get_temp::<Option<f64>>(key)) {
        return v;
    }
    let v = crate::model::video_duration_ms(path);
    ctx.data_mut(|d| d.insert_temp(key, v));
    v
}

/// A file on this computer named by its whole path ("set picture from").
fn outside_file(name: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(name.strip_prefix("file://").unwrap_or(name));
    (p.is_absolute() && p.is_file()).then(|| p.to_path_buf())
}

pub fn picture(ctx: &Context, path: &std::path::Path) -> Option<TextureHandle> {
    let when = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let key = Id::new(("picture", path, when));
    if let Some(t) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(key)) {
        return t;
    }
    let t = image::open(path).ok().map(|img| {
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        ctx.load_texture(format!("picture {}", path.display()), ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(key, t.clone()));
    t
}

/// Draw the whole screen into a content rect.
pub fn draw_screen(ui: &mut Ui, content: Rect, cx: &mut Ctx) {
    let mut child = ui.new_child(UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)));
    child.set_clip_rect(content);
    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let body = |ui: &mut Ui, cx: &mut Ctx| {
        ui.set_min_size(content.size());
        let kids = draw_children(ui, "root", false, 1, cx);
        cx.out.containers.push(ContainerHit { id: "root".into(), rect: content, horizontal: false, children: kids });
    };
    if cx.mode == Mode::Run {
        ScrollArea::vertical().id_salt("phone_scroll").auto_shrink([false, false]).show(&mut child, |ui| body(ui, cx));
    } else {
        // designing: the wheel scrolls a screen taller than the phone; a drag
        // moves widgets, not the screen (the designer scrolls while a widget
        // is dragged near the top or bottom, see DESIGN_SCROLL_BY)
        ScrollArea::vertical().id_salt("design_scroll").drag_to_scroll(false).auto_shrink([false, false]).show(&mut child, |ui| {
            if let Some(dy) = ui.ctx().data_mut(|d| d.remove_temp::<f32>(Id::new(DESIGN_SCROLL_BY))) {
                // straight away: an animation restarted every frame barely moves
                ui.scroll_with_delta_animation(vec2(0.0, dy), egui::style::ScrollAnimation::none());
            }
            body(ui, cx)
        });
    }
}

/// How far the designer wants the design phone's screen scrolled this frame
/// (points; positive shows what's above).
pub const DESIGN_SCROLL_BY: &str = "design_scroll_by";

fn draw_children(ui: &mut Ui, parent: &str, horizontal: bool, depth: usize, cx: &mut Ctx) -> Vec<(String, Rect)> {
    let idx = cx.screen.children(parent);
    let mut kids = vec![];
    let screen = cx.screen;
    let mut inner = |ui: &mut Ui, cx: &mut Ctx| {
        for i in idx.iter() {
            let w = &screen.widgets[*i];
            if let Some(r) = draw_widget(ui, w, depth, horizontal, cx) {
                kids.push((w.id.clone(), r));
            }
        }
    };
    inner(ui, cx);
    kids
}

fn align_of(frac: f32) -> Align {
    if frac >= 1.0 {
        Align::Max
    } else if frac > 0.0 {
        Align::Center
    } else {
        Align::Min
    }
}

fn layout_for(w: &Widget) -> Layout {
    if w.kind.is_horizontal() {
        Layout::left_to_right(Align::Min)
    } else {
        Layout::top_down(align_of(w.gravity.frac()))
    }
}

/// Share a layout's spare room between its weighted children, like a phone's
/// LinearLayout: what the other children don't use (last frame's sizes) goes
/// to the weighted ones in proportion to their weights, along the layout's
/// direction. Without a known size (wrap_content) weights do nothing.
fn share_weights(ctx: &Context, screen: &Screen, w: &Widget, room: f32) {
    let row = w.kind.is_horizontal();
    let kids = screen.children(&w.id);
    let total: f32 = kids.iter().map(|i| screen.widgets[*i].weight.max(0.0)).sum();
    let along = |v: Vec2| if row { v.x } else { v.y };
    let used: f32 = kids
        .iter()
        .map(|i| &screen.widgets[*i])
        .filter(|c| c.weight <= 0.0)
        .map(|c| ctx.data(|d| d.get_temp::<Vec2>(Id::new(("phone_outer", &c.id)))).map_or(0.0, along))
        .sum();
    for i in kids {
        let c = &screen.widgets[i];
        let key = Id::new(("phone_weight", &c.id));
        let share = (total > 0.0 && room > 0.0 && c.weight > 0.0).then(|| (row, ((room - used).max(0.0) * c.weight / total).floor()));
        ctx.data_mut(|d| match share {
            Some(v) => d.insert_temp(key, v),
            None => d.remove::<(bool, f32)>(key),
        });
    }
}

/// Lays a layout's children out with its gravity, like a phone's
/// LinearLayout: each child sits across the layout on its own (horizontal
/// gravity in a column, vertical gravity in a row), and the children as a
/// group move along it when the layout has room to spare. The group's size is
/// only known after drawing, so last frame's size places it.
fn gravity_children(ui: &mut Ui, w: &Widget, depth: usize, cx: &mut Ctx) -> Vec<(String, Rect)> {
    let row = w.kind.is_horizontal();
    let (fx, fy) = (w.gravity.frac(), w.vgravity.frac());
    let fixed_w = matches!(w.width, Size::Match | Size::Px(_));
    let fixed_h = matches!(w.height, Size::Match | Size::Px(_));
    let finite = |v: f32| if v.is_finite() && v > 0.0 && v < 4000.0 { v } else { 0.0 };
    let room = vec2(if fixed_w { finite(ui.available_width()) } else { 0.0 }, if fixed_h { finite(ui.available_height()) } else { 0.0 });
    share_weights(ui.ctx(), cx.screen, w, if row { room.x } else { room.y });
    let key = Id::new(("phone_content", &w.id));
    let prev: Option<Vec2> = ui.ctx().data(|d| d.get_temp(key));
    let free = prev.map(|c| (room - c).max(Vec2::ZERO)).unwrap_or(Vec2::ZERO);
    // the group moves along the layout's own direction only
    let shift = if row { vec2((free.x * fx).floor(), 0.0) } else { vec2(0.0, (free.y * fy).floor()) };
    let at = ui.cursor().min + shift;
    let avail_w = (ui.available_width() - shift.x).max(1.0);
    let r = if row {
        // Each child gets its own cell as tall as the row, so a wrap_content
        // child stays its own height (a horizontal egui layout would stretch
        // it to the row) and vertical gravity places it in the cell.
        let h = if room.y > 0.0 { room.y } else { prev.map_or(0.0, |c| c.y) };
        let max = Rect::from_min_size(at, vec2(avail_w, if h > 0.0 { h } else { ui.available_height() }.max(1.0)));
        ui.allocate_new_ui(UiBuilder::new().max_rect(max).layout(Layout::left_to_right(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let screen = cx.screen;
            let mut kids = vec![];
            for i in screen.children(&w.id) {
                let c = &screen.widgets[i];
                let ckey = Id::new(("phone_cell", &c.id));
                let ch: Option<f32> = ui.ctx().data(|d| d.get_temp(ckey));
                let dy = if h > 0.0 { ch.map_or(0.0, |ch| ((h - ch).max(0.0) * fy).floor()) } else { 0.0 };
                let cell_h = if h > 0.0 { h } else { ui.available_height() };
                let cell = ui.allocate_ui_with_layout(vec2(ui.available_width(), cell_h), Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    ui.add_space(dy);
                    draw_widget(ui, c, depth + 1, true, cx)
                });
                let own = cell.response.rect.height() - dy;
                if ch.map_or(true, |p| (p - own).abs() > 0.5) {
                    ui.ctx().data_mut(|d| d.insert_temp(ckey, own));
                    ui.ctx().request_repaint();
                }
                if let Some(rr) = cell.inner {
                    kids.push((c.id.clone(), rr));
                }
            }
            let mut b = Rect::NOTHING;
            for (_, k) in &kids {
                b = b.union(*k);
            }
            (kids, if b.is_positive() { b.size() } else { Vec2::ZERO })
        })
    } else {
        let max = Rect::from_min_size(at, vec2(avail_w, (ui.available_height() - shift.y).max(1.0)));
        ui.allocate_new_ui(UiBuilder::new().max_rect(max).layout(layout_for(w)), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let kids = draw_children(ui, &w.id, false, depth + 1, cx);
            // measured from the children themselves: a column's own width is the
            // layout's, which isn't the group's
            let mut b = Rect::NOTHING;
            for (_, k) in &kids {
                b = b.union(*k);
            }
            (kids, if b.is_positive() { b.size() } else { Vec2::ZERO })
        })
    };
    let (kids, size) = r.inner;
    if prev.map_or(true, |p| (p - size).length() > 0.5) {
        ui.ctx().data_mut(|d| d.insert_temp(key, size));
        ui.ctx().request_repaint();
    }
    kids
}

/// Is `w` inside a scrolling column (`across` false) or row (true), with
/// nothing of a fixed size or a fixed share (weight) between them? Then its
/// height (width) is unbounded: match_parent can only mean its content.
fn in_scroll(screen: &Screen, w: &Widget, across: bool) -> bool {
    let mut parent = w.parent.as_str();
    for _ in 0..64 {
        let Some(p) = screen.widget(parent) else { return false };
        if p.kind == if across { WKind::ScrollH } else { WKind::ScrollV } {
            return true;
        }
        let size = if across { p.width } else { p.height };
        if matches!(size, Size::Px(_)) || p.weight > 0.0 {
            return false;
        }
        parent = p.parent.as_str();
    }
    false
}

fn draw_widget(ui: &mut Ui, w: &Widget, depth: usize, _in_horizontal: bool, cx: &mut Ctx) -> Option<Rect> {
    // a weighted child gets its share of the parent as a fixed size
    let shared: Option<(bool, f32)> = ui.ctx().data(|d| d.get_temp(Id::new(("phone_weight", &w.id))));
    let sized;
    let w = match shared {
        Some((row, main)) => {
            let mut c = w.clone();
            let px = Size::Px((main - 2.0 * w.margin.max(0) as f32).max(1.0).round() as i32);
            if row {
                c.width = px;
            } else {
                c.height = px;
            }
            sized = c;
            &sized
        }
        None => w,
    };
    // inside a scrolling column (row) there is no height (width) to fill: a
    // match_parent widget is as tall (wide) as its content, as on a phone
    let unbounded;
    let (free_h, free_w) = (matches!(w.height, Size::Match) && in_scroll(cx.screen, w, false), matches!(w.width, Size::Match) && in_scroll(cx.screen, w, true));
    let w = if free_h || free_w {
        let mut c = w.clone();
        if free_h {
            c.height = Size::Wrap;
        }
        if free_w {
            c.width = Size::Wrap;
        }
        unbounded = c;
        &unbounded
    } else {
        w
    };
    let mut st = cx.states.get(&w.id).cloned().unwrap_or_else(|| crate::runtime::initial_state(w));
    if st.visibility == 2 && cx.mode == Mode::Run {
        return None;
    }
    let faded = st.visibility != 0 && cx.mode == Mode::Design;
    let alpha = if faded { 0.35 } else { st.alpha };
    let invisible = st.visibility == 1 && cx.mode == Mode::Run;
    let pad = w.padding.max(0) as f32;
    let margin = w.margin.max(0) as f32;
    let enabled = st.enabled;
    let mode = cx.mode;
    let id = w.id.clone();

    let mut frame = Frame::none().inner_margin(Margin::same(pad)).outer_margin(Margin::same(margin)).fill(if invisible { Color32::TRANSPARENT } else { c32(st.bg).gamma_multiply(alpha) }).rounding(st.radius);
    if !invisible {
        if st.border > 0.0 {
            frame = frame.stroke(Stroke::new(st.border, c32(st.border_color).gamma_multiply(alpha)));
        }
        // a shadow needs something to cast it, as on a phone
        if st.elevation > 0.0 && ((st.bg >> 24) != 0 || w.kind == WKind::Button) {
            let e = st.elevation;
            frame = frame.shadow(egui::epaint::Shadow { offset: vec2(0.0, e * 0.5), blur: e * 1.5, spread: 0.0, color: Color32::from_black_alpha((40.0 + e * 4.0).min(110.0) as u8).gamma_multiply(alpha) });
        }
    }
    // A layout that is narrower than its column (wrap or fixed width) follows
    // the column's gravity, like on a phone. Its width is only known after
    // drawing, so last frame's width places it.
    let parent = *ui.layout();
    let follows_gravity = w.kind.is_container() && parent.is_vertical() && parent.horizontal_align() != Align::Min && !matches!(w.width, Size::Match);
    let width_id = Id::new(("phone_width", &w.id));
    let shift = if follows_gravity {
        let prev: Option<f32> = ui.ctx().data(|d| d.get_temp(width_id));
        let free = prev.map(|pw| (ui.available_width() - pw).max(0.0)).unwrap_or(0.0);
        Some(if parent.horizontal_align() == Align::Center { (free / 2.0).floor() } else { free })
    } else {
        None
    };
    let body = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let avail = ui.available_width();
        match w.width {
            Size::Match => ui.set_min_width(avail.max(0.0)),
            Size::Px(px) => {
                let inner = (px as f32 - 2.0 * pad).max(1.0);
                ui.set_min_width(inner);
                ui.set_max_width(inner);
            }
            Size::Wrap => {}
        }
        match w.height {
            Size::Px(px) => {
                let inner = (px as f32 - 2.0 * pad).max(1.0);
                ui.set_min_height(inner);
                ui.set_max_height(inner);
            }
            Size::Match => {
                // Fill what's left of the parent's height (match_parent).
                let h = ui.available_height();
                if h.is_finite() && h > 0.0 && h < 4000.0 {
                    ui.set_min_height(h);
                }
            }
            Size::Wrap => {}
        }
        if w.kind.is_container() {
            if mode == Mode::Design && cx.screen.children(&w.id).is_empty() {
                ui.set_min_size(vec2(ui.min_size().x.max(if w.kind.is_horizontal() { 120.0 } else { 80.0 }), 36.0));
            }
            let lay = layout_for(w);
            let r = ui.with_layout(lay, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if matches!(w.width, Size::Match | Size::Px(_)) {
                    ui.set_min_width(ui.available_width());
                }
                if mode == Mode::Run && matches!(w.kind, WKind::ScrollH | WKind::ScrollV) {
                    let mut kids = vec![];
                    if w.kind == WKind::ScrollH {
                        ScrollArea::horizontal().id_salt(("hs", &w.id)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                kids = draw_children(ui, &w.id, true, depth + 1, cx);
                            });
                        });
                    } else {
                        // it scrolls within its own height: all of it when it
                        // fills (match_parent) or is fixed, else what's left
                        let room = ui.available_height();
                        let room = if room.is_finite() && room > 40.0 { room } else { 400.0 };
                        let fills = matches!(w.height, Size::Match | Size::Px(_));
                        ScrollArea::vertical().id_salt(("vs", &w.id)).max_height(room).auto_shrink([false, !fills]).show(ui, |ui| {
                            kids = draw_children(ui, &w.id, false, depth + 1, cx);
                        });
                    }
                    kids
                } else {
                    gravity_children(ui, w, depth, cx)
                }
            });
            let kids = r.inner;
            let crect = ui.min_rect();
            cx.out.containers.push(ContainerHit { id: w.id.clone(), rect: crect, horizontal: w.kind.is_horizontal(), children: kids });
        } else {
            draw_leaf(ui, w, &mut st, alpha, invisible, enabled, cx);
        }
    };
    let resp = match shift {
        Some(dx) => {
            let at = ui.cursor().min + vec2(dx, 0.0);
            let max = Rect::from_min_size(at, vec2((ui.available_width() - dx).max(1.0), ui.available_height()));
            ui.allocate_new_ui(UiBuilder::new().max_rect(max).layout(Layout::top_down(Align::Min)), |ui| frame.show(ui, body)).inner
        }
        None => frame.show(ui, body),
    };
    if follows_gravity {
        let wnow = resp.response.rect.width();
        let prev: Option<f32> = ui.ctx().data(|d| d.get_temp(width_id));
        if prev.map_or(true, |p| (p - wnow).abs() > 0.5) {
            ui.ctx().data_mut(|d| d.insert_temp(width_id, wnow));
            ui.ctx().request_repaint();
        }
    }
    let rect = resp.response.rect;
    // remembered for the parent's weights next frame
    let outer_key = Id::new(("phone_outer", &w.id));
    if ui.ctx().data(|d| d.get_temp::<Vec2>(outer_key)).map_or(true, |p| (p - rect.size()).length() > 0.5) {
        ui.ctx().data_mut(|d| d.insert_temp(outer_key, rect.size()));
        ui.ctx().request_repaint();
    }
    let inner_rect = rect.shrink(margin);
    if mode == Mode::Design {
        let p = ui.painter();
        if w.kind.is_container() {
            p.rect_stroke(inner_rect, 0.0, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0x5B, 0x5B, 0xD6, 80)));
            if cx.screen.children(&w.id).is_empty() {
                p.text(inner_rect.center(), Align2::CENTER_CENTER, w.kind.palette_name(), FontId::proportional(10.0), Color32::from_gray(170));
            }
        }
        if cx.selected == Some(id.as_str()) {
            p.rect_stroke(inner_rect.expand(1.0), 0.0, Stroke::new(2.0_f32, PRIMARY));
        }
        if cx.dragging == Some(id.as_str()) {
            p.rect_filled(inner_rect, 0.0, Color32::from_rgba_unmultiplied(0x5B, 0x5B, 0xD6, 40));
        }
    }
    cx.out.rects.push((id.clone(), inner_rect, depth));
    cx.states.insert(id, st);
    Some(inner_rect)
}

fn text_color(st: &WState, alpha: f32, enabled: bool) -> Color32 {
    let c = c32(st.text_color);
    let c = if enabled { c } else { c.gamma_multiply(0.45) };
    c.gamma_multiply(alpha)
}

/// Size for a leaf widget given its content size.
fn leaf_size(ui: &Ui, w: &Widget, content: Vec2) -> Vec2 {
    let x = match w.width {
        Size::Wrap => content.x,
        _ => ui.available_width().max(content.x.min(ui.available_width())),
    };
    let y = match w.height {
        Size::Px(_) => ui.available_height().max(content.y).min(content.y.max(ui.min_size().y)),
        Size::Match => {
            let h = ui.available_height();
            if h.is_finite() && h < 4000.0 { h.max(content.y) } else { content.y }
        }
        _ => content.y,
    };
    let y = if let Size::Px(_) = w.height { ui.min_size().y.max(1.0) } else { y };
    vec2(x.max(1.0), y.max(1.0))
}

/// Where content of `size` sits in `rect` for gravity fractions (0 start,
/// 0.5 middle, 1 end).
fn aligned(rect: Rect, size: Vec2, fx: f32, fy: f32) -> Pos2 {
    pos2(rect.left() + (rect.width() - size.x) * fx, rect.top() + (rect.height() - size.y) * fy)
}

fn draw_leaf(ui: &mut Ui, w: &Widget, st: &mut WState, alpha: f32, invisible: bool, enabled: bool, cx: &mut Ctx) {
    let run = cx.mode == Mode::Run;
    let live = run && enabled && !invisible;
    let tc = text_color(st, alpha, enabled);
    let f = font(ui, st.text_size, &st.font);
    let fire = |cx: &mut Ctx, ev: &str, locals: HashMap<String, Val>| {
        if cx.screen.events.iter().any(|e| e.target == w.id && e.name == ev) {
            cx.out.events.push((w.id.clone(), ev.to_string(), locals));
        }
    };
    match w.kind {
        WKind::Button | WKind::TextView => {
            let g = galley(ui, &st.text, f.clone(), tc);
            let min = if w.kind == WKind::Button { vec2(64.0, 36.0) } else { vec2(0.0, 0.0) };
            let size = leaf_size(ui, w, text_size(&g, w.bold).max(min));
            let (rect, resp) = ui.allocate_exact_size(size, if live { Sense::click() } else { Sense::hover() });
            if invisible {
                return;
            }
            let p = ui.painter();
            if w.kind == WKind::Button && (st.bg >> 24) == 0 {
                let fill = if resp.is_pointer_button_down_on() { Color32::from_gray(200) } else if resp.hovered() && live { Color32::from_gray(222) } else { Color32::from_gray(230) };
                p.rect_filled(rect, st.radius.max(2.0), fill.gamma_multiply(alpha));
            }
            // a button's text sits in the middle unless a gravity is picked
            let (fx, fy) = if w.kind == WKind::Button && w.gravity == Gravity::None && w.vgravity == VGravity::None { (0.5, 0.5) } else { (w.gravity.frac(), w.vgravity.frac()) };
            paint_text(p, aligned(rect, text_size(&g, w.bold), fx, fy), g, tc, w.bold);
            if live {
                if resp.clicked() {
                    fire(cx, "onClick", HashMap::new());
                }
                if resp.long_touched() || resp.secondary_clicked() {
                    fire(cx, "onLongClick", HashMap::new());
                }
            }
        }
        WKind::EditText => {
            let input = w.input_kind();
            let secret = input == InputKind::Password && !st.reveal;
            // a password field has its eye at the end
            let eye_w = if input == InputKind::Password { 26.0 } else { 0.0 };
            let shown = if st.text.is_empty() { st.hint.clone() } else if secret { "•".repeat(st.text.chars().count()) } else { st.text.clone() };
            let g = galley(ui, &shown, f.clone(), tc);
            let size = leaf_size(ui, w, vec2(text_size(&g, w.bold).x.max(80.0) + eye_w, g.size().y + 8.0));
            let eye = |ui: &Ui, rect: Rect, open: bool, alpha: f32| {
                let c = Color32::from_gray(120).gamma_multiply(alpha);
                ui.painter().text(pos2(rect.right() - eye_w / 2.0, rect.center().y - 1.0), Align2::CENTER_CENTER, if open { crate::icons::EYE_OFF } else { crate::icons::EYE }, FontId::proportional(15.0), c);
            };
            if run && !invisible {
                let before = st.text.clone();
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let field = Rect::from_min_max(rect.min, pos2(rect.right() - eye_w, rect.bottom()));
                let mut child = ui.new_child(UiBuilder::new().max_rect(field));
                let hint = RichText::new(st.hint.clone()).color(c32(w.hint_color));
                let te = if input == InputKind::Multiline { TextEdit::multiline(&mut st.text).desired_rows(1) } else { TextEdit::singleline(&mut st.text) };
                let te = te.frame(false).font(f).text_color(tc).hint_text(hint).desired_width(field.width()).interactive(enabled).password(secret);
                child.add(te);
                // the keyboard of a number or phone field has no letters
                if st.text != before && st.text.chars().any(|c| !input.allows(c)) {
                    st.text = st.text.chars().filter(|c| input.allows(*c)).collect();
                }
                ui.painter().line_segment([rect.left_bottom() - vec2(0.0, 2.0), rect.right_bottom() - vec2(0.0, 2.0)], Stroke::new(1.5_f32, PRIMARY));
                if eye_w > 0.0 {
                    let er = Rect::from_center_size(pos2(rect.right() - eye_w / 2.0, rect.center().y), vec2(eye_w, rect.height().min(28.0)));
                    let r = ui.interact(er, Id::new(("password_eye", &w.id)), Sense::click());
                    if r.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if r.clicked() && enabled {
                        st.reveal = !st.reveal;
                    }
                    eye(ui, rect, st.reveal, 1.0);
                }
                if st.text != before {
                    let mut l = HashMap::new();
                    l.insert("_charSeq".into(), Val::Str(st.text.clone()));
                    fire(cx, "onTextChanged", l);
                }
            } else {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                if invisible {
                    return;
                }
                let col = if st.text.is_empty() { c32(w.hint_color).gamma_multiply(alpha) } else { tc };
                let g = galley(ui, &shown, f, col);
                paint_text(ui.painter(), rect.left_top() + vec2(0.0, 2.0), g, col, w.bold);
                ui.painter().line_segment([rect.left_bottom() - vec2(0.0, 2.0), rect.right_bottom() - vec2(0.0, 2.0)], Stroke::new(1.0_f32, Color32::from_gray(150)));
                if eye_w > 0.0 {
                    eye(ui, rect, false, alpha);
                }
            }
        }
        WKind::CheckBox | WKind::Switch => {
            let g = galley(ui, &st.text, f, tc);
            let box_w = if w.kind == WKind::Switch { 40.0 } else { 22.0 };
            let size = leaf_size(ui, w, vec2(text_size(&g, w.bold).x + box_w + 8.0, g.size().y.max(24.0)));
            let (rect, resp) = ui.allocate_exact_size(size, if live { Sense::click() } else { Sense::hover() });
            if invisible {
                return;
            }
            if live && resp.clicked() {
                st.checked = !st.checked;
                let mut l = HashMap::new();
                l.insert("_isChecked".into(), Val::Bool(st.checked));
                fire(cx, "onCheckedChange", l);
            }
            let p = ui.painter();
            let on = PRIMARY.gamma_multiply(alpha);
            if w.kind == WKind::CheckBox {
                let b = Rect::from_min_size(pos2(rect.left() + 2.0, rect.center().y - 8.0), vec2(16.0, 16.0));
                if st.checked {
                    p.rect_filled(b, 2.0, on);
                    p.line_segment([b.left_center() + vec2(3.0, 0.0), b.center_bottom() - vec2(1.0, 4.0)], Stroke::new(2.0_f32, Color32::WHITE));
                    p.line_segment([b.center_bottom() - vec2(1.0, 4.0), b.right_top() + vec2(-3.0, 4.0)], Stroke::new(2.0_f32, Color32::WHITE));
                } else {
                    p.rect_stroke(b, 2.0, Stroke::new(2.0_f32, Color32::from_gray(110).gamma_multiply(alpha)));
                }
                paint_text(p, pos2(b.right() + 8.0, rect.center().y - g.size().y / 2.0), g, tc, w.bold);
            } else {
                paint_text(p, pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, tc, w.bold);
                let track = Rect::from_min_size(pos2(rect.right() - 36.0, rect.center().y - 7.0), vec2(34.0, 14.0));
                p.rect_filled(track, 7.0, if st.checked { on.gamma_multiply(0.5) } else { Color32::from_gray(180).gamma_multiply(alpha) });
                let cxp = if st.checked { track.right() - 7.0 } else { track.left() + 7.0 };
                p.circle_filled(pos2(cxp, track.center().y), 9.0, if st.checked { on } else { Color32::from_gray(245).gamma_multiply(alpha) });
                p.circle_stroke(pos2(cxp, track.center().y), 9.0, Stroke::new(0.5_f32, Color32::from_gray(150)));
            }
        }
        WKind::ImageView => {
            let tex = cx.files.get(&st.image).cloned().or_else(|| outside_file(&st.image)).and_then(|p| picture(ui.ctx(), &p));
            // like Android's adjustViewBounds: a picture keeps its shape, and
            // its own size is its pixels (1 pixel, 1 dp)
            let content = match &tex {
                Some(t) => {
                    let n = t.size_vec2();
                    let avail = ui.available_width();
                    let wd = if w.width == Size::Wrap { n.x.min(avail.max(1.0)) } else { avail };
                    vec2(wd, if w.crop && w.height != Size::Wrap { n.y } else { wd * n.y / n.x.max(1.0) })
                }
                None if run => vec2(0.0, 0.0),
                None => vec2(100.0, 100.0),
            };
            let size = leaf_size(ui, w, content);
            let (rect, resp) = ui.allocate_exact_size(size, if live { Sense::click() } else { Sense::hover() });
            if invisible {
                return;
            }
            let p = ui.painter();
            if let Some(t) = &tex {
                let n = t.size_vec2();
                let (r, uv) = if w.crop {
                    // fill the box and cut off what sticks out
                    let s = (rect.width() / n.x).max(rect.height() / n.y);
                    let (u, v) = (rect.width() / (n.x * s), rect.height() / (n.y * s));
                    (rect, Rect::from_center_size(pos2(0.5, 0.5), vec2(u, v)))
                } else {
                    let s = (rect.width() / n.x).min(rect.height() / n.y);
                    (Rect::from_center_size(rect.center(), n * s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)))
                };
                let mut shape = epaint::RectShape::filled(r, st.radius, Color32::WHITE.gamma_multiply(alpha));
                shape.fill_texture_id = t.id();
                shape.uv = uv;
                p.add(shape);
                if live && resp.clicked() {
                    fire(cx, "onClick", HashMap::new());
                }
                return;
            }
            if run {
                return;
            }
            p.rect_filled(rect, 0.0, Color32::from_rgb(0xDD, 0xE6, 0xF0).gamma_multiply(alpha));
            let c = rect.center();
            let s = rect.width().min(rect.height()) * 0.3;
            p.add(Shape::convex_polygon(vec![c + vec2(-s, s * 0.6), c + vec2(-s * 0.2, -s * 0.4), c + vec2(s * 0.5, s * 0.6)], Color32::from_rgb(0x90, 0xA4, 0xAE).gamma_multiply(alpha), Stroke::NONE));
            p.circle_filled(c + vec2(s * 0.5, -s * 0.4), s * 0.18, Color32::from_rgb(0x90, 0xA4, 0xAE).gamma_multiply(alpha));
            if live && resp.clicked() {
                fire(cx, "onClick", HashMap::new());
            }
        }
        WKind::VideoView => {
            let path = cx.files.get(&st.video).cloned().or_else(|| outside_file(&st.video));
            let path = path.as_ref();
            let known = path.is_some() || (run && (st.video.starts_with("http://") || st.video.starts_with("https://")));
            let size = leaf_size(ui, w, if known || !run { vec2(240.0, 135.0) } else { vec2(0.0, 0.0) });
            let (rect, resp) = ui.allocate_exact_size(size, if live { Sense::click() } else { Sense::hover() });
            // no video: nothing on a phone, a placeholder while designing
            if invisible || (run && !known) {
                return;
            }
            let p = ui.painter();
            p.rect_filled(rect, st.radius, Color32::from_rgb(0x11, 0x13, 0x18).gamma_multiply(alpha));
            // running: the real video's picture (player::Preview), fitted in, as VideoView does
            let frame = if run { ui.ctx().data(|d| d.get_temp::<TextureHandle>(crate::player::frame_id(&w.id))) } else { None };
            if let Some(t) = &frame {
                let n = t.size_vec2();
                let k = (rect.width() / n.x).min(rect.height() / n.y);
                let img = Rect::from_center_size(rect.center(), n * k);
                egui::Image::new((t.id(), img.size())).rounding(st.radius).tint(Color32::WHITE.gamma_multiply(alpha)).paint_at(ui, img);
            }
            let p = ui.painter();
            let c = rect.center();
            let r = (rect.width().min(rect.height()) * 0.16).clamp(10.0, 30.0);
            // with a picture, the play sign shows only while paused
            if (frame.is_none() && (w.controls || !run || !st.playing)) || (frame.is_some() && !st.playing) {
                p.circle_filled(c, r, Color32::from_white_alpha(40).gamma_multiply(alpha));
                p.text(c, Align2::CENTER_CENTER, if st.playing { crate::icons::PAUSE } else { crate::icons::PLAY }, FontId::proportional(r), Color32::WHITE.gamma_multiply(alpha));
            }
            let len = path.and_then(|p| video_len(ui.ctx(), p));
            let label = match (path.is_some(), len) {
                _ if frame.is_some() && st.playing => String::new(),
                (false, _) => "Video: pick one in Video".to_string(),
                (true, Some(ms)) if run => format!("{}  {} / {}", if st.playing { "playing" } else { "paused" }, crate::model::clock(st.video_pos), crate::model::clock(ms)),
                (true, Some(ms)) => format!("{}  ·  {}", st.video, crate::model::clock(ms)),
                (true, None) => st.video.clone(),
            };
            p.text(rect.left_bottom() + vec2(8.0, -6.0), Align2::LEFT_BOTTOM, label, FontId::proportional(11.0), Color32::from_gray(210).gamma_multiply(alpha));
            // tapping plays and pauses, as the controls do
            if live && w.controls && known && resp.clicked() {
                st.playing = !st.playing;
            }
        }
        WKind::WebView => {
            let size = leaf_size(ui, w, vec2(200.0, 120.0));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            if invisible {
                return;
            }
            let p = ui.painter();
            p.rect_filled(rect, 0.0, Color32::from_gray(245));
            p.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::from_gray(200)));
            let t = if st.url.is_empty() { "WebView".to_string() } else { format!("Web page:\n{}", st.url) };
            p.text(rect.center(), Align2::CENTER_CENTER, t, FontId::proportional(11.0), Color32::from_gray(120));
        }
        WKind::ProgressBar => {
            let size = leaf_size(ui, w, vec2(40.0, 40.0));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            if invisible {
                return;
            }
            let c = rect.center();
            let r = 15.0;
            let start = if run { cx.time as f32 * 5.0 } else { 0.0 };
            let pts: Vec<Pos2> = (0..=24).map(|i| {
                let a = start + i as f32 / 24.0 * 4.4;
                c + vec2(a.cos(), a.sin()) * r
            }).collect();
            ui.painter().add(Shape::line(pts, Stroke::new(3.5_f32, PRIMARY.gamma_multiply(alpha))));
            if run {
                ui.ctx().request_repaint();
            }
        }
        WKind::ListView => {
            let items: Vec<String> = if run { st.items.clone() } else { (1..=3).map(|i| format!("List item {i}")).collect() };
            let row_h = 40.0;
            let size = leaf_size(ui, w, vec2(160.0, (items.len().max(1) as f32) * row_h));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            if invisible {
                return;
            }
            for (i, it) in items.iter().enumerate() {
                let r = Rect::from_min_size(rect.min + vec2(0.0, i as f32 * row_h), vec2(rect.width(), row_h));
                let resp = ui.interact(r, ui.id().with(("row", &w.id, i)), if live { Sense::click() } else { Sense::hover() });
                if resp.hovered() && live {
                    ui.painter().rect_filled(r, 0.0, Color32::from_gray(238));
                }
                ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, it, FontId::proportional(15.0), Color32::from_gray(if run { 30 } else { 150 }));
                ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0_f32, Color32::from_gray(225)));
                if live && resp.clicked() {
                    let mut l = HashMap::new();
                    l.insert("_position".into(), Val::Num(i as f64));
                    fire(cx, "onItemClicked", l);
                }
            }
        }
        WKind::Spinner => {
            let cur = if run { st.items.get(st.selected).cloned().unwrap_or_default() } else { "Spinner".into() };
            let g = galley(ui, &cur, FontId::proportional(14.0), Color32::from_gray(40));
            let size = leaf_size(ui, w, vec2(g.size().x.max(60.0) + 30.0, 32.0));
            let (rect, resp) = ui.allocate_exact_size(size, if live { Sense::click() } else { Sense::hover() });
            if invisible {
                return;
            }
            let p = ui.painter();
            p.galley(pos2(rect.left() + 6.0, rect.center().y - g.size().y / 2.0), g, Color32::from_gray(40));
            let t = pos2(rect.right() - 14.0, rect.center().y + 3.0);
            p.add(Shape::convex_polygon(vec![t + vec2(-5.0, -6.0), t + vec2(5.0, -6.0), t], Color32::from_gray(90), Stroke::NONE));
            let popup = ui.id().with(("spin", &w.id));
            if live && resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(popup));
            }
            let mut picked = None;
            popup::popup_below_widget(ui, popup, &resp, PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(rect.width());
                for (i, it) in st.items.iter().enumerate() {
                    if ui.selectable_label(i == st.selected, it).clicked() {
                        picked = Some(i);
                    }
                }
            });
            if let Some(i) = picked {
                st.selected = i;
                let mut l = HashMap::new();
                l.insert("_position".into(), Val::Num(i as f64));
                fire(cx, "onItemSelected", l);
            }
        }
        WKind::SeekBar => {
            let size = leaf_size(ui, w, vec2(160.0, 28.0));
            let (rect, resp) = ui.allocate_exact_size(size, if live { Sense::click_and_drag() } else { Sense::hover() });
            if invisible {
                return;
            }
            let track = rect.shrink2(vec2(10.0, 0.0));
            let max = st.max.max(1) as f32;
            if live && (resp.dragged() || resp.clicked()) {
                if let Some(p) = resp.interact_pointer_pos() {
                    let v = (((p.x - track.left()) / track.width()).clamp(0.0, 1.0) * max).round() as i32;
                    if v != st.progress {
                        st.progress = v;
                        let mut l = HashMap::new();
                        l.insert("_progressValue".into(), Val::Num(v as f64));
                        fire(cx, "onProgressChanged", l);
                    }
                }
            }
            let frac = (st.progress as f32 / max).clamp(0.0, 1.0);
            let y = rect.center().y;
            let p = ui.painter();
            p.line_segment([pos2(track.left(), y), pos2(track.right(), y)], Stroke::new(2.0_f32, Color32::from_gray(190)));
            let x = track.left() + track.width() * frac;
            p.line_segment([pos2(track.left(), y), pos2(x, y)], Stroke::new(2.0_f32, PRIMARY.gamma_multiply(alpha)));
            p.circle_filled(pos2(x, y), 7.0, PRIMARY.gamma_multiply(alpha));
        }
        WKind::CalendarView => {
            let size = leaf_size(ui, w, vec2(250.0, 210.0));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            if invisible {
                return;
            }
            let p = ui.painter();
            p.text(rect.center_top() + vec2(0.0, 14.0), Align2::CENTER_CENTER, "‹   Month   ›", FontId::proportional(14.0), Color32::from_gray(60));
            let cw = rect.width() / 7.0;
            for (i, d) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
                p.text(pos2(rect.left() + cw * (i as f32 + 0.5), rect.top() + 40.0), Align2::CENTER_CENTER, *d, FontId::proportional(11.0), Color32::from_gray(130));
            }
            for day in 0..30 {
                let col = (day + 3) % 7;
                let row = (day + 3) / 7;
                let c = pos2(rect.left() + cw * (col as f32 + 0.5), rect.top() + 64.0 + row as f32 * 28.0);
                if day == 14 {
                    p.circle_filled(c, 11.0, PRIMARY);
                }
                p.text(c, Align2::CENTER_CENTER, format!("{}", day + 1), FontId::proportional(12.0), if day == 14 { Color32::WHITE } else { Color32::from_gray(50) });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    /// The home bar strip takes the colour of a full-height root layout.
    #[test]
    fn home_strip_matches_the_screen() {
        let mut p = crate::app::sample_project(1);
        let scr = &mut p.screens[0];
        let none = HashMap::new();
        assert_eq!(bottom_color(scr, &none), Color32::WHITE);
        let root = scr.children("root")[0];
        scr.widgets[root].height = Size::Match;
        scr.widgets[root].bg = 0xFF00_0000;
        assert_eq!(bottom_color(scr, &none), Color32::BLACK);
    }

    /// The screen mask stays inside the round corners of both phone pictures
    /// (a plain rounded rectangle used to poke past the iPhone's bottom corners).
    #[test]
    fn screen_masks_keep_inside_the_corners() {
        {
            let (w, h, px) = drawn_iphone();
            let hole = find_hole(w, h, &px);
            assert!(hole.width() > w as f32 * 0.8 && hole.height() > h as f32 * 0.8, "{hole:?}");
            // the screen is exactly where it was drawn: 1206×2622 in the middle
            assert_eq!((hole.width(), hole.height()), (1206.0, 2622.0), "{hole:?}");
            let m = screen_mask(w, &px, hole);
            let [mw, mh] = m.size;
            let at = |x: usize, y: usize| m.pixels[y * mw + x];
            assert_eq!(at(mw / 2, mh / 2), Color32::WHITE);
            for (x, y) in [(0, 0), (mw - 1, 0), (0, mh - 1), (mw - 1, mh - 1)] {
                assert_eq!(at(x, y), Color32::TRANSPARENT, "corner {x},{y}");
            }
        }
    }
}
