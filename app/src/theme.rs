//! Light and dark colours for Blockwise's own panels. The pretend phone and
//! the blocks keep their colours in both themes: they show how the app looks.

use eframe::egui::Color32;
use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(false);

pub fn set_dark(on: bool) {
    DARK.store(on, Ordering::Relaxed);
}

pub fn dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

/// The background behind everything (canvas, page).
pub fn bg() -> Color32 {
    if dark() { Color32::from_rgb(0x16, 0x19, 0x1F) } else { Color32::from_rgb(0xF0, 0xF3, 0xFB) }
}

/// Panels, cards and side bars.
pub fn surface() -> Color32 {
    if dark() { Color32::from_rgb(0x22, 0x26, 0x2E) } else { Color32::WHITE }
}

/// A surface under the pointer.
pub fn surface_hover() -> Color32 {
    if dark() { Color32::from_rgb(0x2B, 0x31, 0x3B) } else { Color32::from_rgb(0xF7, 0xFB, 0xFE) }
}

/// Main text.
pub fn ink() -> Color32 {
    if dark() { Color32::from_rgb(0xE4, 0xE7, 0xEE) } else { Color32::from_rgb(0x1D, 0x21, 0x29) }
}

/// A grey that keeps its contrast: light-theme grey `n` flips in the dark theme.
pub fn gray(n: u8) -> Color32 {
    if dark() { Color32::from_gray(255u8.saturating_sub(n).saturating_add(25)) } else { Color32::from_gray(n) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_flips_surfaces_and_text() {
        set_dark(false);
        assert_eq!(surface(), Color32::WHITE);
        let light_ink = ink();
        set_dark(true);
        assert_ne!(surface(), Color32::WHITE);
        assert!(ink().r() > light_ink.r());
        assert!(gray(110).r() > 110);
        set_dark(false);
    }
}
