//! Code page colours and highlighting rules: two colour sets and the rule
//! lists.

use eframe::egui::Color32;

/// One colour set of the code page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeTheme {
    pub background: Color32,
    pub text: Color32,
    pub line_highlight: Color32,
    pub line_numbers: Color32,
    /// Keywords.
    pub primary: Color32,
    /// Words starting with a capital letter (Java), tags (XML).
    pub class: Color32,
    /// ( ) , ; < > { } *
    pub symbols: Color32,
    pub strings_numbers: Color32,
    pub comments: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The "light" set.
pub const LIGHT: CodeTheme = CodeTheme {
    background: hex(0xFAFAFA),
    text: hex(0x000000),
    line_highlight: hex(0xEFEFEF),
    line_numbers: hex(0x888888),
    primary: hex(0x0096FF),
    class: hex(0x0096FF),
    symbols: hex(0x0096FF),
    strings_numbers: hex(0xE91E63),
    comments: hex(0x009B00),
};

/// The "dark" set.
pub const DARK: CodeTheme = CodeTheme {
    background: hex(0x292929),
    text: hex(0xD8D8D8),
    line_highlight: hex(0x393939),
    line_numbers: hex(0x646567),
    primary: hex(0xD78B40),
    class: hex(0xF7CF80),
    symbols: hex(0xAAAAB3),
    strings_numbers: hex(0x66A069),
    comments: hex(0x707070),
};

/// The dark set when asked for dark, the light one otherwise.
pub fn theme(dark: bool) -> CodeTheme {
    if dark {
        DARK
    } else {
        LIGHT
    }
}

/// The keyword list.
const KEYWORDS: &[&str] = &[
    "String", "int", "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue", "default", "do", "double", "else", "enum", "extends", "final", "finally", "float", "for", "goto", "if", "implements", "import", "instanceof", "interface", "long", "native", "new", "package", "private", "protected", "public", "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws", "transient", "try", "void", "volatile", "while", "true", "false", "null",
];

/// Kotlin and Swift words too: Blockwise exports
/// Kotlin and Swift, so their own keywords get the keyword colour too.
const KEYWORDS_KT_SWIFT: &[&str] = &[
    "fun", "val", "var", "let", "func", "override", "struct", "object", "when", "in", "is", "as", "guard", "self", "nil", "init", "extension", "protocol", "data", "companion", "lateinit", "internal", "open", "private", "fileprivate", "mutating", "some", "inout", "defer", "repeat", "Unit",
];

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Colour of every character of `text` (None: plain text colour). The rules
/// run in order and a later rule paints over an earlier one.
pub fn colours(text: &str, xml: bool, t: &CodeTheme) -> Vec<Option<Color32>> {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out = vec![None; n];
    let paint = |out: &mut Vec<Option<Color32>>, a: usize, b: usize, col: Color32| {
        for o in &mut out[a..b.min(n)] {
            *o = Some(col);
        }
    };
    if !xml {
        // 1. keywords; 2. words starting with a capital (two letters or more)
        let mut i = 0;
        while i < n {
            if word_char(c[i]) && (i == 0 || !word_char(c[i - 1])) {
                let mut j = i;
                while j < n && word_char(c[j]) {
                    j += 1;
                }
                let w: String = c[i..j].iter().collect();
                if KEYWORDS.contains(&w.as_str()) || KEYWORDS_KT_SWIFT.contains(&w.as_str()) {
                    paint(&mut out, i, j, t.primary);
                }
                i = j;
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < n {
            if word_char(c[i]) && (i == 0 || !word_char(c[i - 1])) {
                let mut j = i;
                while j < n && word_char(c[j]) {
                    j += 1;
                }
                if c[i].is_ascii_uppercase() && j - i >= 2 {
                    paint(&mut out, i, j, t.class);
                }
                i = j;
            } else {
                i += 1;
            }
        }
    } else {
        // 1. tags: "<", optional "/", a name of letters, "_" or ".", optional "/" and ">"
        let mut i = 0;
        while i < n {
            if c[i] == '<' {
                let mut j = i + 1;
                if j < n && c[j] == '/' {
                    j += 1;
                }
                let start = j;
                while j < n && (c[j].is_ascii_alphabetic() || c[j] == '_' || c[j] == '.') {
                    j += 1;
                }
                if j > start {
                    if j < n && c[j] == '/' {
                        j += 1;
                    }
                    if j < n && c[j] == '>' {
                        j += 1;
                    }
                    paint(&mut out, i, j, t.class);
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
    }
    // symbols
    for i in 0..n {
        if "(),;<>{}*".contains(c[i]) {
            out[i] = Some(t.symbols);
        }
    }
    // strings ("..." or '...' on one line), and whole numbers in code
    let mut i = 0;
    while i < n {
        if c[i] == '"' || c[i] == '\'' {
            if let Some(k) = (i + 1..n).take_while(|&k| c[k] != '\n').find(|&k| c[k] == c[i]) {
                paint(&mut out, i, k + 1, t.strings_numbers);
                i = k + 1;
                continue;
            }
        } else if !xml && c[i].is_ascii_digit() && (i == 0 || !word_char(c[i - 1])) {
            let mut j = i;
            while j < n && c[j].is_ascii_digit() {
                j += 1;
            }
            if j == n || !word_char(c[j]) {
                paint(&mut out, i, j, t.strings_numbers);
            }
            i = j;
            continue;
        }
        i += 1;
    }
    // comments, last of all
    let starts = |i: usize, s: &str| s.chars().enumerate().all(|(k, ch)| c.get(i + k) == Some(&ch));
    let mut i = 0;
    while i < n {
        let (open, close) = if xml { ("<!--", "-->") } else { ("/*", "*/") };
        if starts(i, open) {
            let mut j = i + open.len();
            while j < n && !starts(j, close) {
                j += 1;
            }
            if j < n {
                let end = j + close.len();
                paint(&mut out, i, end, t.comments);
                i = end;
                continue;
            }
        }
        if !xml && starts(i, "//") {
            let mut j = i;
            while j < n && c[j] != '\n' {
                j += 1;
            }
            paint(&mut out, i, j, t.comments);
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_set() {
        assert_eq!(theme(true).background, hex(0x292929));
        assert_eq!(theme(false).background, hex(0xFAFAFA));
    }

    #[test]
    fn java_rules() {
        let t = DARK;
        let s = "public Foo x = \"a\"; // hi";
        let col = colours(s, false, &t);
        assert_eq!(col[0], Some(t.primary)); // public
        assert_eq!(col[7], Some(t.class)); // Foo
        assert_eq!(col[11], None); // x
        assert_eq!(col[15], Some(t.strings_numbers)); // "a"
        assert_eq!(col[18], Some(t.symbols)); // ;
        assert_eq!(col[20], Some(t.comments)); // //
        // a comment paints over a string inside it
        let col = colours("/* \"x\" */ 42", false, &t);
        assert_eq!(col[3], Some(t.comments));
        assert_eq!(col[10], Some(t.strings_numbers));
    }

    #[test]
    fn xml_rules() {
        let t = LIGHT;
        let s = "<TextView a=\"b\"/> <!-- c -->";
        let col = colours(s, true, &t);
        assert_eq!(col[1], Some(t.class));
        assert_eq!(col[12], Some(t.strings_numbers));
        assert_eq!(col[20], Some(t.comments));
    }
}
