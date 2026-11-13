//! Settings of the code page and the live log.

use eframe::egui::Color32;

/// Text sizes the code page offers: 10 to 18, one step apart (nine choices).
pub const CODE_SIZES: [u8; 9] = [10, 11, 12, 13, 14, 15, 16, 17, 18];
/// Code text size when none was picked.
pub const CODE_SIZE_DEFAULT: u8 = 14;

static DEFAULT_SIZE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(CODE_SIZE_DEFAULT);

/// The code size a newly opened project starts with (from the settings page).
pub fn set_default_code_size(z: u8) {
    DEFAULT_SIZE.store(code_size(z), std::sync::atomic::Ordering::Relaxed);
}

pub fn default_code_size() -> u8 {
    DEFAULT_SIZE.load(std::sync::atomic::Ordering::Relaxed)
}

/// A remembered size is kept only when it is below the largest
/// choice; anything else (the largest one included) goes back to 14.
pub fn code_size(saved: u8) -> u8 {
    if saved < CODE_SIZES[CODE_SIZES.len() - 1] {
        saved
    } else {
        CODE_SIZE_DEFAULT
    }
}

/// The live log's own settings, from its "⚙ Text" menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogView {
    /// Long lines wrap (off: they scroll sideways).
    pub wrap: bool,
    /// Fixed-width letters.
    pub monospace: bool,
    /// Text size, LOG_SIZE_MIN..=LOG_SIZE_MAX.
    pub size: u8,
}

pub const LOG_SIZE_MIN: u8 = 10;
pub const LOG_SIZE_MAX: u8 = 70;
/// Estimated: start from the log text's own size, which its
/// layout sets; Blockwise starts at 13.
pub const LOG_SIZE_DEFAULT: u8 = 13;

impl Default for LogView {
    fn default() -> Self {
        // wrap starts off and fixed-width letters start on
        LogView { wrap: false, monospace: true, size: LOG_SIZE_DEFAULT }
    }
}

pub const ERROR_COLOR: Color32 = Color32::from_rgb(0xFE, 0x00, 0x00);
pub const WARNING_COLOR: Color32 = Color32::from_rgb(0xFF, 0xC4, 0x00);

/// How a log line is marked: "error:" anywhere, or a numbered
/// "N. ERROR" heading, shows in bold red; a numbered "N. WARNING" heading in
/// bold amber. Case matters.
pub fn mark(line: &str) -> Option<Color32> {
    let t = line.trim_start();
    let numbered = |word: &str| {
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        digits > 0 && t[digits..].starts_with(&format!(". {word}"))
    };
    if line.contains("error:") || numbered("ERROR") {
        Some(ERROR_COLOR)
    } else if numbered("WARNING") {
        Some(WARNING_COLOR)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_size_rule() {
        assert_eq!(code_size(10), 10);
        assert_eq!(code_size(17), 17);
        assert_eq!(code_size(18), 14);
        assert_eq!(code_size(40), 14);
    }

    #[test]
    fn marks_errors_and_warnings() {
        assert_eq!(mark("Main.kt:3: error: unresolved"), Some(ERROR_COLOR));
        assert_eq!(mark("1. ERROR in Main.java"), Some(ERROR_COLOR));
        assert_eq!(mark("12. WARNING in Main.java"), Some(WARNING_COLOR));
        assert_eq!(mark("Error: something"), None);
        assert_eq!(mark("value: count = 3"), None);
        let v = LogView::default();
        assert!(!v.wrap && v.monospace);
    }
}
