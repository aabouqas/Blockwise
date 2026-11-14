//! Data model: projects, screens, widgets, variables, components, events and blocks.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn new_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn bump_ids(min: u64) {
    let cur = NEXT_ID.load(Ordering::Relaxed);
    if min >= cur {
        NEXT_ID.store(min + 1, Ordering::Relaxed);
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum WKind {
    LinearH,
    LinearV,
    ScrollH,
    ScrollV,
    Button,
    TextView,
    EditText,
    ImageView,
    WebView,
    ProgressBar,
    ListView,
    Spinner,
    CheckBox,
    Switch,
    SeekBar,
    CalendarView,
    /// Plays one of the project's videos.
    VideoView,
}

impl WKind {
    pub const LAYOUTS: [WKind; 4] = [WKind::LinearH, WKind::LinearV, WKind::ScrollH, WKind::ScrollV];
    pub const WIDGETS: [WKind; 13] = [
        WKind::Button,
        WKind::TextView,
        WKind::EditText,
        WKind::ImageView,
        WKind::WebView,
        WKind::ProgressBar,
        WKind::ListView,
        WKind::Spinner,
        WKind::CheckBox,
        WKind::Switch,
        WKind::SeekBar,
        WKind::CalendarView,
        WKind::VideoView,
    ];

    /// Platform-neutral names shown in the editor (the generated code still
    /// uses each phone's own class names).
    pub fn palette_name(self) -> &'static str {
        match self {
            WKind::LinearH => "Row",
            WKind::LinearV => "Column",
            WKind::ScrollH => "Scrolling row",
            WKind::ScrollV => "Scrolling column",
            WKind::Button => "Button",
            WKind::TextView => "Text",
            WKind::EditText => "Text field",
            WKind::ImageView => "Image",
            WKind::WebView => "Web page",
            WKind::ProgressBar => "Progress bar",
            WKind::ListView => "List",
            WKind::Spinner => "Dropdown",
            WKind::CheckBox => "Checkbox",
            WKind::Switch => "Switch",
            WKind::SeekBar => "Slider",
            WKind::CalendarView => "Calendar",
            WKind::VideoView => "Video",
        }
    }

    /// Android view class name used in the generated Kotlin and layouts.
    pub fn android_class(self) -> &'static str {
        match self {
            WKind::LinearH | WKind::LinearV => "LinearLayout",
            WKind::ScrollH => "HorizontalScrollView",
            WKind::ScrollV => "ScrollView",
            WKind::Button => "Button",
            WKind::TextView => "TextView",
            WKind::EditText => "EditText",
            WKind::ImageView => "ImageView",
            WKind::WebView => "WebView",
            WKind::ProgressBar => "ProgressBar",
            WKind::ListView => "ListView",
            WKind::Spinner => "Spinner",
            WKind::CheckBox => "CheckBox",
            WKind::Switch => "Switch",
            WKind::SeekBar => "SeekBar",
            WKind::CalendarView => "CalendarView",
            WKind::VideoView => "VideoView",
        }
    }

    /// Id prefix: type name + number (linear1, button1...).
    pub fn id_prefix(self) -> &'static str {
        match self {
            WKind::LinearH | WKind::LinearV => "linear",
            WKind::ScrollH => "hscroll",
            WKind::ScrollV => "vscroll",
            WKind::Button => "button",
            WKind::TextView => "textview",
            WKind::EditText => "edittext",
            WKind::ImageView => "imageview",
            WKind::WebView => "webview",
            WKind::ProgressBar => "progressbar",
            WKind::ListView => "listview",
            WKind::Spinner => "spinner",
            WKind::CheckBox => "checkbox",
            WKind::Switch => "switch",
            WKind::SeekBar => "seekbar",
            WKind::CalendarView => "calendarview",
            WKind::VideoView => "videoview",
        }
    }

    pub fn is_container(self) -> bool {
        matches!(self, WKind::LinearH | WKind::LinearV | WKind::ScrollH | WKind::ScrollV)
    }
    pub fn is_horizontal(self) -> bool {
        matches!(self, WKind::LinearH | WKind::ScrollH)
    }
    pub fn has_text(self) -> bool {
        matches!(self, WKind::Button | WKind::TextView | WKind::EditText | WKind::CheckBox | WKind::Switch)
    }
    pub fn is_compound(self) -> bool {
        matches!(self, WKind::CheckBox | WKind::Switch)
    }

    /// Events offered in the Event tab for this kind of widget.
    pub fn events(self) -> &'static [&'static str] {
        match self {
            WKind::Button | WKind::TextView | WKind::ImageView | WKind::LinearH | WKind::LinearV => &["onClick", "onLongClick"],
            WKind::CheckBox | WKind::Switch => &["onCheckedChange"],
            WKind::EditText => &["onTextChanged"],
            WKind::SeekBar => &["onProgressChanged"],
            WKind::ListView => &["onItemClicked"],
            WKind::Spinner => &["onItemSelected"],
            WKind::VideoView => &["onCompletion"],
            _ => &[],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Size {
    Wrap,
    Match,
    Px(i32),
}

impl Size {
    pub fn xml(self) -> String {
        match self {
            Size::Wrap => "wrap_content".into(),
            Size::Match => "match_parent".into(),
            Size::Px(p) => format!("{p}dp"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Gravity {
    None,
    Left,
    Center,
    Right,
}

impl Gravity {
    /// Where along the width things sit: 0 left, 0.5 middle, 1 right.
    pub fn frac(self) -> f32 {
        match self {
            Gravity::Center => 0.5,
            Gravity::Right => 1.0,
            _ => 0.0,
        }
    }
}

/// Vertical gravity, picked on its own: top / center_vertical / bottom
/// (0x30, 0x10, 0x50).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
pub enum VGravity {
    #[default]
    None,
    Top,
    Center,
    Bottom,
}

impl VGravity {
    /// Where along the height things sit: 0 top, 0.5 middle, 1 bottom.
    pub fn frac(self) -> f32 {
        match self {
            VGravity::Center => 0.5,
            VGravity::Bottom => 1.0,
            _ => 0.0,
        }
    }
}

/// The gravity as one number, the sum of the ticked choices (Android's flags:
/// left 3, right 5,
/// center_horizontal 1, top 0x30, bottom 0x50, center_vertical 0x10).
pub fn gravity_flags(h: Gravity, v: VGravity) -> i32 {
    let hf = match h {
        Gravity::Left => 3,
        Gravity::Right => 5,
        Gravity::Center => 1,
        Gravity::None => 0,
    };
    let vf = match v {
        VGravity::Top => 0x30,
        VGravity::Bottom => 0x50,
        VGravity::Center => 0x10,
        VGravity::None => 0,
    };
    hf | vf
}

/// Splits a stored gravity number back into the two choices: the low part
/// (value & 7) is 1 for center, else 3 for left, 5 for right; the high part
/// (value & 0x70) is 0x10 for center, else 0x30 for top, 0x50 for bottom.
pub fn gravity_from_flags(value: i32) -> (Gravity, VGravity) {
    let h = value & 7;
    let v = value & 0x70;
    let hg = if h == 1 {
        Gravity::Center
    } else if h & 5 == 5 {
        Gravity::Right
    } else if h & 3 == 3 {
        Gravity::Left
    } else {
        Gravity::None
    };
    let vg = if v == 0x10 {
        VGravity::Center
    } else if v & 0x50 == 0x50 {
        VGravity::Bottom
    } else if v & 0x30 == 0x30 {
        VGravity::Top
    } else {
        VGravity::None
    };
    (hg, vg)
}

/// The Android gravity value for a widget; None when nothing is picked.
pub fn gravity_xml(h: Gravity, v: VGravity) -> Option<String> {
    let hs = match h {
        Gravity::Left => Some("left"),
        Gravity::Center => Some("center_horizontal"),
        Gravity::Right => Some("right"),
        Gravity::None => None,
    };
    let vs = match v {
        VGravity::Top => Some("top"),
        VGravity::Center => Some("center_vertical"),
        VGravity::Bottom => Some("bottom"),
        VGravity::None => None,
    };
    match (hs, vs) {
        (Some("center_horizontal"), Some("center_vertical")) => Some("center".into()),
        (Some(a), Some(b)) => Some(format!("{a}|{b}")),
        (Some(a), None) | (None, Some(a)) => Some(a.into()),
        (None, None) => None,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Widget {
    pub id: String,
    pub kind: WKind,
    pub parent: String,
    pub width: Size,
    pub height: Size,
    pub padding: i32,
    pub margin: i32,
    pub gravity: Gravity,
    #[serde(default)]
    pub vgravity: VGravity,
    /// ARGB; alpha 0 = transparent (default 0x00FFFFFF).
    pub bg: u32,
    pub text: String,
    pub text_size: f32,
    pub text_color: u32,
    pub bold: bool,
    pub hint: String,
    pub hint_color: u32,
    pub checked: bool,
    pub progress: i32,
    pub max: i32,
    pub enabled: bool,
    pub visible: bool,
    pub weight: f32,
    /// A text field whose letters show as dots (older projects; `input`
    /// says it now).
    #[serde(default)]
    pub password: bool,
    /// What a text field takes: the keyboard it brings up and how it shows
    /// what's typed.
    #[serde(default)]
    pub input: InputKind,
    /// Rounded corners, in dp.
    #[serde(default)]
    pub radius: i32,
    /// How far the widget lifts off the screen (its shadow), in dp. Only
    /// shows on a widget with a background colour.
    #[serde(default)]
    pub elevation: i32,
    /// An outline around the widget: its width in dp (0: none) and colour.
    #[serde(default)]
    pub border: i32,
    #[serde(default = "default_border_color")]
    pub border_color: u32,
    /// An image widget's picture: the name of one of the project's files.
    #[serde(default)]
    pub image: String,
    /// An image fills its box and is cut to fit, instead of fitting inside.
    #[serde(default)]
    pub crop: bool,
    /// The letters' font: one of the project's font files ("" for the phone's own).
    #[serde(default)]
    pub font: String,
    /// A video widget's video (a project file's name), and how it plays.
    #[serde(default)]
    pub video: String,
    #[serde(default)]
    pub autoplay: bool,
    #[serde(default)]
    pub looping: bool,
    /// Play/pause and the time bar over the video.
    #[serde(default = "yes")]
    pub controls: bool,
}

fn yes() -> bool {
    true
}

fn default_border_color() -> u32 {
    0xFF9E_9E9E
}

impl Widget {
    /// What the text field takes (a field from before input types, with
    /// dots on, is a password field).
    pub fn input_kind(&self) -> InputKind {
        if self.input == InputKind::Text && self.password {
            InputKind::Password
        } else {
            self.input
        }
    }

    pub fn new(id: String, kind: WKind, parent: String) -> Self {
        // Starting text: the widget's palette label.
        let text = match kind {
            WKind::Button => "Button",
            WKind::TextView => "TextView",
            WKind::CheckBox => "CheckBox",
            WKind::Switch => "Switch",
            _ => "",
        }
        .to_string();
        // Starting sizes: rows and scrolling layouts fill the width, the
        // rest wrap their content.
        let (width, height) = match kind {
            WKind::LinearH | WKind::ScrollH => (Size::Match, Size::Wrap),
            WKind::LinearV | WKind::ScrollV => (Size::Wrap, Size::Match),
            WKind::ListView | WKind::Spinner | WKind::SeekBar | WKind::ProgressBar => (Size::Match, Size::Wrap),
            WKind::WebView | WKind::CalendarView => (Size::Match, Size::Match),
            WKind::VideoView => (Size::Match, Size::Px(200)),
            _ => (Size::Wrap, Size::Wrap),
        };
        // Padding starts at 8, except pictures, web pages and videos (0).
        let padding = if matches!(kind, WKind::ImageView | WKind::WebView | WKind::VideoView) { 0 } else { 8 };
        Widget {
            id,
            kind,
            parent,
            width,
            height,
            padding,
            margin: 0,
            gravity: Gravity::None,
            vgravity: VGravity::None,
            bg: 0x00FF_FFFF,
            text,
            text_size: 12.0,
            text_color: 0xFF00_0000,
            bold: false,
            hint: if kind == WKind::EditText { "Edit Text".into() } else { String::new() },
            hint_color: 0xFF60_7D8B,
            checked: false,
            progress: if kind == WKind::ProgressBar || kind == WKind::SeekBar { 0 } else { 0 },
            max: 100,
            enabled: true,
            visible: true,
            weight: 0.0,
            password: false,
            input: InputKind::Text,
            radius: 0,
            elevation: 0,
            border: 0,
            border_color: default_border_color(),
            image: String::new(),
            crop: false,
            font: String::new(),
            video: String::new(),
            autoplay: false,
            looping: false,
            controls: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VarType {
    Bool,
    Num,
    Str,
    Map,
}

impl VarType {
    pub fn label(self) -> &'static str {
        match self {
            VarType::Bool => "Boolean",
            VarType::Num => "Number",
            VarType::Str => "String",
            VarType::Map => "Map",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Var {
    pub name: String,
    pub ty: VarType,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListType {
    Num,
    Str,
    Map,
}

impl ListType {
    pub fn label(self) -> &'static str {
        match self {
            ListType::Num => "List Number",
            ListType::Str => "List String",
            ListType::Map => "List Map",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ListVar {
    pub name: String,
    pub ty: ListType,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CompKind {
    Intent,
    SharedPref,
    Timer,
    Dialog,
    Vibrator,
    Calendar,
    /// Web requests (HTTP), e.g. to Supabase.
    Web,
    /// Plays the project's sounds.
    Sound,
    /// Takes photos and records videos with the phone's camera.
    Camera,
}

impl CompKind {
    pub const ALL: [CompKind; 9] = [CompKind::Intent, CompKind::SharedPref, CompKind::Timer, CompKind::Dialog, CompKind::Vibrator, CompKind::Calendar, CompKind::Web, CompKind::Sound, CompKind::Camera];
    pub fn label(self) -> &'static str {
        match self {
            CompKind::Intent => "Navigation",
            CompKind::SharedPref => "Saved data",
            CompKind::Timer => "Timer",
            CompKind::Dialog => "Pop-up",
            CompKind::Vibrator => "Vibration",
            CompKind::Calendar => "Date & time",
            CompKind::Web => "Web request",
            CompKind::Sound => "Sound player",
            CompKind::Camera => "Camera",
        }
    }
    /// Short descriptions of each component.
    pub fn description(self) -> &'static str {
        match self {
            CompKind::Intent => "Opens another screen of your app",
            CompKind::SharedPref => "Saves small pieces of data on the device",
            CompKind::Timer => "Runs blocks after a delay or repeatedly",
            CompKind::Dialog => "Shows a pop-up message with buttons",
            CompKind::Vibrator => "Vibrates the device",
            CompKind::Calendar => "Works with dates and times",
            CompKind::Web => "Talks to websites and online services, like Supabase",
            CompKind::Sound => "Plays the sounds you add to the project",
            CompKind::Camera => "Takes photos and records videos",
        }
    }
    pub fn default_name(self) -> &'static str {
        match self {
            CompKind::Intent => "i",
            CompKind::SharedPref => "data",
            CompKind::Timer => "t",
            CompKind::Dialog => "d",
            CompKind::Vibrator => "vib",
            CompKind::Calendar => "cal",
            CompKind::Web => "web",
            CompKind::Sound => "player",
            CompKind::Camera => "cam",
        }
    }
}

/// A Supabase project the app talks to: its address and public key.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SupabaseLink {
    pub url: String,
    pub key: String,
}

impl SupabaseLink {
    /// "https://abc.supabase.co", without a trailing slash.
    pub fn base(&self) -> String {
        self.url.trim().trim_end_matches('/').to_string()
    }
    pub fn is_set(&self) -> bool {
        !self.url.trim().is_empty() && !self.key.trim().is_empty()
    }
    /// What looks wrong, said plainly; None when it can be saved.
    pub fn problem(&self) -> Option<&'static str> {
        let u = self.url.trim();
        let k = self.key.trim();
        if !u.starts_with("https://") {
            Some("The project URL starts with https://")
        } else if k.is_empty() {
            Some("Paste the public key too")
        } else if k.starts_with("sb_secret_") || k.contains("service_role") {
            Some("That is the secret key: anyone with the app could read it. Use the anon or publishable key")
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Component {
    pub name: String,
    pub kind: CompKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Arg {
    /// Literal text / number / drop-down choice.
    Text(String),
    /// A reporter block plugged into the slot.
    Block(Box<Block>),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Block {
    pub id: u64,
    pub op: String,
    pub args: Vec<Arg>,
    #[serde(default)]
    pub sub1: Vec<Block>,
    #[serde(default)]
    pub sub2: Vec<Block>,
}

impl Block {
    pub fn max_id(&self) -> u64 {
        let mut m = self.id;
        for a in &self.args {
            if let Arg::Block(b) = a {
                m = m.max(b.max_id());
            }
        }
        for b in self.sub1.iter().chain(self.sub2.iter()) {
            m = m.max(b.max_id());
        }
        m
    }
    /// Deep copy with fresh ids.
    pub fn fresh(&self) -> Block {
        Block {
            id: new_id(),
            op: self.op.clone(),
            args: self
                .args
                .iter()
                .map(|a| match a {
                    Arg::Text(t) => Arg::Text(t.clone()),
                    Arg::Block(b) => Arg::Block(Box::new(b.fresh())),
                })
                .collect(),
            sub1: self.sub1.iter().map(|b| b.fresh()).collect(),
            sub2: self.sub2.iter().map(|b| b.fresh()).collect(),
        }
    }
    pub fn contains_id(&self, id: u64) -> bool {
        if self.id == id {
            return true;
        }
        self.args.iter().any(|a| matches!(a, Arg::Block(b) if b.contains_id(id)))
            || self.sub1.iter().any(|b| b.contains_id(id))
            || self.sub2.iter().any(|b| b.contains_id(id))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LooseStack {
    pub x: f32,
    pub y: f32,
    pub blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    /// Widget id, component name, "" for the activity, or "moreBlock".
    pub target: String,
    pub name: String,
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub loose: Vec<LooseStack>,
    /// A more block's inputs, in order (text, number or true/false).
    #[serde(default)]
    pub params: Vec<Var>,
}

impl Event {
    pub fn title(&self) -> String {
        if self.target == "moreBlock" {
            let mut t = format!("define {}", self.name);
            for p in &self.params {
                t.push_str(&format!(" ({})", p.name));
            }
            t
        } else if self.target.is_empty() {
            match self.name.as_str() {
                "onCreate" => "When the screen opens".into(),
                "onBackPressed" => "When back is pressed".into(),
                n => n.into(),
            }
        } else {
            format!("When {} {}", self.target, event_words(&self.name))
        }
    }
    pub fn max_id(&self) -> u64 {
        let mut m = 0;
        for b in &self.blocks {
            m = m.max(b.max_id());
        }
        for l in &self.loose {
            for b in &l.blocks {
                m = m.max(b.max_id());
            }
        }
        m
    }
}

/// Plain words for an event name ("onClick" → "is clicked").
pub fn event_words(name: &str) -> &str {
    match name {
        "onClick" => "is clicked",
        "onLongClick" => "is held down",
        "onCheckedChange" => "is checked or unchecked",
        "onTextChanged" => "text changes",
        "onProgressChanged" => "value changes",
        "onItemClicked" => "item is tapped",
        "onItemSelected" => "item is picked",
        "onCompletion" => "video ends",
        "onTimer" => "time is up",
        "onPositive" => "OK is pressed",
        "onNegative" => "Cancel is pressed",
        n => n,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Screen {
    /// Layout name, e.g. "main" → main.xml / MainActivity.kt.
    pub name: String,
    pub widgets: Vec<Widget>,
    pub vars: Vec<Var>,
    pub lists: Vec<ListVar>,
    pub components: Vec<Component>,
    pub events: Vec<Event>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub app_bar: AppBar,
    /// Shown as a bottom sheet over the screen that opens it (never the
    /// first screen, which the app starts on).
    #[serde(default)]
    pub sheet: Sheet,
}

/// How a screen opens: a whole page, or a bottom sheet of half or nearly
/// all of the phone's height.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Sheet {
    #[default]
    No,
    Half,
    Full,
}

impl Sheet {
    /// The part of the phone's height the sheet takes (iPhone: the medium
    /// and large detents).
    pub fn height(self) -> f32 {
        match self {
            Sheet::No => 1.0,
            Sheet::Half => 0.5,
            Sheet::Full => 0.92,
        }
    }
}

/// The room above a sheet's content, where its handle sits (dp / pt).
pub const SHEET_TOP: f32 = 20.0;

/// How a screen's app bar looks. Zero colours mean the phone's own: the
/// theme colour with a white title on Android, the system bar on iPhone.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct AppBar {
    /// No app bar: the screen starts right under the status bar.
    pub hidden: bool,
    pub color: u32,
    /// Zero: white or black, whichever reads on the bar.
    pub title_color: u32,
    /// No back arrow, even on a screen opened from another.
    pub no_back: bool,
    /// Icon buttons on the right of the bar; each has its own onClick event.
    pub actions: Vec<BarAction>,
}

/// One icon button in the app bar. `id` names it in events, like a widget's.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct BarAction {
    pub id: String,
    /// A name from BAR_ICONS.
    pub icon: String,
    /// Read aloud by screen readers; Android shows it on a long press.
    pub label: String,
}

/// The app bar's icons: name, Lucide character (the preview, and Android's
/// icon drawn from it), SF Symbol (iPhone).
pub const BAR_ICONS: &[(&str, char, &str)] = &[
    ("search", '\u{e155}', "magnifyingglass"),
    ("add", '\u{e141}', "plus"),
    ("edit", '\u{e1f9}', "pencil"),
    ("delete", '\u{e18e}', "trash"),
    ("share", '\u{e159}', "square.and.arrow.up"),
    ("done", '\u{e070}', "checkmark"),
    ("close", '\u{e1b2}', "xmark"),
    ("refresh", '\u{e149}', "arrow.clockwise"),
    ("settings", '\u{e158}', "gearshape"),
    ("info", '\u{e0fe}', "info.circle"),
    ("help", '\u{e082}', "questionmark.circle"),
    ("more", '\u{e0bb}', "ellipsis"),
    ("menu", '\u{e119}', "line.3.horizontal"),
    ("filter", '\u{e0d5}', "line.3.horizontal.decrease"),
    ("favorite", '\u{e0f5}', "heart"),
    ("star", '\u{e17a}', "star"),
    ("bookmark", '\u{e064}', "bookmark"),
    ("notifications", '\u{e05d}', "bell"),
    ("person", '\u{e19f}', "person"),
    ("home", '\u{e0f8}', "house"),
    ("cart", '\u{e160}', "cart"),
    ("camera", '\u{e068}', "camera"),
    ("send", '\u{e156}', "paperplane"),
    ("mail", '\u{e113}', "envelope"),
    ("phone", '\u{e137}', "phone"),
    ("calendar", '\u{e067}', "calendar"),
    ("download", '\u{e0b6}', "square.and.arrow.down"),
    ("logout", '\u{e112}', "rectangle.portrait.and.arrow.right"),
];

/// A bar icon's Lucide character and SF Symbol; unknown names get "more".
pub fn bar_icon(name: &str) -> (char, &'static str) {
    let (_, c, sf) = BAR_ICONS.iter().find(|(n, _, _)| *n == name).unwrap_or(&BAR_ICONS[11]);
    (*c, sf)
}

/// Android's app bar and status bar colours when none is picked.
pub const ANDROID_BAR: u32 = 0xFF5B_5BD6;
pub const ANDROID_STATUS: u32 = 0xFF4A_4AC4;
/// iPhone's navigation bar, its title and its buttons, when none is picked.
pub const IOS_BAR: u32 = 0xFFF9_F9F9;
pub const IOS_INK: u32 = 0xFF11_1111;

/// True when black text reads better than white on this colour.
pub fn is_light(argb: u32) -> bool {
    let (r, g, b) = ((argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF);
    (r * 299 + g * 587 + b * 114) / 1000 > 140
}

impl AppBar {
    pub fn bar(&self, iphone: bool) -> u32 {
        match (self.color, iphone) {
            (0, true) => IOS_BAR,
            (0, false) => ANDROID_BAR,
            (c, _) => c | 0xFF00_0000,
        }
    }
    pub fn title(&self, iphone: bool) -> u32 {
        if self.title_color != 0 {
            return self.title_color | 0xFF00_0000;
        }
        if self.color == 0 {
            return if iphone { IOS_INK } else { 0xFFFF_FFFF };
        }
        if is_light(self.bar(iphone)) { 0xFF11_1111 } else { 0xFFFF_FFFF }
    }
    /// The back arrow's colour: the title's on Android; on iPhone the
    /// system's, dark or light by the bar.
    pub fn back(&self, iphone: bool) -> u32 {
        if !iphone {
            self.title(false)
        } else if is_light(self.bar(true)) {
            IOS_INK
        } else {
            0xFFFF_FFFF
        }
    }
    /// Android's status bar: a darker shade of the app bar.
    pub fn status(&self) -> u32 {
        if self.color == 0 {
            return ANDROID_STATUS;
        }
        // with no bar, the status bar takes the page's own colour
        if self.hidden {
            return self.bar(false);
        }
        let c = self.bar(false);
        let dim = |sh: u32| (((c >> sh) & 0xFF) * 85 / 100) << sh;
        0xFF00_0000 | dim(16) | dim(8) | dim(0)
    }
}

impl Screen {
    pub fn new(name: &str) -> Self {
        Screen {
            name: name.to_string(),
            widgets: vec![],
            vars: vec![],
            lists: vec![],
            components: vec![],
            events: vec![Event { target: String::new(), name: "onCreate".into(), blocks: vec![], loose: vec![], params: vec![] }],
            title: String::new(),
            app_bar: AppBar::default(),
            sheet: Sheet::No,
        }
    }
    pub fn activity_name(&self) -> String {
        activity_name(&self.name)
    }
    /// How the editor names the screen: "main" → "Main screen".
    pub fn display_name(&self) -> String {
        let words = self.name.replace('_', " ");
        let mut c = words.chars();
        match c.next() {
            Some(f) => format!("{}{} screen", f.to_ascii_uppercase(), c.as_str()),
            None => "Screen".into(),
        }
    }
    pub fn widget(&self, id: &str) -> Option<&Widget> {
        self.widgets.iter().find(|w| w.id == id)
    }
    pub fn widget_mut(&mut self, id: &str) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|w| w.id == id)
    }
    pub fn children(&self, parent: &str) -> Vec<usize> {
        self.widgets.iter().enumerate().filter(|(_, w)| w.parent == parent).map(|(i, _)| i).collect()
    }
    pub fn next_widget_id(&self, kind: WKind) -> String {
        let p = kind.id_prefix();
        let mut n = 1;
        loop {
            let id = format!("{p}{n}");
            if !self.widgets.iter().any(|w| w.id == id) {
                return id;
            }
            n += 1;
        }
    }
    pub fn is_descendant(&self, id: &str, ancestor: &str) -> bool {
        let mut cur = id.to_string();
        for _ in 0..200 {
            if cur == ancestor {
                return true;
            }
            match self.widget(&cur) {
                Some(w) => cur = w.parent.clone(),
                None => return false,
            }
        }
        false
    }
    /// Remove a widget and everything inside it; also drop its events.
    pub fn remove_widget(&mut self, id: &str) {
        let doomed: Vec<String> = self.widgets.iter().filter(|w| self.is_descendant(&w.id, id)).map(|w| w.id.clone()).collect();
        self.widgets.retain(|w| !doomed.contains(&w.id));
        self.events.retain(|e| !doomed.contains(&e.target));
    }
    pub fn name_taken(&self, name: &str) -> bool {
        self.widgets.iter().any(|w| w.id == name)
            || self.vars.iter().any(|v| v.name == name)
            || self.lists.iter().any(|l| l.name == name)
            || self.components.iter().any(|c| c.name == name)
            || self.app_bar.actions.iter().any(|a| a.id == name)
    }
    pub fn action(&self, id: &str) -> Option<&BarAction> {
        self.app_bar.actions.iter().find(|a| a.id == id)
    }
    /// A free id for a new app bar button: its icon's name + "Button", numbered.
    pub fn next_action_id(&self, icon: &str) -> String {
        let base = format!("{icon}Button");
        if !self.name_taken(&base) {
            return base;
        }
        (2..).map(|n| format!("{base}{n}")).find(|n| !self.name_taken(n)).unwrap()
    }
    pub fn max_block_id(&self) -> u64 {
        self.events.iter().map(|e| e.max_id()).max().unwrap_or(0)
    }
}

/// What a text field takes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    #[default]
    Text,
    /// Dots, with an eye to show what's typed.
    Password,
    Email,
    /// Whole numbers.
    Number,
    /// Numbers with a decimal point.
    Decimal,
    Phone,
    /// Several lines of text.
    Multiline,
}

impl InputKind {
    pub const ALL: [InputKind; 7] = [InputKind::Text, InputKind::Password, InputKind::Email, InputKind::Number, InputKind::Decimal, InputKind::Phone, InputKind::Multiline];

    pub fn label(self) -> &'static str {
        match self {
            InputKind::Text => "Text",
            InputKind::Password => "Password",
            InputKind::Email => "Email",
            InputKind::Number => "Number",
            InputKind::Decimal => "Decimal number",
            InputKind::Phone => "Phone number",
            InputKind::Multiline => "Several lines",
        }
    }

    /// Its name for AI assistants (MCP): text, password, email, …
    pub fn key(self) -> &'static str {
        match self {
            InputKind::Text => "text",
            InputKind::Password => "password",
            InputKind::Email => "email",
            InputKind::Number => "number",
            InputKind::Decimal => "decimal",
            InputKind::Phone => "phone",
            InputKind::Multiline => "multiline",
        }
    }

    pub fn from_key(k: &str) -> Option<InputKind> {
        InputKind::ALL.into_iter().find(|i| i.key() == k.trim().to_lowercase())
    }

    /// A letter the field's keyboard lets in: numbers and phone numbers
    /// have no letters.
    pub fn allows(self, c: char) -> bool {
        match self {
            InputKind::Number => c.is_ascii_digit() || c == '-',
            InputKind::Decimal => c.is_ascii_digit() || matches!(c, '-' | '.' | ','),
            InputKind::Phone => c.is_ascii_digit() || "+-() #*".contains(c),
            InputKind::Multiline => true,
            _ => c != '\n',
        }
    }
}

/// A screen's name becomes its layout file and its code's name: a small
/// letter first, then small letters, digits and _.
pub fn check_screen_name(n: &str) -> Result<(), String> {
    let ok = !n.is_empty() && n.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(())
    } else {
        Err("Start with a small letter; use small letters, digits and _".into())
    }
}

pub fn activity_name(layout: &str) -> String {
    let mut s = String::new();
    for part in layout.split('_') {
        let mut c = part.chars();
        if let Some(f) = c.next() {
            s.extend(f.to_uppercase());
            s.push_str(c.as_str());
        }
    }
    s.push_str("Activity");
    s
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: u32,
    pub name: String,
    pub app_name: String,
    pub package: String,
    pub ver_code: u32,
    pub ver_name: String,
    /// Icon tint.
    pub icon_color: u32,
    /// A Lucide symbol shown on the icon colour (see icons::APP_GLYPHS); empty: the app name's first letter.
    #[serde(default)]
    pub icon_glyph: String,
    /// A picture used as the whole icon (kept in Blockwise's icon folder).
    #[serde(default)]
    pub icon_image: Option<String>,
    pub screens: Vec<Screen>,
    #[serde(default)]
    pub modified: u64,
    /// The Supabase project the Supabase blocks talk to.
    #[serde(default)]
    pub supabase: Option<SupabaseLink>,
    /// Pictures, sounds, fonts and data files the app carries.
    #[serde(default)]
    pub files: Vec<ProjFile>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    Image,
    Sound,
    Font,
    /// Text the app reads: a word list, JSON, CSV...
    Data,
    Video,
}

impl FileKind {
    pub const ALL: [FileKind; 5] = [FileKind::Image, FileKind::Sound, FileKind::Video, FileKind::Font, FileKind::Data];
    pub fn label(self) -> &'static str {
        match self {
            FileKind::Image => "Pictures",
            FileKind::Sound => "Sounds",
            FileKind::Font => "Fonts",
            FileKind::Data => "Data files",
            FileKind::Video => "Videos",
        }
    }
    pub fn one(self) -> &'static str {
        match self {
            FileKind::Image => "picture",
            FileKind::Sound => "sound",
            FileKind::Font => "font",
            FileKind::Data => "data file",
            FileKind::Video => "video",
        }
    }
    pub fn exts(self) -> &'static [&'static str] {
        match self {
            FileKind::Image => &IMAGE_EXTS,
            FileKind::Sound => &SOUND_EXTS,
            FileKind::Font => &FONT_EXTS,
            FileKind::Data => &DATA_EXTS,
            FileKind::Video => &VIDEO_EXTS,
        }
    }
    /// The block drop-down that picks a file of this kind.
    pub fn menu(self) -> &'static str {
        match self {
            FileKind::Image => "image",
            FileKind::Sound => "sound",
            FileKind::Font => "font",
            FileKind::Data => "asset",
            FileKind::Video => "video",
        }
    }
    pub fn of_ext(ext: &str) -> Option<FileKind> {
        FileKind::ALL.into_iter().find(|k| k.exts().contains(&ext))
    }
}

/// A picture or sound the app carries. The file itself is kept in
/// Blockwise's files folder for the project; `name` is what blocks and the
/// phone's code call it (an Android resource name: small letters, digits, _).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProjFile {
    pub name: String,
    pub kind: FileKind,
    /// "png", "jpg", "mp3", "wav" or "m4a".
    pub ext: String,
}

impl ProjFile {
    pub fn file_name(&self) -> String {
        format!("{}.{}", self.name, self.ext)
    }
}

/// The picture and sound files a project can carry: what both phones read.
pub const IMAGE_EXTS: [&str; 3] = ["png", "jpg", "jpeg"];
pub const SOUND_EXTS: [&str; 3] = ["mp3", "wav", "m4a"];
pub const FONT_EXTS: [&str; 2] = ["ttf", "otf"];
/// MP4 (H.264 video, AAC sound) plays on every Android phone and iPhone.
pub const VIDEO_EXTS: [&str; 3] = ["mp4", "m4v", "mov"];
pub const DATA_EXTS: [&str; 6] = ["txt", "json", "csv", "xml", "html", "md"];
/// A video's length in ms, read from its MP4/MOV header (the movie header
/// box, "mvhd", inside "moov"); None if it can't be read.
pub fn video_duration_ms(path: &std::path::Path) -> Option<f64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    // walks the boxes between `start` and `end`, looking for `want`
    fn find(f: &mut std::fs::File, mut at: u64, end: u64, want: &[u8; 4]) -> Option<(u64, u64)> {
        while at + 8 <= end {
            f.seek(SeekFrom::Start(at)).ok()?;
            let mut h = [0u8; 8];
            f.read_exact(&mut h).ok()?;
            let mut size = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
            let mut body = at + 8;
            if size == 1 {
                let mut big = [0u8; 8];
                f.read_exact(&mut big).ok()?;
                size = u64::from_be_bytes(big);
                body += 8;
            } else if size == 0 {
                size = end - at;
            }
            if size < 8 {
                return None;
            }
            if &h[4..8] == want {
                return Some((body, at + size));
            }
            at += size;
        }
        None
    }
    let (moov, moov_end) = find(&mut f, 0, len, b"moov")?;
    let (mvhd, _) = find(&mut f, moov, moov_end, b"mvhd")?;
    f.seek(SeekFrom::Start(mvhd)).ok()?;
    let mut v = [0u8; 4];
    f.read_exact(&mut v).ok()?;
    let (scale, dur) = if v[0] == 1 {
        let mut b = [0u8; 28];
        f.read_exact(&mut b).ok()?;
        (u32::from_be_bytes(b[16..20].try_into().ok()?) as f64, u64::from_be_bytes(b[20..28].try_into().ok()?) as f64)
    } else {
        let mut b = [0u8; 16];
        f.read_exact(&mut b).ok()?;
        (u32::from_be_bytes(b[8..12].try_into().ok()?) as f64, u32::from_be_bytes(b[12..16].try_into().ok()?) as f64)
    };
    (scale > 0.0).then(|| (dur * 1000.0 / scale).round())
}

/// "1:05" for 65 000 ms.
pub fn clock(ms: f64) -> String {
    let s = (ms / 1000.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A font file the preview can draw with (and the phones can likely read).
pub fn font_ok(bytes: &[u8]) -> bool {
    ab_glyph::FontRef::try_from_slice(bytes).is_ok()
}

/// Every kind of file the Assets page takes.
pub fn asset_exts() -> Vec<&'static str> {
    FileKind::ALL.iter().flat_map(|k| k.exts().iter().copied()).collect()
}

/// Where every project's assets are kept. Tests keep theirs in a temporary
/// folder, never beside the real projects (BLOCKWISE_FILES_ROOT can say).
pub fn files_root() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("BLOCKWISE_FILES_ROOT") {
        return d.into();
    }
    if cfg!(test) {
        return std::env::temp_dir().join("blockwise-test-files");
    }
    real_files_root()
}

/// Where Blockwise keeps the real projects' assets.
pub fn real_files_root() -> std::path::PathBuf {
    Store::path().parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir).join("files")
}

/// A file name made into a name the phones' code accepts: "My Cat!.png" → "my_cat".
pub fn file_base_name(stem: &str) -> String {
    let mut out = String::new();
    for c in stem.to_lowercase().chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '_' };
        if c == '_' && (out.is_empty() || out.ends_with('_')) {
            continue;
        }
        out.push(c);
    }
    let mut out = out.trim_end_matches('_').to_string();
    out.truncate(40);
    if !out.starts_with(|c: char| c.is_ascii_lowercase()) {
        out = format!("file_{out}").trim_end_matches('_').to_string();
    }
    // words Java, Kotlin or Swift keep for themselves can't name a resource
    const KEPT: &[&str] = &["abstract", "as", "break", "case", "catch", "class", "continue", "default", "do", "else", "enum", "false", "final", "for", "fun", "func", "if", "import", "in", "int", "is", "let", "new", "null", "object", "package", "private", "public", "return", "static", "super", "switch", "this", "throw", "true", "try", "val", "var", "void", "when", "while"];
    if KEPT.contains(&out.as_str()) {
        out.push_str("_1");
    }
    out
}

impl Screen {
    /// The colour of the page itself: its first layout's background, when
    /// that layout fills the screen and isn't see-through.
    pub fn page_color(&self) -> Option<u32> {
        let w = self.widgets.iter().find(|w| w.parent == "root")?;
        (w.height == Size::Match && w.bg >> 24 == 0xFF).then_some(w.bg)
    }
    /// The app bar as the phones show it: a screen without a bar and with no
    /// colour picked shows its page's colour behind the status bar.
    pub fn look(&self) -> AppBar {
        let mut b = self.app_bar.clone();
        if b.hidden && b.color == 0 {
            if let Some(c) = self.page_color() {
                b.color = c;
            }
        }
        b
    }
}

impl Project {
    /// How a screen opens; the first screen is always a page.
    pub fn sheet_of(&self, s: &Screen) -> Sheet {
        if self.screens.first().is_some_and(|f| f.name == s.name) { Sheet::No } else { s.sheet }
    }
    pub fn files_dir(&self) -> std::path::PathBuf {
        files_root().join(self.id.to_string())
    }
    pub fn file_path(&self, f: &ProjFile) -> std::path::PathBuf {
        self.files_dir().join(f.file_name())
    }
    pub fn file(&self, name: &str) -> Option<&ProjFile> {
        self.files.iter().find(|f| f.name == name)
    }
    pub fn file_names(&self, kind: FileKind) -> Vec<String> {
        self.files.iter().filter(|f| f.kind == kind).map(|f| f.name.clone()).collect()
    }
    /// Copies a picture, sound, font or data file into the project. Returns the name it got.
    pub fn add_file(&mut self, src: &std::path::Path) -> Result<String, String> {
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let Some(kind) = FileKind::of_ext(&ext) else {
            return Err(format!(
                "{} can't be added. Pictures can be PNG or JPEG; sounds MP3, WAV or M4A; videos MP4, M4V or MOV; fonts TTF or OTF; data files TXT, JSON, CSV, XML, HTML or MD",
                src.file_name().and_then(|n| n.to_str()).unwrap_or("That file")
            ));
        };
        if !src.is_file() {
            return Err(format!("{} isn't a file", src.display()));
        }
        if kind == FileKind::Font && !std::fs::read(src).is_ok_and(|b| font_ok(&b)) {
            return Err(format!("{} isn't a font file Blockwise can read", src.file_name().and_then(|n| n.to_str()).unwrap_or("That file")));
        }
        let ext = if ext == "jpeg" { "jpg".to_string() } else { ext };
        let name = self.free_file_name(&file_base_name(src.file_stem().and_then(|s| s.to_str()).unwrap_or("")), None);
        let f = ProjFile { name: name.clone(), kind, ext };
        std::fs::create_dir_all(self.files_dir()).map_err(|e| format!("Couldn't make the files folder: {e}"))?;
        std::fs::copy(src, self.file_path(&f)).map_err(|e| format!("Couldn't copy the file: {e}"))?;
        self.files.push(f);
        Ok(name)
    }
    /// Every file with where it is kept, those still there.
    pub fn file_paths(&self) -> Vec<(&ProjFile, std::path::PathBuf)> {
        self.files.iter().map(|f| (f, self.file_path(f))).filter(|(_, p)| p.exists()).collect()
    }

    /// `base`, or base_2, base_3... whichever no other file has. Names are
    /// unique across kinds: Android keeps pictures and sounds apart, but the
    /// iPhone app holds them all in one folder.
    fn free_file_name(&self, base: &str, except: Option<&str>) -> String {
        let taken = |n: &str| self.files.iter().any(|f| f.name == n && Some(n) != except);
        let mut name = base.to_string();
        let mut n = 2;
        while taken(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        name
    }

    /// Every place a file is used: "imageview1 on main", "a block in main › button1 clicked".
    /// Gives screen `si` a new name; the blocks that open it follow.
    pub fn rename_screen(&mut self, si: usize, wanted: &str) -> Result<(), String> {
        let n = wanted.trim().to_string();
        check_screen_name(&n)?;
        let Some(old) = self.screens.get(si).map(|s| s.activity_name()) else { return Err("There is no such screen".into()) };
        if self.screens.iter().enumerate().any(|(i, s)| i != si && (s.name == n || s.activity_name() == activity_name(&n))) {
            return Err("There is already a screen with that name".into());
        }
        let new = activity_name(&n);
        fn follow(b: &mut Block, old: &str, new: &str) {
            let opens = matches!(b.op.as_str(), "openScreen" | "intentSetScreen");
            for a in &mut b.args {
                match a {
                    Arg::Text(t) if opens && t == old => *t = new.to_string(),
                    Arg::Block(x) => follow(x, old, new),
                    _ => {}
                }
            }
            for x in b.sub1.iter_mut().chain(b.sub2.iter_mut()) {
                follow(x, old, new);
            }
        }
        for s in &mut self.screens {
            for e in &mut s.events {
                for b in e.blocks.iter_mut().chain(e.loose.iter_mut().flat_map(|l| l.blocks.iter_mut())) {
                    follow(b, &old, &new);
                }
            }
        }
        self.screens[si].name = n;
        Ok(())
    }

    /// The events on other screens with blocks that open screen `si` (they
    /// are left with nowhere to go when it's deleted), with how many blocks.
    pub fn screen_uses(&self, si: usize) -> Vec<String> {
        fn opens(b: &Block, target: &str) -> usize {
            let here = (matches!(b.op.as_str(), "openScreen" | "intentSetScreen") && b.args.iter().any(|a| matches!(a, Arg::Text(t) if t == target))) as usize;
            here + b.args.iter().map(|a| if let Arg::Block(x) = a { opens(x, target) } else { 0 }).sum::<usize>() + b.sub1.iter().chain(b.sub2.iter()).map(|x| opens(x, target)).sum::<usize>()
        }
        let Some(target) = self.screens.get(si).map(|s| s.activity_name()) else { return vec![] };
        let mut out = vec![];
        for (_, s) in self.screens.iter().enumerate().filter(|(i, _)| *i != si) {
            for e in &s.events {
                let n: usize = e.blocks.iter().chain(e.loose.iter().flat_map(|l| l.blocks.iter())).map(|b| opens(b, &target)).sum();
                if n > 0 {
                    out.push(format!("{} block{} in {} › {}", n, if n == 1 { "" } else { "s" }, s.display_name().to_lowercase(), e.title()));
                }
            }
        }
        out
    }

    pub fn file_uses(&self, name: &str) -> Vec<String> {
        let Some(f) = self.file(name) else { return vec![] };
        let mut out = vec![];
        for s in &self.screens {
            for w in &s.widgets {
                if (f.kind == FileKind::Image && w.image == name) || (f.kind == FileKind::Font && w.font == name) || (f.kind == FileKind::Video && w.video == name) {
                    out.push(format!("{} on {}", w.id, s.display_name().to_lowercase()));
                }
            }
            for e in &s.events {
                let mut n = 0;
                for b in e.blocks.iter().chain(e.loose.iter().flat_map(|l| l.blocks.iter())) {
                    n += file_refs(&mut b.clone(), f.kind, name, None);
                }
                if n > 0 {
                    out.push(format!("{} block{} in {} › {}", n, if n == 1 { "" } else { "s" }, s.display_name().to_lowercase(), e.title()));
                }
            }
        }
        out
    }

    /// Gives a file a new name, and every widget and block that used the
    /// old one follows. Returns the name it got (made fit for the phones' code).
    pub fn rename_file(&mut self, old: &str, wanted: &str) -> Result<String, String> {
        let Some(i) = self.files.iter().position(|f| f.name == old) else { return Err(format!("There is no file {old}")) };
        let base = file_base_name(wanted.trim());
        if base == old {
            return Ok(base);
        }
        if self.files.iter().any(|f| f.name == base) {
            return Err(format!("Another file is already called {base}"));
        }
        let kind = self.files[i].kind;
        let mut f = self.files[i].clone();
        f.name = base.clone();
        // the old file stays until the project is closed, so ⌘Z still finds it
        std::fs::copy(self.file_path(&self.files[i]), self.file_path(&f)).map_err(|e| format!("Couldn't rename the file: {e}"))?;
        self.files[i] = f;
        self.replace_file_uses(kind, old, &base);
        Ok(base)
    }

    /// Takes a file out of the project; widgets and blocks that used it are
    /// left without one. The file itself goes when the project is closed
    /// (until then ⌘Z brings it back).
    pub fn remove_file(&mut self, name: &str) {
        if let Some(i) = self.files.iter().position(|f| f.name == name) {
            let kind = self.files.remove(i).kind;
            self.replace_file_uses(kind, name, "");
        }
    }

    fn replace_file_uses(&mut self, kind: FileKind, old: &str, new: &str) {
        for s in &mut self.screens {
            for w in &mut s.widgets {
                if kind == FileKind::Image && w.image == old {
                    w.image = new.to_string();
                }
                if kind == FileKind::Font && w.font == old {
                    w.font = new.to_string();
                }
                if kind == FileKind::Video && w.video == old {
                    w.video = new.to_string();
                }
            }
            for e in &mut s.events {
                for b in e.blocks.iter_mut().chain(e.loose.iter_mut().flat_map(|l| l.blocks.iter_mut())) {
                    file_refs(b, kind, old, Some(new));
                }
            }
        }
    }

    /// Deletes files in the project's folder that it no longer lists.
    pub fn clean_files(&self) {
        let Ok(rd) = std::fs::read_dir(self.files_dir()) else { return };
        let keep: std::collections::HashSet<String> = self.files.iter().map(|f| f.file_name()).collect();
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !keep.contains(&n) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Counts (and with `to`, replaces) a file's name in the drop-downs of a
/// block and everything inside it.
fn file_refs(b: &mut Block, kind: FileKind, name: &str, to: Option<&str>) -> usize {
    let mut n = 0;
    if let Some(d) = crate::blocks::def(&b.op) {
        for t in crate::blocks::parse_spec(d.spec) {
            if let crate::blocks::Token::Param(i, crate::blocks::Param::Menu(m)) = t {
                if m == kind.menu() {
                    if let Some(Arg::Text(v)) = b.args.get_mut(i) {
                        if v == name {
                            n += 1;
                            if let Some(to) = to {
                                *v = to.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    for a in &mut b.args {
        if let Arg::Block(c) = a {
            n += file_refs(c, kind, name, to);
        }
    }
    for c in b.sub1.iter_mut().chain(b.sub2.iter_mut()) {
        n += file_refs(c, kind, name, to);
    }
    n
}

impl Project {
    pub fn new(id: u32, name: &str, app_name: &str, package: &str) -> Self {
        Project {
            id,
            name: name.into(),
            app_name: app_name.into(),
            package: package.into(),
            ver_code: 1,
            ver_name: "1.0".into(),
            icon_color: 0xFF5B_5BD6,
            icon_glyph: String::new(),
            icon_image: None,
            screens: vec![Screen::new("main")],
            modified: now_secs(),
            supabase: None,
            files: vec![],
        }
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Store {
    pub projects: Vec<Project>,
    pub next_project: u32,
    /// Blockwise's own dark theme.
    #[serde(default)]
    pub dark: bool,
    /// The settings page.
    #[serde(default)]
    pub settings: Settings,
}

/// What the settings page changes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Kept so older saved settings still load; always 1.0 now (the size control was removed).
    pub ui_scale: f32,
    /// "iphone" or "android": the phone the designer and the pretend phone show.
    pub preview: String,
    /// Code page text size when a project opens.
    pub code_size: u8,
    /// Ask before deleting a project.
    pub confirm_delete: bool,
    /// Open the last project again when Blockwise starts.
    pub reopen_last: bool,
    /// The project open when Blockwise last closed.
    pub last_project: Option<u32>,
    /// "Automatically save" (off by default): on,
    /// leaving a project keeps the changes without asking; off, leaving a
    /// changed project asks Don't save / Save and leave / Cancel.
    pub auto_save: bool,
    /// The folder exports go in (a folder per project is made inside it);
    /// None: ~/Blockwise.
    pub export_dir: Option<String>,
    /// What the last export wrote.
    pub export_kotlin: bool,
    pub export_swift: bool,
    /// Let AI assistants (MCP clients) work on the projects while Blockwise
    /// is open; off by default.
    pub mcp: bool,
    /// The local port the MCP server listens on.
    pub mcp_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { ui_scale: 1.0, preview: "iphone".into(), code_size: 14, confirm_delete: true, reopen_last: false, last_project: None, auto_save: false, export_dir: None, export_kotlin: true, export_swift: true, mcp: false, mcp_port: crate::mcp::DEFAULT_PORT }
    }
}

/// A project's content as text, for telling whether it changed since it was
/// opened (the time it was last edited doesn't count).
pub fn project_snapshot(p: &Project) -> String {
    let mut c = p.clone();
    c.modified = 0;
    serde_json::to_string(&c).unwrap_or_default()
}

/// The extension of Blockwise project files.
pub const BW_EXT: &str = "bw";

/// The "Back up" project action: a .bw file holding the whole project and
/// its files, in Documents/Blockwise backups. Returns where it went.
pub fn backup(p: &Project) -> std::io::Result<std::path::PathBuf> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (folder, file) = backup_name(p, stamp);
    let dir = dirs::document_dir().unwrap_or_else(std::env::temp_dir).join("Blockwise backups").join(folder);
    std::fs::create_dir_all(&dir)?;
    // when the file is there already, a number keeps both
    let mut path = dir.join(format!("{file}.{BW_EXT}"));
    let mut k = 2;
    while path.exists() {
        path = dir.join(format!("{file} {k}.{BW_EXT}"));
        k += 1;
    }
    std::fs::write(&path, bw_bytes(p)?)?;
    Ok(path)
}

/// A .bw project file: a zip holding project.json and the project's files in
/// assets/, the same layout as an example's folder in the repository.
pub fn bw_bytes(p: &Project) -> std::io::Result<Vec<u8>> {
    let mut entries = vec![("project.json".to_string(), serde_json::to_vec_pretty(p).map_err(std::io::Error::other)?)];
    for (f, path) in p.file_paths() {
        entries.push((format!("assets/{}", f.file_name()), std::fs::read(path)?));
    }
    crate::archive::write(&entries)
}

/// A project read from a file, ready for `Store::add_copy`.
pub struct Imported {
    pub project: Project,
    /// (file name, bytes) of each of its files that was found.
    pub files: Vec<(String, Vec<u8>)>,
    /// Files the project lists that weren't found; they're left out.
    pub missing: Vec<String>,
}

/// Reads a project to import: a .bw file (or any zip holding a
/// project.json and its assets folder, such as a zipped example folder), or a
/// bare project.json with its files in an `assets` folder beside it.
pub fn read_project_file(path: &std::path::Path) -> Result<Imported, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read the file: {e}"))?;
    read_project_bytes(&bytes, path.parent())
}

/// `read_project_file` for bytes in hand: a .bw file's, or a project.json's
/// whose files are looked for in `beside` (and its `assets` folder).
pub fn read_project_bytes(bytes: &[u8], beside: Option<&std::path::Path>) -> Result<Imported, String> {
    let not_one = || "That file isn't a Blockwise project.".to_string();
    let zipped = crate::archive::is_zip(bytes);
    // project.json, and for a zip the files of its assets folder
    let (json, inside): (Vec<u8>, Vec<(String, Vec<u8>)>) = if zipped {
        let mut entries = crate::archive::read(bytes)?;
        // at the top, or inside one folder
        let i = (0..entries.len()).filter(|&i| entries[i].0 == "project.json" || entries[i].0.ends_with("/project.json")).min_by_key(|&i| entries[i].0.len()).ok_or_else(not_one)?;
        let (name, json) = entries.swap_remove(i);
        let assets = format!("{}assets/", name.strip_suffix("project.json").unwrap_or_default());
        (json, entries.into_iter().filter_map(|(n, b)| Some((n.strip_prefix(&assets)?.to_string(), b))).collect())
    } else {
        (bytes.to_vec(), vec![])
    };
    let v: serde_json::Value = serde_json::from_slice(&json).map_err(|_| not_one())?;
    if v.get("screens").is_none() {
        return Err(not_one());
    }
    let mut project: Project = serde_json::from_value(v).map_err(|e| format!("The project doesn't load: {e}"))?;
    let (mut files, mut missing) = (vec![], vec![]);
    for f in &project.files {
        let n = f.file_name();
        let found = if !safe_file_name(&n) {
            None
        } else if zipped {
            inside.iter().find(|(m, _)| *m == n).map(|(_, b)| b.clone())
        } else {
            beside.and_then(|dir| [dir.join("assets").join(&n), dir.join(&n)].into_iter().find_map(|p| std::fs::read(p).ok()))
        };
        match found {
            Some(b) => files.push((n, b)),
            None => missing.push(n),
        }
    }
    project.files.retain(|f| !missing.contains(&f.file_name()));
    Ok(Imported { project, files, missing })
}

/// A file name that stays inside the folder it's written to.
fn safe_file_name(n: &str) -> bool {
    !n.is_empty() && !n.contains(['/', '\\', ':']) && n != "." && n != ".."
}

impl Store {
    /// Adds a project made elsewhere (an example, an imported file) with its
    /// files. It gets a new id, and a number after its name when the name is
    /// taken. Returns the new id.
    pub fn add_copy<N: AsRef<str>, B: AsRef<[u8]>>(&mut self, mut p: Project, files: impl IntoIterator<Item = (N, B)>) -> Result<u32, String> {
        p.id = self.next_project;
        let base = p.name.clone();
        let mut k = 1;
        while self.projects.iter().any(|q| q.name == p.name) {
            k += 1;
            p.name = format!("{base} {k}");
        }
        if k > 1 {
            p.app_name = format!("{} {k}", p.app_name);
        }
        p.modified = now_secs();
        // blocks made from now on must not take the project's block ids
        bump_ids(p.screens.iter().map(|s| s.max_block_id()).max().unwrap_or(0));
        let dir = p.files_dir();
        let _ = std::fs::remove_dir_all(&dir);
        let mut made = false;
        for (name, bytes) in files {
            let name = name.as_ref();
            if !safe_file_name(name) {
                continue;
            }
            if !made {
                std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the files folder: {e}"))?;
                made = true;
            }
            std::fs::write(dir.join(name), bytes.as_ref()).map_err(|e| format!("Couldn't copy {name}: {e}"))?;
        }
        self.next_project += 1;
        let id = p.id;
        self.projects.push(p);
        Ok(id)
    }
}

/// Backup naming: a folder
/// named after the project (with "_d" and path separators taken out), and a
/// file "<name> v<version> (<app id>, <version code>) <yyyy-M-dd'T'HHmmss>".
/// Blockwise writes the time in UTC.
pub fn backup_name(p: &Project, unix_secs: u64) -> (String, String) {
    let base = p.name.replace("_d", "").replace(['/', '\\'], "");
    let t = unix_secs as i64;
    let (y, m, d) = crate::runtime::civil(t.div_euclid(86400));
    let s = t.rem_euclid(86400);
    let when = format!("{y:04}-{m}-{d:02}T{:02}{:02}{:02}", s / 3600, (s / 60) % 60, s % 60);
    let file = format!("{base} v{} ({}, {}) {when}", p.ver_name, p.package, p.ver_code);
    (base, file.replace(['/', '\\', ':'], "_"))
}

/// Project order: by
/// project id, highest (newest) first.
pub fn newest_first(a: &Project, b: &Project) -> std::cmp::Ordering {
    b.id.cmp(&a.id)
}

/// What leaving a project does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Leave {
    /// Nothing changed, or automatic saving is on: leave and keep everything.
    Go,
    /// Ask whether to keep the changes.
    Ask,
}

pub fn leave_rule(auto_save: bool, changed: bool) -> Leave {
    if auto_save || !changed {
        Leave::Go
    } else {
        Leave::Ask
    }
}

impl Store {
    pub fn path() -> std::path::PathBuf {
        // BLOCKWISE_DATA_DIR: another folder for the projects, for trying things out
        if let Some(d) = std::env::var_os("BLOCKWISE_DATA_DIR") {
            return std::path::PathBuf::from(d).join("projects.json");
        }
        let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
        base.join("Blockwise").join("projects.json")
    }
    pub fn load() -> Store {
        let p = Self::path();
        let mut s: Store = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if s.next_project < 601 {
            s.next_project = 601;
        }
        let mut m = 0;
        for pr in &s.projects {
            for sc in &pr.screens {
                m = m.max(sc.max_block_id());
            }
        }
        bump_ids(m);
        s
    }
    pub fn save(&self) {
        let p = Self::path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn text_fields_take_what_their_input_says() {
        use super::{InputKind, WKind, Widget};
        // a field from before input types, with dots on, is a password field
        let old: Widget = serde_json::from_str(&{
            let mut v = serde_json::to_value(Widget::new("e".into(), WKind::EditText, "linear1".into())).unwrap();
            v.as_object_mut().unwrap().remove("input");
            v["password"] = true.into();
            v.to_string()
        })
        .unwrap();
        assert_eq!(old.input_kind(), InputKind::Password);
        for k in InputKind::ALL {
            assert_eq!(InputKind::from_key(k.key()), Some(k));
        }
        assert!(InputKind::Number.allows('7') && !InputKind::Number.allows('a'));
        assert!(InputKind::Decimal.allows('.') && !InputKind::Decimal.allows('x'));
        assert!(InputKind::Phone.allows('+') && !InputKind::Phone.allows('b'));
        assert!(InputKind::Multiline.allows('\n') && !InputKind::Text.allows('\n'));
    }

    #[test]
    fn gravity_numbers_round_trip() {
        use super::{gravity_flags, gravity_from_flags, gravity_xml, Gravity, VGravity};
        for h in [Gravity::None, Gravity::Left, Gravity::Center, Gravity::Right] {
            for v in [VGravity::None, VGravity::Top, VGravity::Center, VGravity::Bottom] {
                assert_eq!(gravity_from_flags(gravity_flags(h, v)), (h, v));
            }
        }
        // Android's own numbers: center = 0x11, bottom|right = 0x55
        assert_eq!(gravity_flags(Gravity::Center, VGravity::Center), 0x11);
        assert_eq!(gravity_flags(Gravity::Right, VGravity::Bottom), 0x55);
        assert_eq!(gravity_xml(Gravity::Center, VGravity::Center).as_deref(), Some("center"));
        assert_eq!(gravity_xml(Gravity::Right, VGravity::Bottom).as_deref(), Some("right|bottom"));
        assert_eq!(gravity_xml(Gravity::None, VGravity::Top).as_deref(), Some("top"));
        assert_eq!(gravity_xml(Gravity::None, VGravity::None), None);
    }

    use super::*;

    #[test]
    fn leaving_a_project() {
        // automatic saving is off by default, and leaving a
        // changed project then asks first
        assert!(!Settings::default().auto_save);
        assert_eq!(leave_rule(false, true), Leave::Ask);
        assert_eq!(leave_rule(false, false), Leave::Go);
        assert_eq!(leave_rule(true, true), Leave::Go);
    }

    #[test]
    fn import_bw_files_and_project_files() {
        let tmp = std::env::temp_dir().join(format!("bw-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut st = Store { next_project: 950, ..Default::default() };

        // a .bw file brings back the project and its files
        let ex = crate::examples::EXAMPLES.iter().find(|e| e.key == "brain-quiz").unwrap();
        let id = crate::examples::add(&mut st, ex).unwrap();
        let p = st.projects.iter().find(|p| p.id == id).unwrap().clone();
        assert_eq!(p.files.len(), 1);
        let bw = tmp.join("Brain Quiz.bw");
        std::fs::write(&bw, bw_bytes(&p).unwrap()).unwrap();
        let names: Vec<String> = crate::archive::read(&std::fs::read(&bw).unwrap()).unwrap().into_iter().map(|e| e.0).collect();
        assert_eq!(names, ["project.json", "assets/questions.json"]);
        let im = read_project_file(&bw).unwrap();
        assert!(im.missing.is_empty());
        let again = st.add_copy(im.project, im.files).unwrap();
        let q = st.projects.iter().find(|p| p.id == again).unwrap();
        assert_eq!(q.name, format!("{} 2", p.name));
        assert_eq!(std::fs::read(q.file_path(&q.files[0])).unwrap(), std::fs::read(p.file_path(&p.files[0])).unwrap());
        assert_eq!(project_snapshot(&Project { id: 0, name: String::new(), app_name: String::new(), ..q.clone() }), project_snapshot(&Project { id: 0, name: String::new(), app_name: String::new(), ..p.clone() }));

        // so does a zipped project folder: everything inside one folder
        let unpacked = crate::archive::read(ex.bw).unwrap();
        let entries: Vec<(String, Vec<u8>)> = unpacked.iter().map(|(n, b)| (format!("brain-quiz/{n}"), b.clone())).collect();
        std::fs::write(tmp.join("brain-quiz.zip"), crate::archive::write(&entries).unwrap()).unwrap();
        let im = read_project_file(&tmp.join("brain-quiz.zip")).unwrap();
        assert!(im.missing.is_empty() && im.files.len() == 1);

        // a bare project.json finds its files in assets/ beside it
        let repo = tmp.join("brain-quiz");
        for (n, b) in &unpacked {
            std::fs::create_dir_all(repo.join(n).parent().unwrap()).unwrap();
            std::fs::write(repo.join(n), b).unwrap();
        }
        let im = read_project_file(&repo.join("project.json")).unwrap();
        assert!(im.missing.is_empty(), "{:?}", im.missing);
        assert_eq!(im.files[0].0, "questions.json");

        // without them it still imports, leaving the files out
        let lone = tmp.join("lone.json");
        std::fs::copy(repo.join("project.json"), &lone).unwrap();
        let im = read_project_file(&lone).unwrap();
        assert_eq!(im.missing, vec!["questions.json".to_string()]);
        assert!(im.project.files.is_empty() && im.files.is_empty());

        // other files are refused
        std::fs::write(tmp.join("x.json"), r#"{"hello": 1}"#).unwrap();
        assert!(read_project_file(&tmp.join("x.json")).is_err());
        std::fs::write(tmp.join("y.bw"), "not a project").unwrap();
        assert!(read_project_file(&tmp.join("y.bw")).is_err());
        std::fs::write(tmp.join("z.bw"), crate::archive::write(&[("readme.txt".into(), b"hi".to_vec())]).unwrap()).unwrap();
        assert!(read_project_file(&tmp.join("z.bw")).is_err());

        // a file name can't reach outside the project's folder
        let mut evil = p.clone();
        evil.files[0].name = "../../evil".into();
        let entries = vec![("project.json".to_string(), serde_json::to_vec(&evil).unwrap()), ("assets/../../evil.json".to_string(), b"x".to_vec())];
        std::fs::write(tmp.join("evil.bw"), crate::archive::write(&entries).unwrap()).unwrap();
        let im = read_project_file(&tmp.join("evil.bw")).unwrap();
        assert!(im.files.is_empty());
        assert_eq!(im.missing, vec!["../../evil.json".to_string()]);

        for p in &st.projects {
            let _ = std::fs::remove_dir_all(p.files_dir());
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn backup_naming_and_order() {
        let mut p = crate::app::sample_project(601);
        p.name = "Counter_d".into();
        p.ver_name = "1.0".into();
        p.ver_code = 1;
        p.package = "com.my.counter".into();
        // 2026-03-05 07:08:09 UTC
        let (folder, file) = backup_name(&p, 1772694489);
        assert_eq!(folder, "Counter");
        assert_eq!(file, "Counter v1.0 (com.my.counter, 1) 2026-3-05T070809");
        let mut q = p.clone();
        q.id = p.id + 1;
        assert_eq!(newest_first(&q, &p), std::cmp::Ordering::Less);
    }
}
