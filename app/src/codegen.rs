//! Shared code generation: the block-to-code walker used for Kotlin and Swift,
//! the layout XML, and exporting.

use crate::blocks::*;
use crate::model::*;

/// Which language a block is written in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Kotlin,
    Swift,
}

pub(crate) struct Gen<'a> {
    pub screen: &'a Screen,
    pub repeat_depth: usize,
    pub lang: Lang,
    /// Debug builds: log each event, block and variable change (tag "Blockwise").
    pub trace: bool,
    /// How many web requests' answer stacks we are inside: the answer's
    /// text and code only exist there.
    pub web_depth: usize,
    /// Inside a camera or pick block, where "file path" has a value.
    pub media_depth: usize,
    /// The inputs of the more block being written (they read as `_p_name`).
    pub params: Vec<String>,
}

/// Tag the debug builds log under, so the live log can pick them out.
pub const LOG_TAG: &str = "Blockwise";

pub(crate) fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A string literal for the language (Kotlin also escapes `$`).
pub(crate) fn quote_for(s: &str, lang: Lang) -> String {
    let q = quote(s);
    if lang == Lang::Kotlin {
        q.replace('$', "\\$")
    } else {
        q
    }
}

fn num_literal(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return "0".into();
    }
    if t.parse::<f64>().is_ok() {
        t.to_string()
    } else {
        "0".into()
    }
}

/// Kotlin and Swift numbers are Doubles: write "2" as "2.0", and keep
/// negative numbers in brackets.
fn num_literal_for(s: &str, lang: Lang) -> String {
    let _ = lang;
    let n = num_literal(s);
    let n = if n.contains('.') || n.contains('e') || n.contains('E') { n } else { format!("{n}.0") };
    if n.starts_with('-') {
        format!("({n})")
    } else {
        n
    }
}

impl<'a> Gen<'a> {
    pub fn new(screen: &'a Screen, lang: Lang) -> Self {
        Gen { screen, repeat_depth: 0, lang, trace: false, web_depth: 0, media_depth: 0, params: vec![] }
    }

    /// One debug log line (Kotlin: Log.d, Swift: print).
    fn log_text(&self, text: &str) -> String {
        // braces would upset the re-indenting, which counts them
        let text = text.replace('{', "(").replace('}', ")");
        let text = text.as_str();
        match self.lang {
            Lang::Kotlin => format!("Log.d(\"{LOG_TAG}\", {})", quote_for(text, Lang::Kotlin)),
            Lang::Swift => format!("print({})", quote(&format!("[{LOG_TAG}] {text}"))),
        }
    }

    /// A debug line showing a variable's new value.
    fn log_value(&self, name: &str) -> String {
        match self.lang {
            Lang::Kotlin => format!("Log.d(\"{LOG_TAG}\", \"value: {name} = \" + {name})"),
            Lang::Swift => format!("print(\"[{LOG_TAG}] value: {name} = \\({name})\")"),
        }
    }

    /// An event's blocks, with a "this event started" line first in debug builds.
    pub fn event_body(&mut self, e: &Event) -> String {
        let body = self.stack(&e.blocks);
        if self.trace {
            format!("{}\n{body}", self.log_text(&format!("event: {}", e.title())))
        } else {
            body
        }
    }

    pub(crate) fn arg(&mut self, a: Option<&Arg>, p: &Param) -> String {
        match a {
            Some(Arg::Block(b)) => {
                let code = self.block(b);
                match shape_of(b, self.screen) {
                    Shape::Num | Shape::Bool => {
                        if matches!(b.op.as_str(), "getVar" | "true" | "false") || !code.contains(' ') {
                            code
                        } else {
                            format!("({code})")
                        }
                    }
                    // "a + b".startsWith(...) would only ask b
                    _ if code.contains(' ') && !(code.starts_with('"') && code.ends_with('"') && code.matches('"').count() == 2) => format!("({code})"),
                    _ => code,
                }
            }
            Some(Arg::Text(t)) => match p {
                Param::Text => quote_for(t, self.lang),
                Param::Num => num_literal_for(t, self.lang),
                Param::Bool => "false".into(),
                Param::Menu(m) if m == "color" => match self.lang {
                    Lang::Kotlin => {
                        if t.starts_with("0x") {
                            format!("{t}.toInt()")
                        } else {
                            t.clone()
                        }
                    }
                    Lang::Swift => {
                        if t.starts_with("0x") {
                            t.clone()
                        } else {
                            "0x00000000".into()
                        }
                    }
                },
                Param::Menu(m) if m == "visible" && self.lang == Lang::Swift => match t.as_str() {
                    "INVISIBLE" => "1".into(),
                    "GONE" => "2".into(),
                    _ => "0".into(),
                },
                Param::Menu(m) if m == "screen" && self.lang == Lang::Swift => quote(t),
                Param::Menu(m) if m == "method" => quote(t),
                // a picture or sound is named by text, so none is just ""
                Param::Menu(m) if matches!(m.as_str(), "image" | "sound" | "font" | "asset" | "video" | "mediaKind") => quote_for(t, self.lang),
                Param::Menu(_) => {
                    if t.is_empty() {
                        if self.lang == Lang::Swift { "\"\"".into() } else { "null".into() }
                    } else {
                        t.clone()
                    }
                }
            },
            None => match p {
                Param::Text => "\"\"".into(),
                Param::Num => num_literal_for("0", self.lang),
                Param::Bool => "false".into(),
                Param::Menu(_) => if self.lang == Lang::Swift { "\"\"".into() } else { "null".into() },
            },
        }
    }

    pub(crate) fn stack(&mut self, blocks: &[Block]) -> String {
        let mut out = vec![];
        for b in blocks {
            if self.trace {
                out.push(self.log_text(&format!("block: {}", describe(b))));
            }
            out.push(self.block(b));
            if self.trace && matches!(b.op.as_str(), "setVarBoolean" | "setVarInt" | "setVarString" | "increaseInt" | "decreaseInt") {
                if let Some(Arg::Text(name)) = b.args.first() {
                    if !name.is_empty() {
                        out.push(self.log_value(name));
                    }
                }
            }
        }
        out.join("\n")
    }

    fn block(&mut self, b: &Block) -> String {
        let Some(d) = def(&b.op) else { return format!("// unknown block {}", b.op) };
        // outside a request there is no answer: nothing, as in the preview
        match d.op {
            "webResponse" | "supaError" if self.web_depth == 0 => return "\"\"".into(),
            "webStatus" if self.web_depth == 0 => return num_literal_for("0", self.lang),
            "mediaPath" if self.media_depth == 0 => return "\"\"".into(),
            _ => {}
        }
        // a more block input read outside its own definition has no value
        if matches!(d.op, "paramStr" | "paramNum" | "paramBool") {
            let n = match b.args.first() {
                Some(Arg::Text(t)) => t.as_str(),
                _ => "",
            };
            if !self.params.iter().any(|p| p == n) {
                return match d.op {
                    "paramNum" => num_literal_for("0", self.lang),
                    "paramBool" => "false".into(),
                    _ => "\"\"".into(),
                };
            }
        }
        let spec = spec_for(b, self.screen);
        let toks = parse_spec(&spec);
        let mut args = vec![];
        for t in &toks {
            match t {
                Token::Param(i, p) => {
                    let s = self.arg(b.args.get(*i), p);
                    args.push(s);
                }
                Token::ArgLabel(i) => {
                    if let Some(Arg::Text(t)) = b.args.get(*i) {
                        args.push(t.clone());
                    } else {
                        args.push(String::new());
                    }
                }
                Token::Label(_) => {}
            }
        }
        if d.op == "moreBlock" {
            let name = args.first().cloned().unwrap_or_default();
            return format!("_{name}({})", args[1..].join(", "));
        }
        let tpl = match self.lang {
            Lang::Kotlin => crate::native::kotlin_tpl(d.op),
            Lang::Swift => crate::native::swift_tpl(d.op),
        };
        let mut code = match tpl {
            Some(t) => t.to_string(),
            None => {
                let what = describe(b);
                let comment = if self.lang == Lang::Swift { "not available on iOS" } else { "not available" };
                return match d.shape {
                    Shape::Num => format!("0.0 /* {what}: {comment} */"),
                    Shape::Str => format!("\"\" /* {what}: {comment} */"),
                    Shape::Bool => format!("false /* {what}: {comment} */"),
                    _ => format!("// {what}: {comment}"),
                };
            }
        };
        if code.contains("{R}") {
            let r = format!("_repeat{}", 10 + self.repeat_depth * 10 + 1);
            code = code.replace("{R}", &r);
        }
        for (i, a) in args.iter().enumerate().rev() {
            code = code.replace(&format!("{{{i}}}"), a);
        }
        let web = d.op == "webRequest" || (d.op.starts_with("supa") && d.shape == Shape::E);
        let media = matches!(d.op, "cameraPhoto" | "cameraVideo" | "pickMedia");
        if web {
            self.web_depth += 1;
        }
        if media {
            self.media_depth += 1;
        }
        if code.contains("{S1}") {
            if d.op == "repeat" {
                self.repeat_depth += 1;
            }
            let s1 = self.stack(&b.sub1);
            if d.op == "repeat" {
                self.repeat_depth -= 1;
            }
            code = code.replace("{S1}", &s1);
        }
        if code.contains("{S2}") {
            let s2 = self.stack(&b.sub2);
            code = code.replace("{S2}", &s2);
        }
        if web {
            self.web_depth -= 1;
        }
        if media {
            self.media_depth -= 1;
        }
        code
    }
}

/// Re-indent generated code by counting braces.
pub(crate) fn indent(src: &str, base: usize) -> String {
    let mut out = String::new();
    let mut depth = base as i32;
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() {
            out.push('\n');
            continue;
        }
        let closes = line.starts_with('}');
        if closes {
            depth -= 1;
        }
        for _ in 0..depth.max(0) {
            out.push('\t');
        }
        out.push_str(line);
        out.push('\n');
        let opens = line.matches('{').count() as i32 - line.matches('}').count() as i32 + if closes { 1 } else { 0 };
        depth += opens;
    }
    out
}

fn color_hex(c: u32) -> String {
    format!("#{:08X}", c)
}

fn xml_widget(s: &Screen, w: &Widget, depth: usize, out: &mut String) {
    let pad = "\t".repeat(depth);
    let tag = w.kind.android_class();
    out.push_str(&format!("{pad}<{tag}\n"));
    let a = |out: &mut String, k: &str, v: &str| out.push_str(&format!("{pad}\tandroid:{k}=\"{v}\"\n"));
    a(out, "id", &format!("@+id/{}", w.id));
    // a weighted child takes its share along the parent's direction: 0dp
    // there, so several weights split the room the way they read
    let parent = s.widgets.iter().find(|p| p.id == w.parent).filter(|p| matches!(p.kind, WKind::LinearH | WKind::LinearV));
    let share = |horizontal: bool| w.weight > 0.0 && parent.is_some_and(|p| (p.kind == WKind::LinearH) == horizontal);
    a(out, "layout_width", &if share(true) { "0dp".into() } else { w.width.xml() });
    a(out, "layout_height", &if share(false) { "0dp".into() } else { w.height.xml() });
    if w.padding != 0 {
        a(out, "padding", &format!("{}dp", w.padding));
    }
    if w.margin != 0 {
        a(out, "layout_margin", &format!("{}dp", w.margin));
    }
    if (w.bg >> 24) != 0 {
        a(out, "background", &color_hex(w.bg));
    }
    if let Some(g) = crate::model::gravity_xml(w.gravity, w.vgravity) {
        a(out, "gravity", &g);
    }
    if w.weight > 0.0 {
        a(out, "layout_weight", &format!("{}", w.weight));
    }
    if matches!(w.kind, WKind::LinearH | WKind::LinearV) {
        a(out, "orientation", if w.kind == WKind::LinearH { "horizontal" } else { "vertical" });
    }
    if w.kind.has_text() {
        if !w.text.is_empty() {
            a(out, "text", &xml_escape(&w.text));
        }
        a(out, "textSize", &format!("{}sp", w.text_size));
        a(out, "textColor", &color_hex(w.text_color));
        if w.bold {
            a(out, "textStyle", "bold");
        }
    }
    if w.kind == WKind::EditText {
        a(out, "hint", &xml_escape(&w.hint));
        a(out, "textColorHint", &color_hex(w.hint_color));
        let input = match w.input_kind() {
            InputKind::Text => "",
            InputKind::Password => "textPassword",
            InputKind::Email => "textEmailAddress",
            InputKind::Number => "number|numberSigned",
            InputKind::Decimal => "numberDecimal|numberSigned",
            InputKind::Phone => "phone",
            InputKind::Multiline => "textMultiLine|textCapSentences",
        };
        if !input.is_empty() {
            a(out, "inputType", input);
        }
    }
    if w.kind.is_compound() && w.checked {
        a(out, "checked", "true");
    }
    if matches!(w.kind, WKind::ProgressBar | WKind::SeekBar) {
        a(out, "max", &w.max.to_string());
        a(out, "progress", &w.progress.to_string());
    }
    if w.kind == WKind::ProgressBar {
        out.push_str(&format!("{pad}\tstyle=\"?android:progressBarStyleHorizontal\"\n"));
    }
    if w.kind == WKind::ImageView {
        // the picture is set in the code, which copes with a missing file
        if w.crop {
            a(out, "scaleType", "centerCrop");
        } else {
            a(out, "scaleType", "fitCenter");
            a(out, "adjustViewBounds", "true");
        }
    }
    if !w.enabled {
        a(out, "enabled", "false");
    }
    if !w.visible {
        a(out, "visibility", "gone");
    }
    let kids = s.children(&w.id);
    if w.kind.is_container() && !kids.is_empty() {
        // a ScrollView usually holds one LinearLayout; we keep the children directly
        out.truncate(out.len() - 1);
        out.push_str(">\n");
        for i in kids {
            xml_widget(s, &s.widgets[i], depth + 1, out);
        }
        out.push_str(&format!("{pad}</{tag}>\n"));
    } else {
        out.truncate(out.len() - 1);
        out.push_str("/>\n");
    }
}

pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn xml(s: &Screen) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str("<LinearLayout\n\txmlns:android=\"http://schemas.android.com/apk/res/android\"\n\tandroid:layout_width=\"match_parent\"\n\tandroid:layout_height=\"match_parent\"\n\tandroid:orientation=\"vertical\">\n");
    for i in s.children("root") {
        xml_widget(s, &s.widgets[i], 1, &mut out);
    }
    out.push_str("</LinearLayout>\n");
    out
}

/// The folder exports go in when none was picked: ~/Blockwise.
pub fn default_export_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_else(std::env::temp_dir).join("Blockwise")
}

/// Write the Kotlin (Android) and/or Swift (iPhone) projects into a folder
/// named after the project inside `parent`; returns that folder.
pub fn export(p: &Project, parent: &std::path::Path, kotlin: bool, swift: bool) -> std::io::Result<std::path::PathBuf> {
    let base = parent.join(sanitize(&p.name));
    std::fs::create_dir_all(&base)?;
    crate::native::export_native(p, &base, kotlin, swift)?;
    Ok(base)
}

pub(crate) fn sanitize(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    if t.is_empty() {
        "project".into()
    } else {
        t
    }
}

#[cfg(test)]
mod weight_tests {
    use super::*;

    #[test]
    fn weights_are_zero_along_the_parent() {
        let mut p = crate::app::sample_project(1);
        let s = &mut p.screens[0];
        for w in s.widgets.iter_mut() {
            match w.id.as_str() {
                "linear2" => w.weight = 1.0,
                "button1" | "button2" => w.weight = 1.0,
                _ => {}
            }
        }
        let mut out = String::new();
        let s = &p.screens[0];
        for w in &s.widgets {
            if ["linear2", "button1"].contains(&w.id.as_str()) {
                xml_widget(s, w, 0, &mut out);
            }
        }
        // the row sits in a column: its height shares; the button in a row: its width
        assert!(out.contains("@+id/linear2\"\n\tandroid:layout_width=\"match_parent\"\n\tandroid:layout_height=\"0dp\""), "{out}");
        assert!(out.contains("@+id/button1\"\n\tandroid:layout_width=\"0dp\""), "{out}");
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;

    #[test]
    fn text_fields_bring_up_their_keyboard_on_both_phones() {
        let mut p = crate::app::sample_project(1);
        for (id, k) in [("pw", InputKind::Password), ("mail", InputKind::Email), ("age", InputKind::Number), ("tel", InputKind::Phone), ("note", InputKind::Multiline)] {
            let mut w = Widget::new(id.into(), WKind::EditText, "linear1".into());
            w.input = k;
            p.screens[0].widgets.push(w);
        }
        let s = &p.screens[0];
        let mut xml = String::new();
        for w in s.widgets.iter().filter(|w| w.kind == WKind::EditText) {
            xml_widget(s, w, 0, &mut xml);
        }
        for t in ["textPassword", "textEmailAddress", "number|numberSigned", "\"phone\"", "textMultiLine"] {
            assert!(xml.contains(t), "{t} missing from {xml}");
        }
        let kt = crate::native::kotlin(&p, s);
        assert!(kt.contains("AppUtil.passwordEye(pw)"), "{kt}");
        let sw = crate::native::swift_screen(&p, s);
        assert!(sw.contains("PasswordField(hint: m.pw.hint"), "{sw}");
        assert!(sw.contains(".keyboardType(.emailAddress)") && sw.contains(".keyboardType(.phonePad)"), "{sw}");
        assert!(sw.contains("axis: .vertical"), "{sw}");
        assert!(crate::native::swift_helpers(&p).contains("struct PasswordField"));
    }
}
