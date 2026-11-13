//! Lucide icons (https://lucide.dev, ISC licence, see assets/LUCIDE-LICENSE.txt).
//!
//! The Lucide icon font is added to egui's proportional font family, so an icon
//! is just a character: put one of the constants below in any label or button.
//! This file also draws the app icon of a project (projects page, project
//! dialog) and renders it to PNG files for the exported and built apps.

use crate::model::Project;
use egui::{Color32, Context, FontData, FontFamily, FontId, Painter, Rect, Rounding, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::sync::Mutex;

static LUCIDE_TTF: &[u8] = include_bytes!("../assets/lucide.ttf");

// ------------------------------------------------------------ UI icons

pub const ARROW_LEFT: &str = "\u{e04c}";
pub const PLAY: &str = "\u{e140}";
pub const HAMMER: &str = "\u{e0ef}";
pub const SMARTPHONE: &str = "\u{e167}";
pub const CODE: &str = "\u{e097}";
pub const SUN: &str = "\u{e17c}";
pub const MOON: &str = "\u{e122}";
pub const SETTINGS: &str = "\u{e158}";
pub const FOLDER: &str = "\u{e0dc}";
pub const FOLDER_OPEN: &str = "\u{e247}";
pub const PLUS: &str = "\u{e141}";
pub const SEARCH: &str = "\u{e155}";
pub const GRID: &str = "\u{e105}";
pub const LIST: &str = "\u{e10c}";
pub const ELLIPSIS: &str = "\u{e0ba}";
pub const PACKAGE: &str = "\u{e12d}";
pub const APPLE: &str = "\u{e352}";
pub const FILE_OUTPUT: &str = "\u{e0cc}";
pub const TRASH: &str = "\u{e18e}";
pub const COPY: &str = "\u{e0a2}";
pub const PENCIL: &str = "\u{e1f9}";
pub const X: &str = "\u{e1b2}";
pub const BUG: &str = "\u{e20c}";
pub const CLIPBOARD_LIST: &str = "\u{e08a}";
pub const ROTATE_CCW: &str = "\u{e14c}";
pub const STOP: &str = "\u{e16b}";
pub const REFRESH: &str = "\u{e149}";
pub const CIRCLE_CHECK: &str = "\u{e226}";
pub const CIRCLE_X: &str = "\u{e088}";
pub const CHECK: &str = "\u{e070}";
pub const ARCHIVE: &str = "\u{e045}";
pub const TIMER: &str = "\u{e1e0}";
pub const MESSAGE: &str = "\u{e11b}";
pub const VIBRATE: &str = "\u{e223}";
pub const CALENDAR: &str = "\u{e067}";
pub const ARROW_LEFT_RIGHT: &str = "\u{e24a}";
pub const DATABASE: &str = "\u{e0b1}";
pub const IMAGE: &str = "\u{e0f9}";
pub const CHEVRON_DOWN: &str = "\u{e071}";
pub const CIRCLE_PAUSE: &str = "\u{e084}";
pub const TYPE: &str = "\u{e198}";
pub const TEXT_INPUT: &str = "\u{e265}";
pub const GLOBE: &str = "\u{e0eb}";
pub const MUSIC: &str = "\u{e126}";
pub const IMAGE_PLUS: &str = "\u{e1f7}";
pub const LOADER: &str = "\u{e10e}";
pub const SQUARE_CHECK: &str = "\u{e55e}";
pub const TOGGLE: &str = "\u{e18b}";
pub const SLIDERS: &str = "\u{e29a}";
pub const CALENDAR_DAYS: &str = "\u{e2b9}";
pub const ROWS: &str = "\u{e58f}";
pub const COLUMNS: &str = "\u{e09d}";
pub const MOVE_H: &str = "\u{e1c6}";
pub const MOVE_V: &str = "\u{e1c7}";
pub const POINTER_CLICK: &str = "\u{e124}";
pub const CHEVRONS_UP_DOWN: &str = "\u{e211}";
pub const FOLDER_PLUS: &str = "\u{e0de}";
pub const BLOCKS: &str = "\u{e4ff}";
pub const UPLOAD: &str = "\u{e19e}";
pub const TERMINAL: &str = "\u{e185}";
pub const CIRCLE_PLAY: &str = "\u{e085}";
pub const FILE_TEXT: &str = "\u{e0d0}";
pub const VIDEO: &str = "\u{e1a5}";
pub const FILM: &str = "\u{e0d4}";
pub const PAUSE: &str = "\u{e132}";
pub const CAMERA: &str = "\u{e068}";
pub const DOWNLOAD: &str = "\u{e0b6}";
pub const EYE: &str = "\u{e0be}";
pub const EYE_OFF: &str = "\u{e0bf}";
pub const FILE_PLUS: &str = "\u{e0cd}";
pub const HOUSE: &str = "\u{e0f8}";
pub const LAYOUT: &str = "\u{e475}";
pub const ZAP: &str = "\u{e1b4}";
pub const PUZZLE: &str = "\u{e29c}";
pub const KEYBOARD: &str = "\u{e284}";
pub const COPY_PLUS: &str = "\u{e402}";
pub const PASTE: &str = "\u{e3ec}";
pub const SCISSORS: &str = "\u{e152}";
pub const UNDO: &str = "\u{e2a1}";
pub const REDO: &str = "\u{e2a0}";

/// "icon  text" for a button or menu item.
pub fn lbl(icon: &str, text: &str) -> String {
    format!("{icon}  {text}")
}

/// Adds the Lucide font after egui's text font, so its characters show in
/// every label. It comes before egui's own emoji fonts, so nothing they hold
/// can take the place of a Lucide icon.
pub fn install(ctx: &Context) {
    install_with(ctx, vec![]);
}

/// `install`, plus fonts of the open project: each one its own family
/// (named after the file), falling back to the usual letters and icons.
pub fn install_with(ctx: &Context, extra: Vec<(String, Vec<u8>)>) {
    let mut fonts = egui::FontDefinitions::default();
    let mut data = FontData::from_static(LUCIDE_TTF);
    // Lucide draws its icons on a square that sits a little high next to
    // text; nudge them down and make them a touch larger than the letters
    data.tweak.scale = 1.08;
    data.tweak.y_offset_factor = 0.10;
    fonts.font_data.insert("lucide".into(), data);
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.entry(fam).or_default();
        let at = 1.min(list.len());
        list.insert(at, "lucide".into());
    }
    // the shortcut symbols ⇧ ↑ ↓ live in Hack only; let plain text use them too
    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.push("Hack".into());
    }
    let base = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for (name, bytes) in extra {
        let key = format!("project:{name}");
        fonts.font_data.insert(key.clone(), FontData::from_owned(bytes));
        let mut list = vec![key];
        list.extend(base.iter().cloned());
        fonts.families.insert(FontFamily::Name(format!("project:{name}").into()), list);
    }
    ctx.set_fonts(fonts);
}

// ------------------------------------------------------------ app icons

/// The pictures a project can use as its app icon: (saved name, character).
pub const APP_GLYPHS: &[(&str, &str)] = &[
    ("star", "\u{e17a}"),
    ("heart", "\u{e0f5}"),
    ("zap", "\u{e1b4}"),
    ("rocket", "\u{e286}"),
    ("music", "\u{e126}"),
    ("camera", "\u{e068}"),
    ("shopping-cart", "\u{e160}"),
    ("gamepad-2", "\u{e0e2}"),
    ("book-open", "\u{e063}"),
    ("map-pin", "\u{e115}"),
    ("cloud", "\u{e08c}"),
    ("bell", "\u{e05d}"),
    ("user", "\u{e19f}"),
    ("graduation-cap", "\u{e234}"),
    ("dumbbell", "\u{e3a5}"),
    ("utensils", "\u{e2f6}"),
    ("plane", "\u{e1de}"),
    ("car", "\u{e1d5}"),
    ("leaf", "\u{e2de}"),
    ("coffee", "\u{e09a}"),
    ("brain", "\u{e3ca}"),
    ("calculator", "\u{e1bc}"),
    ("wallet", "\u{e204}"),
    ("shield", "\u{e15c}"),
    ("lock", "\u{e10f}"),
    ("gift", "\u{e0e4}"),
    ("flame", "\u{e0d7}"),
    ("trophy", "\u{e377}"),
    ("dice-5", "\u{e28b}"),
    ("sparkles", "\u{e417}"),
    ("timer", "\u{e1e0}"),
    ("house", "\u{e0f8}"),
    ("globe", "\u{e0eb}"),
    ("palette", "\u{e1dd}"),
    ("message-square", "\u{e11b}"),
    ("calendar", "\u{e067}"),
];

pub fn app_glyph(name: &str) -> Option<&'static str> {
    APP_GLYPHS.iter().find(|g| g.0 == name).map(|g| g.1)
}

/// What a project's icon shows on its coloured square.
pub enum Face {
    Letter(String),
    Glyph(&'static str),
    Image(std::path::PathBuf),
}

pub fn face(p: &Project) -> Face {
    if let Some(path) = p.icon_image.as_ref().map(std::path::PathBuf::from).filter(|p| p.exists()) {
        return Face::Image(path);
    }
    if let Some(g) = app_glyph(&p.icon_glyph) {
        return Face::Glyph(g);
    }
    Face::Letter(p.app_name.chars().next().unwrap_or('A').to_uppercase().to_string())
}

/// Folder that keeps the pictures picked as app icons.
pub fn icons_dir() -> std::path::PathBuf {
    crate::model::Store::path().parent().map(|d| d.join("icons")).unwrap_or_else(|| std::env::temp_dir().join("blockwise-icons"))
}

/// Reads a PNG or JPEG, makes it square (centre crop) and at most 1024 px,
/// and keeps a copy in Blockwise's icon folder. Returns the copy's path.
pub fn import_picture(src: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let img = image::open(src).map_err(|e| format!("Couldn't read that picture: {e}"))?.to_rgba8();
    let side = img.width().min(img.height());
    if side < 16 {
        return Err("That picture is too small for an icon".into());
    }
    let (x, y) = ((img.width() - side) / 2, (img.height() - side) / 2);
    let sq = image::imageops::crop_imm(&img, x, y, side, side).to_image();
    let sq = if side > 1024 { image::imageops::resize(&sq, 1024, 1024, image::imageops::FilterType::Lanczos3) } else { sq };
    let dir = icons_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let out = dir.join(format!("icon-{nanos}.png"));
    sq.save(&out).map_err(|e| format!("Couldn't keep the picture: {e}"))?;
    Ok(out)
}

static TEXTURES: Mutex<Option<HashMap<std::path::PathBuf, TextureHandle>>> = Mutex::new(None);

fn texture(ctx: &Context, path: &std::path::Path) -> Option<TextureHandle> {
    let mut guard = TEXTURES.lock().ok()?;
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(t) = map.get(path) {
        return Some(t.clone());
    }
    let img = image::open(path).ok()?.to_rgba8();
    let small = image::imageops::resize(&img, 192, 192, image::imageops::FilterType::Triangle);
    let ci = egui::ColorImage::from_rgba_unmultiplied([192, 192], small.as_raw());
    let t = ctx.load_texture(path.display().to_string(), ci, TextureOptions::LINEAR);
    map.insert(path.to_path_buf(), t.clone());
    Some(t)
}

/// Paints a project's icon into `r` (a rounded square).
pub fn paint(painter: &Painter, r: Rect, p: &Project) {
    paint_face(painter, r, p.icon_color, &face(p));
}

pub fn paint_face(painter: &Painter, r: Rect, color: u32, face: &Face) {
    let round = Rounding::same(r.width() * 0.24);
    match face {
        Face::Image(path) => {
            if let Some(t) = texture(painter.ctx(), path) {
                let mut shape = egui::epaint::RectShape::filled(r, round, Color32::WHITE);
                shape.fill_texture_id = t.id();
                shape.uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.add(shape);
                return;
            }
            painter.rect_filled(r, round, crate::phone::c32(color));
        }
        Face::Glyph(g) => {
            painter.rect_filled(r, round, crate::phone::c32(color));
            painter.text(r.center(), egui::Align2::CENTER_CENTER, *g, FontId::proportional(r.width() * 0.5), Color32::WHITE);
        }
        Face::Letter(l) => {
            painter.rect_filled(r, round, crate::phone::c32(color));
            painter.text(r.center(), egui::Align2::CENTER_CENTER, l, FontId::proportional(r.width() * 0.46), Color32::WHITE);
        }
    }
}

// ------------------------------------------------------------ icon files

/// How a rendered icon is shaped.
#[derive(Clone, Copy, PartialEq)]
pub enum Shape {
    /// Fills the square edge to edge (iPhone: the system rounds it).
    Full,
    /// A rounded square with clear corners (Android before adaptive icons).
    Rounded,
    /// Only the picture, on a clear 108-unit canvas (Android adaptive foreground).
    Foreground,
}

fn rgba(argb: u32) -> [u8; 4] {
    [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, 255]
}

/// Draws a project's icon as a `size`×`size` picture.
pub fn render(p: &Project, size: u32, shape: Shape) -> image::RgbaImage {
    let face = face(p);
    let s = size as f32;
    let bg = rgba(p.icon_color);
    let mut img = image::RgbaImage::new(size, size);
    if shape != Shape::Foreground {
        let radius = if shape == Shape::Rounded { s * 0.22 } else { 0.0 };
        for (x, y, px) in img.enumerate_pixels_mut() {
            let a = rounded_coverage(x as f32 + 0.5, y as f32 + 0.5, s, radius);
            if a > 0.0 {
                *px = image::Rgba([bg[0], bg[1], bg[2], (a * 255.0).round() as u8]);
            }
        }
    }
    match face {
        Face::Image(path) => {
            if let Ok(pic) = image::open(&path) {
                // the picture fills the icon; on an adaptive foreground it fills
                // the 72-unit part the launcher shows
                let side = if shape == Shape::Foreground { (s * 72.0 / 108.0).round() as u32 } else { size };
                let pic = image::imageops::resize(&pic.to_rgba8(), side, side, image::imageops::FilterType::Lanczos3);
                let off = ((size - side) / 2) as i64;
                image::imageops::overlay(&mut img, &pic, off, off);
                if shape == Shape::Rounded {
                    let radius = s * 0.22;
                    for (x, y, px) in img.enumerate_pixels_mut() {
                        let a = rounded_coverage(x as f32 + 0.5, y as f32 + 0.5, s, radius);
                        px.0[3] = (px.0[3] as f32 * a).round() as u8;
                    }
                }
            }
        }
        Face::Glyph(g) => {
            let frac = if shape == Shape::Foreground { 0.36 } else { 0.54 };
            draw_char(&mut img, LUCIDE_TTF, g.chars().next().unwrap_or(' '), s * frac);
        }
        Face::Letter(l) => {
            let frac = if shape == Shape::Foreground { 0.34 } else { 0.5 };
            let fonts = egui::FontDefinitions::default();
            if let Some(f) = fonts.font_data.get("Ubuntu-Light") {
                draw_char(&mut img, &f.font, l.chars().next().unwrap_or('A'), s * frac);
            }
        }
    }
    img
}

/// How much of the pixel at (x, y) is inside a rounded square of side `s`.
fn rounded_coverage(x: f32, y: f32, s: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 1.0;
    }
    let cx = x.clamp(radius, s - radius);
    let cy = y.clamp(radius, s - radius);
    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    (radius - d + 0.5).clamp(0.0, 1.0)
}

/// Draws one white character, centred, `px` tall at most.
fn draw_char(img: &mut image::RgbaImage, font: &[u8], ch: char, px: f32) {
    use ab_glyph::{Font, FontRef, PxScale};
    let Ok(font) = FontRef::try_from_slice(font) else { return };
    let glyph = font.glyph_id(ch).with_scale_and_position(PxScale::from(px), ab_glyph::point(0.0, 0.0));
    let Some(out) = font.outline_glyph(glyph) else { return };
    let b = out.px_bounds();
    let (w, h) = (img.width() as f32, img.height() as f32);
    let (ox, oy) = (((w - b.width()) / 2.0 - b.min.x).round(), ((h - b.height()) / 2.0 - b.min.y).round());
    out.draw(|x, y, c| {
        let (px_x, px_y) = (x as i64 + (b.min.x + ox) as i64, y as i64 + (b.min.y + oy) as i64);
        if px_x < 0 || px_y < 0 || px_x >= img.width() as i64 || px_y >= img.height() as i64 {
            return;
        }
        let p = img.get_pixel_mut(px_x as u32, px_y as u32);
        let a = c.clamp(0.0, 1.0);
        let dst_a = p.0[3] as f32 / 255.0;
        let out_a = a + dst_a * (1.0 - a);
        if out_a > 0.0 {
            for k in 0..3 {
                p.0[k] = ((255.0 * a + p.0[k] as f32 * dst_a * (1.0 - a)) / out_a).round() as u8;
            }
        }
        p.0[3] = (out_a * 255.0).round() as u8;
    });
}

/// Android launcher icons: (path inside app/src/main/res, PNG bytes or XML).
pub fn android_icon_files(p: &Project) -> Vec<(String, Vec<u8>)> {
    let mut v = vec![];
    for (dir, scale) in [("mdpi", 1.0), ("hdpi", 1.5), ("xhdpi", 2.0), ("xxhdpi", 3.0), ("xxxhdpi", 4.0)] {
        let legacy = (48.0 * scale) as u32;
        let fg = (108.0 * scale) as u32;
        v.push((format!("mipmap-{dir}/app_icon.png"), png(&render(p, legacy, Shape::Rounded))));
        v.push((format!("mipmap-{dir}/app_icon_foreground.png"), png(&render(p, fg, Shape::Foreground))));
    }
    // a picture icon sits on white, a symbol or letter on the icon colour
    let bg = if matches!(face(p), Face::Image(_)) { 0xFFFF_FFFF } else { p.icon_color };
    v.push(("values/app_icon_colors.xml".into(), format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n\t<color name=\"app_icon_background\">#{:06X}</color>\n</resources>\n", bg & 0xFF_FFFF).into_bytes()));
    v.push((
        "mipmap-anydpi-v26/app_icon.xml".into(),
        b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n\t<background android:drawable=\"@color/app_icon_background\"/>\n\t<foreground android:drawable=\"@mipmap/app_icon_foreground\"/>\n</adaptive-icon>\n".to_vec(),
    ));
    v
}

/// The iPhone app icon as an asset catalogue for Xcode: (path inside
/// Assets.xcassets, bytes).
pub fn ios_asset_files(p: &Project) -> Vec<(String, Vec<u8>)> {
    vec![
        ("Contents.json".into(), b"{\n  \"info\" : { \"author\" : \"xcode\", \"version\" : 1 }\n}\n".to_vec()),
        (
            "AppIcon.appiconset/Contents.json".into(),
            b"{\n  \"images\" : [\n    { \"filename\" : \"AppIcon.png\", \"idiom\" : \"universal\", \"platform\" : \"ios\", \"size\" : \"1024x1024\" }\n  ],\n  \"info\" : { \"author\" : \"xcode\", \"version\" : 1 }\n}\n".to_vec(),
        ),
        ("AppIcon.appiconset/AppIcon.png".into(), png(&opaque(render(p, 1024, Shape::Full)))),
    ]
}

/// Icon files put straight into a built iPhone app bundle (no Xcode asset
/// step): (file name, PNG bytes).
pub fn ios_bundle_icons(p: &Project) -> Vec<(String, Vec<u8>)> {
    vec![("AppIcon60x60@2x.png".into(), png(&opaque(render(p, 120, Shape::Full)))), ("AppIcon60x60@3x.png".into(), png(&opaque(render(p, 180, Shape::Full))))]
}

/// The iPhone wants icons without see-through parts.
fn opaque(mut img: image::RgbaImage) -> image::RgbaImage {
    for px in img.pixels_mut() {
        let a = px.0[3] as f32 / 255.0;
        for k in 0..3 {
            px.0[k] = (px.0[k] as f32 * a + 255.0 * (1.0 - a)).round() as u8;
        }
        px.0[3] = 255;
    }
    img
}

fn png(img: &image::RgbaImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut out, image::ImageFormat::Png);
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_every_kind_of_icon() {
        let mut p = Project::new(1, "Demo", "Demo", "com.example.demo");
        // letter
        let img = render(&p, 96, Shape::Rounded);
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "rounded corners are clear");
        assert_eq!(img.get_pixel(48, 4).0[3], 255);
        // symbol: some white shows in the middle
        p.icon_glyph = "star".into();
        let img = render(&p, 120, Shape::Full);
        assert!(img.pixels().any(|px| px.0[0] > 240 && px.0[1] > 240 && px.0[2] > 240));
        let fg = render(&p, 108, Shape::Foreground);
        assert_eq!(fg.get_pixel(0, 0).0[3], 0, "foreground keeps a clear edge");
        assert!(fg.pixels().any(|px| px.0[3] == 255));
        // the Android set has five sizes of each kind plus the adaptive files
        assert_eq!(android_icon_files(&p).len(), 12);
        assert!(ios_asset_files(&p).iter().any(|f| f.0.ends_with("AppIcon.png") && f.1.starts_with(b"\x89PNG")));
    }

    #[test]
    fn every_app_glyph_is_in_the_font() {
        use ab_glyph::{Font, FontRef};
        let f = FontRef::try_from_slice(LUCIDE_TTF).unwrap();
        for (name, g) in APP_GLYPHS {
            assert_ne!(f.glyph_id(g.chars().next().unwrap()).0, 0, "{name}");
        }
    }
}
