//! The window: projects list, project editor (View / Event / Component), blocks editor,
//! source view and the pretend phone.

use crate::blocks::*;
use crate::designer::{self, DesignState};
use crate::logic::{self, LogicState};
use crate::model::*;
use crate::phone::{self, c32, Mode, PhoneOut, PRIMARY, PRIMARY_DARK};
use crate::runtime::Runtime;
use crate::icons::{self as ic, lbl};
use egui::*;
#[allow(unused_imports)]
use crate::model::{Event, Widget};

pub const INK: Color32 = Color32::from_rgb(0x1D, 0x21, 0x29);

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    View,
    Event,
    Component,
    /// The app's pictures, sounds, fonts and data files.
    Assets,
}

/// One file on the code page. Kotlin is the Android app, Swift the iPhone app.
#[derive(Clone, Copy, PartialEq)]
enum SrcTab {
    KtScreen,
    KtLayout,
    KtSettings,
    KtHelpers,
    SwScreen,
    SwApp,
    SwHelpers,
}

impl SrcTab {
    fn swift(self) -> bool {
        matches!(self, SrcTab::SwScreen | SrcTab::SwApp | SrcTab::SwHelpers)
    }
    /// The same kind of file in the other language.
    fn other_lang(self) -> SrcTab {
        match self {
            SrcTab::KtScreen | SrcTab::KtLayout => SrcTab::SwScreen,
            SrcTab::KtSettings => SrcTab::SwApp,
            SrcTab::KtHelpers => SrcTab::SwHelpers,
            SrcTab::SwScreen => SrcTab::KtScreen,
            SrcTab::SwApp => SrcTab::KtSettings,
            SrcTab::SwHelpers => SrcTab::KtHelpers,
        }
    }
}

struct ProjDialog {
    edit: Option<u32>,
    app_name: String,
    name: String,
    package: String,
    ver_code: u32,
    ver_name: String,
    color: u32,
    /// Lucide symbol name, or empty for the first letter.
    glyph: String,
    /// A picture used as the whole icon.
    image: Option<String>,
    error: String,
}

impl ProjDialog {
    fn face(&self) -> ic::Face {
        if let Some(i) = self.image.as_ref().map(std::path::PathBuf::from).filter(|p| p.exists()) {
            return ic::Face::Image(i);
        }
        match ic::app_glyph(&self.glyph) {
            Some(g) => ic::Face::Glyph(g),
            None => ic::Face::Letter(self.app_name.trim().chars().next().unwrap_or('A').to_uppercase().to_string()),
        }
    }
}

/// "Export source": which projects to write and where.
struct ExportDialog {
    pid: u32,
    kotlin: bool,
    swift: bool,
    /// The folder a folder named after the project is made in.
    dir: String,
    error: String,
}

/// The Blockwise logo (the bolt made of two puzzle pieces, from
/// brand/logo.svg at the top of the repository), loaded once as a texture.
fn brand_mark(ctx: &Context) -> Option<TextureHandle> {
    let key = Id::new("brand_mark");
    if let Some(t) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(key)) {
        return t;
    }
    let t = image::load_from_memory(include_bytes!("../assets/brand/logo-192.png")).ok().map(|img| {
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        ctx.load_texture("brand_mark", ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(key, t.clone()));
    t
}

/// "~/x" → the home folder's x.
fn expand_home(path: &str) -> std::path::PathBuf {
    let path = path.trim();
    match (path.strip_prefix("~/").or(if path == "~" { Some("") } else { None }), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => std::path::PathBuf::from(path),
    }
}

/// The home folder written as ~ again, for showing a path.
fn tilde(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|h| path.strip_prefix(&h).ok().map(|r| r.to_path_buf())) {
        Some(r) if r.as_os_str().is_empty() => "~".into(),
        Some(r) => format!("~/{}", r.display()),
        None => path.display().to_string(),
    }
}

struct Editor {
    pid: u32,
    screen: usize,
    tab: Tab,
    logic: Option<usize>,
    design: DesignState,
    logic_state: LogicState,
    source: Option<SrcTab>,
    run: Option<Runtime>,
    add_event: bool,
    /// What's typed in the add-event list's search box.
    event_search: String,
    add_comp: Option<(CompKind, String, String)>,
    add_screen: Option<(String, String)>,
    /// The template picked in the "Add a screen" window (None: blank).
    add_template: Option<&'static str>,
    notice: Option<(String, f64)>,
    /// Run page bottom tool area: 0 debugger, 1 app state, 2 device log.
    run_tab: u8,
    /// The run page's bottom tool area is open (▾ folds it to its tab row).
    tools_open: bool,
    /// The running app was paused on the last frame.
    pause_seen: bool,
    /// Pause at the start of every event when running.
    step_mode: bool,
    /// The app running on a real emulator or simulator, with its live log.
    device: Option<crate::device::DeviceRun>,
    device_shown: bool,
    /// The device run's phase on the last frame, to tell when a build fails.
    device_phase: Option<crate::device::Phase>,
    device_filter: [bool; 4],
    /// Picked in the "Run on device" menu; started on the next frame.
    device_start: Option<crate::device::Device>,
    /// Picked in the "Build" menu; started on the next frame.
    build_start: Option<crate::device::Device>,
    /// Code page text size (see viewer::CODE_SIZES).
    code_size: u8,
    /// The live log's wrap / fixed-width / size settings.
    log_view: crate::viewer::LogView,
    /// The project as it was when it was opened (see model::project_snapshot).
    opened: Option<String>,
    /// The "Leave project?" question is showing.
    quit_ask: bool,
    /// "Export source" was picked; the window opens on the next frame.
    want_export: bool,
    /// The menu asked to close the project (goes through the save question).
    close_req: bool,
    /// Undo history: the project as it was before each change (snapshots).
    undo: Vec<String>,
    redo: Vec<String>,
    /// The project as it last settled (what the next change is undone to).
    base: Option<String>,
    /// The phone ⌘R runs on and ⌘B builds for (the last one picked).
    last_device: crate::device::Device,
    /// Assets page: the file being renamed (its name, the new name typed).
    asset_rename: Option<(String, String)>,
    /// Assets page: the file whose delete is being asked about.
    asset_delete: Option<String>,
    /// The x beside a screen in the screen picker: asks before deleting it.
    screen_delete: Option<usize>,
    /// The pencil beside a screen in the screen picker: (screen, the name
    /// being typed, what's wrong with it).
    screen_rename: Option<(usize, String, String)>,
    /// Assets page: what went wrong adding or renaming.
    asset_msg: String,
    /// Assets page: "Add files…" was clicked (the dialog opens after the frame).
    asset_pick: bool,
    /// Assets page: the file shown or playing in the viewer over the page.
    asset_view: Option<crate::asset_view::AssetView>,
    /// The offline preview's real video and sound players.
    preview_media: crate::player::Preview,
    /// The debugger is docked under the editor (Debug, or a breakpoint
    /// was hit). While it's open the app keeps running on every tab, so
    /// breakpoints can be set in the blocks as it runs.
    debugging: bool,
}

impl Editor {
    fn new(pid: u32) -> Self {
        Editor {
            pid,
            screen: 0,
            tab: Tab::View,
            logic: None,
            design: DesignState::default(),
            logic_state: LogicState::default(),
            source: None,
            run: None,
            add_event: false,
            event_search: String::new(),
            add_comp: None,
            add_screen: None,
            add_template: None,
            notice: None,
            run_tab: 0,
            tools_open: true,
            pause_seen: false,
            step_mode: false,
            device: None,
            device_shown: false,
            device_phase: None,
            device_filter: [true, true, true, true],
            device_start: None,
            build_start: None,
            code_size: crate::viewer::default_code_size(),
            log_view: crate::viewer::LogView::default(),
            opened: None,
            quit_ask: false,
            want_export: false,
            close_req: false,
            undo: vec![],
            redo: vec![],
            base: None,
            last_device: crate::device::Device::Android,
            asset_rename: None,
            asset_delete: None,
            screen_delete: None,
            screen_rename: None,
            asset_msg: String::new(),
            asset_pick: false,
            asset_view: None,
            preview_media: crate::player::Preview::default(),
            debugging: false,
        }
    }
}

pub struct App {
    store: Store,
    editor: Option<Editor>,
    proj_dialog: Option<ProjDialog>,
    confirm_delete: Option<u32>,
    notice: Option<(String, f64)>,
    dirty: bool,
    last_save: f64,
    no_save: bool,
    shot: Option<String>,
    frames: u32,
    shot_sent: bool,
    /// The macOS menu bar (none in screenshots or on other systems).
    menu: Option<crate::menu::AppMenu>,
    /// A menu command waiting for the open project to close first.
    after_close: Option<crate::menu::Cmd>,
    /// The settings page is showing (⌘, opens and closes it).
    settings_open: bool,
    /// Projects page: search text, sort by name (else last edited), list view (else grid).
    search: String,
    /// 0 newest first, 1 last edited, 2 name.
    sort: u8,
    list_view: bool,
    export: Option<ExportDialog>,
    /// The MCP server for AI assistants, while it's switched on.
    mcp: Option<crate::mcp::Server>,
    /// Why it couldn't start: (port, error, when).
    mcp_err: Option<(u16, String, f64)>,
    /// A preview_screenshot waiting for the window's picture.
    mcp_shot: Option<McpShot>,
}

struct McpShot {
    reply: std::sync::mpsc::Sender<crate::mcp::Reply>,
    frames: u32,
    sent: bool,
    note: String,
}

const ICON_COLORS: [u32; 10] = [0xFF5B_5BD6, 0xFF5C_B722, 0xFFEE_7D16, 0xFF8A_55D7, 0xFFE1_A92A, 0xFF23_B9A9, 0xFFCC_5B22, 0xFF4A_6CD4, 0xFFE9_1E63, 0xFF26_2B33];

pub fn theme(ctx: &Context) {
    let dark = crate::theme::dark();
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = crate::theme::bg();
    v.window_fill = crate::theme::surface();
    v.extreme_bg_color = if dark { Color32::from_rgb(0x1A, 0x1D, 0x24) } else { Color32::WHITE };
    if dark {
        v.faint_bg_color = Color32::from_rgb(0x2B, 0x31, 0x3B);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x30, 0x36, 0x41);
        v.widgets.inactive.bg_fill = Color32::from_rgb(0x30, 0x36, 0x41);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x3A, 0x42, 0x4F);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x38, 0x3E, 0x49));
    }
    v.selection.bg_fill = if dark { Color32::from_rgb(0x34, 0x35, 0x7A) } else { Color32::from_rgb(0xDE, 0xE0, 0xFF) };
    v.selection.stroke = Stroke::new(1.0_f32, if dark { Color32::from_rgb(0xB4, 0xB5, 0xFA) } else { PRIMARY_DARK });
    v.hyperlink_color = PRIMARY;
    v.override_text_color = Some(crate::theme::ink());
    v.window_rounding = Rounding::same(8.0);
    v.widgets.inactive.rounding = Rounding::same(4.0);
    v.widgets.hovered.rounding = Rounding::same(4.0);
    v.widgets.active.rounding = Rounding::same(4.0);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.interaction.selectable_labels = false;
        s.spacing.button_padding = vec2(8.0, 4.0);
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Heading, FontId::proportional(19.0));
    });
}

fn t(s: &str) -> Arg {
    Arg::Text(s.into())
}
fn bk(b: Block) -> Arg {
    Arg::Block(Box::new(b))
}
fn blk(op: &str, args: Vec<Arg>) -> Block {
    Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] }
}

/// A small finished example so the first start isn't empty.
/// Adds a second screen, opened by a timer when the first one starts (for
/// the screen tests and screenshots).
/// A scrolling column filling the screen, holding a centred column taller
/// than the phone, and a dropdown after it (a test and screenshot scene).
pub fn scroll_column_screen(p: &mut Project) {
    let s = &mut p.screens[0];
    s.widgets.clear();
    s.events.retain(|e| e.target.is_empty());
    let mut put = |id: &str, kind: WKind, parent: &str, f: &dyn Fn(&mut Widget)| {
        let mut w = Widget::new(id.into(), kind, parent.into());
        f(&mut w);
        s.widgets.push(w);
    };
    put("vscroll2", WKind::ScrollV, "root", &|w| { w.width = Size::Match; w.height = Size::Match; });
    put("spinner1", WKind::Spinner, "root", &|w| w.width = Size::Match);
    put("linear1", WKind::LinearV, "vscroll2", &|w| { w.width = Size::Match; w.height = Size::Match; w.gravity = Gravity::Center; w.vgravity = VGravity::Center; });
    put("calendarview1", WKind::CalendarView, "linear1", &|w| { w.width = Size::Match; w.height = Size::Wrap; });
    put("textview1", WKind::TextView, "linear1", &|w| { w.text = "0".into(); w.text_size = 96.0; });
    put("seekbar1", WKind::SeekBar, "linear1", &|w| { w.width = Size::Match; w.padding = 30; });
    put("btn", WKind::Button, "linear1", &|w| w.text = "Click me".into());
    put("webview1", WKind::WebView, "linear1", &|w| { w.width = Size::Match; w.height = Size::Match; });
    put("videoview1", WKind::VideoView, "linear1", &|w| { w.width = Size::Match; w.height = Size::Px(200); });
}

pub fn add_second_screen(p: &mut Project) {
    let mut second = Screen::new("second");
    second.widgets.push(Widget::new("textview1".into(), WKind::TextView, "root".into()));
    let target = second.activity_name();
    p.screens.push(second);
    let blk = |op: &str, args: Vec<Arg>| Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
    let mut later = blk("timerAfter", vec![Arg::Text("t".into()), Arg::Text("100".into())]);
    later.sub1 = vec![blk("openScreen", vec![Arg::Text(target)])];
    p.screens[0].events[0].blocks.push(later);
}

pub fn sample_project(id: u32) -> Project {
    let mut p = Project::new(id, "Counter", "Tap Counter", "com.my.counter");
    let s = &mut p.screens[0];
    let mut root = Widget::new("linear1".into(), WKind::LinearV, "root".into());
    root.padding = 24;
    root.width = Size::Match;
    root.gravity = Gravity::Center;
    let mut big = Widget::new("textview1".into(), WKind::TextView, "linear1".into());
    big.text = "0".into();
    big.text_size = 48.0;
    big.bold = true;
    big.text_color = 0xFF5B_5BD6;
    let mut hint = Widget::new("textview2".into(), WKind::TextView, "linear1".into());
    hint.text = "Tap the button to count".into();
    hint.text_color = 0xFF60_7D8B;
    hint.text_size = 14.0;
    let row = Widget::new("linear2".into(), WKind::LinearH, "linear1".into());
    let mut add = Widget::new("button1".into(), WKind::Button, "linear2".into());
    add.text = "Add one".into();
    add.margin = 4;
    let mut reset = Widget::new("button2".into(), WKind::Button, "linear2".into());
    reset.text = "Reset".into();
    reset.margin = 4;
    let mut two = Widget::new("checkbox1".into(), WKind::CheckBox, "linear1".into());
    two.text = "Count by two".into();
    s.widgets = vec![root, big, hint, row, add, reset, two];
    s.vars.push(Var { name: "count".into(), ty: VarType::Num });
    s.components.push(Component { name: "d".into(), kind: CompKind::Dialog });
    let show_count = || blk("setText", vec![t("textview1"), bk(blk("toString", vec![bk(named_block("getVar", "count"))]))]);
    let mut check = blk("ifElse", vec![bk(blk("getChecked", vec![t("checkbox1")]))]);
    check.sub1 = vec![blk("setVarInt", vec![t("count"), bk(blk("+", vec![bk(named_block("getVar", "count")), t("2")]))])];
    check.sub2 = vec![blk("increaseInt", vec![t("count")])];
    s.events[0].blocks = vec![blk("setVarInt", vec![t("count"), t("0")]), blk("setText", vec![t("textview2"), t("Tap the button to count")])];
    s.events.push(Event { target: "button1".into(), name: "onClick".into(), blocks: vec![check, show_count()], loose: vec![], params: vec![] });
    let mut ok = blk("dialogOkButton", vec![t("d"), t("Reset")]);
    ok.sub1 = vec![blk("setVarInt", vec![t("count"), t("0")]), show_count(), blk("doToast", vec![t("Counter reset")])];
    s.events.push(Event {
        target: "button2".into(),
        name: "onClick".into(),
        blocks: vec![blk("dialogSetTitle", vec![t("d"), t("Start again?")]), blk("dialogSetMessage", vec![t("d"), t("The count goes back to zero.")]), ok, blk("dialogCancelButton", vec![t("d"), t("Cancel")]), blk("dialogShow", vec![t("d")])],
        loose: vec![],
        params: vec![],
    });
    p
}

/// The window's toolbar: a plain desktop bar with a hairline under it.
fn top_bar(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
    TopBottomPanel::top(ui.next_auto_id()).exact_height(48.0).frame(Frame::none().fill(crate::theme::surface()).inner_margin(Margin::symmetric(10.0, 0.0)).stroke(Stroke::new(1.0_f32, line))).show_inside(ui, |ui| {
        ui.visuals_mut().override_text_color = Some(crate::theme::ink());
        ui.horizontal_centered(|ui| add(ui));
    });
}

/// A filled blue button for the main action of a bar.
fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(PRIMARY).min_size(vec2(0.0, 30.0)).rounding(6.0))
}

/// A bordered button for the other actions of a bar.
fn plain_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(Button::new(RichText::new(text)).fill(crate::theme::surface()).stroke(Stroke::new(1.0_f32, crate::theme::gray(205))).min_size(vec2(0.0, 30.0)).rounding(6.0))
}

/// Relative time, like "5 min ago".
fn ago(then: u64) -> String {
    let d = now_secs().saturating_sub(then);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86399 => format!("{} h ago", d / 3600),
        86400..=604799 => format!("{} days ago", d / 86400),
        _ => format!("{} weeks ago", d / 604800),
    }
}

fn bar_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(Button::new(RichText::new(text).size(14.5)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).min_size(vec2(0.0, 30.0)).rounding(6.0))
}

fn valid_package(p: &str) -> bool {
    let parts: Vec<&str> = p.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|s| {
        let mut c = s.chars();
        matches!(c.next(), Some(f) if f.is_ascii_alphabetic()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

impl App {
    pub fn new(cc: &eframe::CreationContext, shot: Option<String>, open: Option<String>) -> Self {
        let mut store = if shot.is_some() { Store { projects: vec![], next_project: 601, dark: std::env::var("BLOCKWISE_DARK").is_ok_and(|v| v == "1"), settings: Settings { preview: std::env::var("BLOCKWISE_PREVIEW").unwrap_or_else(|_| "iphone".into()), ..Default::default() } } } else { Store::load() };
        crate::theme::set_dark(store.dark);
        ic::install(&cc.egui_ctx);
        theme(&cc.egui_ctx);
        phone::set_preview_iphone(store.settings.preview != "android");
        crate::viewer::set_default_code_size(store.settings.code_size);
        // the interface size control is gone, so a size picked with it before goes back to normal
        store.settings.ui_scale = 1.0;
        cc.egui_ctx.set_zoom_factor(1.0);
        let mut dirty = false;
        if store.projects.is_empty() {
            let id = store.next_project;
            store.projects.push(sample_project(id));
            store.next_project += 1;
            dirty = true;
        }
        let reopen = if shot.is_none() && store.settings.reopen_last { store.settings.last_project.filter(|id| store.projects.iter().any(|p| p.id == *id)) } else { None };
        let mut app = App { store, editor: reopen.map(Editor::new), proj_dialog: None, confirm_delete: None, notice: None, dirty, last_save: 0.0, no_save: shot.is_some(), shot, frames: 0, shot_sent: false, settings_open: false, search: String::new(), sort: 0, list_view: false, export: None, menu: None, after_close: None, mcp: None, mcp_err: None, mcp_shot: None };
        if app.shot.is_none() {
            app.menu = crate::menu::AppMenu::install(&cc.egui_ctx);
        }
        if let Some(o) = open {
            let pid = app.store.projects[0].id;
            let mut ed = Editor::new(pid);
            match o.as_str() {
                "design" => {
                    ed.design.selected = Some("button1".into());
                }
                // the button row set to wrap content, inside the centered column
                "design-wrap" => {
                    if let Some(w) = app.store.projects[0].screens[0].widgets.iter_mut().find(|w| w.id == "linear2") {
                        w.width = Size::Wrap;
                    }
                    ed.design.selected = Some("linear1".into());
                }
                // a full-screen row holding one text, with both gravities set
                "design-gravity" | "design-gravity-br" | "design-gravity-col" => {
                    let (h, v) = if o == "design-gravity-br" { (Gravity::Right, VGravity::Bottom) } else { (Gravity::Center, VGravity::Center) };
                    let s = &mut app.store.projects[0].screens[0];
                    s.widgets.retain(|w| w.id == "linear1" || w.id == "textview1" || (o == "design-gravity-col" && w.id == "textview2"));
                    if let Some(w) = s.widgets.iter_mut().find(|w| w.id == "linear1") {
                        w.kind = if o == "design-gravity-col" { WKind::LinearV } else { WKind::LinearH };
                        w.width = Size::Match;
                        w.height = Size::Match;
                        w.padding = 34;
                        w.gravity = h;
                        w.vgravity = v;
                    }
                    ed.design.selected = Some("linear1".into());
                }
                // a full-height column whose row takes the spare height, and
                // two buttons sharing that row 1:2
                "design-weight" => {
                    let s = &mut app.store.projects[0].screens[0];
                    for w in s.widgets.iter_mut() {
                        match w.id.as_str() {
                            "linear1" => w.height = Size::Match,
                            "linear2" => {
                                w.width = Size::Match;
                                w.height = Size::Match;
                                w.weight = 1.0;
                                w.bg = 0xFFE8_E8FF;
                            }
                            "button1" => w.weight = 1.0,
                            "button2" => w.weight = 2.0,
                            _ => {}
                        }
                    }
                    ed.design.selected = Some("linear2".into());
                }
                // the settings page with the MCP server on
                "settings-mcp" => {
                    app.settings_open = true;
                    app.store.settings.mcp = true;
                }
                // a screen from each template, and the window that picks them
                "templates" => {
                    let screens = crate::blocks::screen_choices(&app.store.projects[0]);
                    for t in crate::templates::TEMPLATES {
                        if let Ok(s) = crate::templates::screen(t, t.suggested, &screens) {
                            app.store.projects[0].screens.push(s);
                        }
                    }
                    let pick = std::env::var("BLOCKWISE_TEMPLATE").ok().and_then(|n| app.store.projects[0].screens.iter().position(|s| s.name == n));
                    match pick {
                        Some(i) => ed.screen = i,
                        None => {
                            ed.add_screen = Some((String::from("login"), String::new()));
                            ed.add_template = Some("login");
                        }
                    }
                }
                "event" => ed.tab = Tab::Event,
                "component" => ed.tab = Tab::Component,
                "logic" => {
                    ed.tab = Tab::Event;
                    ed.logic = Some(1);
                    ed.logic_state.cat = 6;
                }
                // the camera blocks in use, the palette at them
                "logic-camera" => {
                    ed.tab = Tab::Event;
                    ed.logic = Some(1);
                    ed.logic_state.cat = 7;
                    let sc = &mut app.store.projects[0].screens[ed.screen];
                    sc.components.push(Component { name: "cam".into(), kind: CompKind::Camera });
                    sc.components.push(Component { name: "player".into(), kind: CompKind::Sound });
                    let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
                    let t = |x: &str| Arg::Text(x.into());
                    let path = || Arg::Block(Box::new(blk("mediaPath", vec![])));
                    let pic = sc.widgets.iter().find(|w| w.kind == WKind::ImageView).map(|w| w.id.clone()).unwrap_or_default();
                    let mut photo = blk("cameraPhoto", vec![t("cam")]);
                    photo.sub1 = vec![blk("setImageFrom", vec![t(&pic), path()])];
                    photo.sub2 = vec![blk("doToast", vec![t("No photo")])];
                    let mut pick = blk("pickMedia", vec![t("sound")]);
                    pick.sub1 = vec![blk("soundPlayFrom", vec![t("player"), path()])];
                    if let Some(e) = sc.events.get_mut(1) {
                        e.blocks = vec![photo, pick];
                    }
                }
                // the Supabase blocks: the link form, or linked
                "logic-supabase" | "logic-supabase-linked" => {
                    ed.tab = Tab::Event;
                    ed.logic = Some(1);
                    ed.logic_state.cat = 9;
                    if o == "logic-supabase" {
                        ed.logic_state.open_supabase_form(SupabaseLink { url: "https://abcd.supabase.co".into(), key: String::new() });
                    } else {
                        app.store.projects[0].supabase = Some(SupabaseLink { url: "https://abcd.supabase.co".into(), key: "sb_publishable_x".into() });
                    }
                }
                // a more block with inputs, used in itself's palette, and its window
                "logic-more" | "logic-more-edit" => {
                    let s = &mut app.store.projects[0].screens[0];
                    let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
                    let read = |op: &str, n: &str| Arg::Block(Box::new(blk(op, vec![Arg::Text(n.into())])));
                    let mut rep = blk("repeat", vec![read("paramNum", "times")]);
                    rep.sub1 = vec![blk("doToast", vec![Arg::Block(Box::new(blk("stringJoin", vec![Arg::Text("Hello ".into()), read("paramStr", "name")])))])];
                    s.events.push(Event { target: "moreBlock".into(), name: "greet".into(), blocks: vec![rep], loose: vec![], params: vec![Var { name: "name".into(), ty: VarType::Str }, Var { name: "times".into(), ty: VarType::Num }] });
                    let call = blk("moreBlock", vec![Arg::Text("greet".into()), Arg::Text("Sam".into()), Arg::Text("3".into())]);
                    s.events[0].loose.push(LooseStack { x: 40.0, y: 260.0, blocks: vec![call] });
                    let ei = s.events.len() - 1;
                    ed.tab = Tab::Event;
                    ed.logic = Some(ei);
                    ed.logic_state.cat = 8;
                    if o == "logic-more-edit" {
                        let e = s.events[ei].clone();
                        ed.logic_state.edit_more(&e);
                    }
                }
                // corners, shadows, an outline, and a picture
                // the Assets page with every kind of file: BLOCKWISE_SHOT_FILES is a folder of them
                "assets" | "design-font" | "design-video" | "asset-view-image" | "asset-view-sound" | "asset-view-video" | "asset-view-font" | "asset-view-data" | "run-media" => {
                    let p = &mut app.store.projects[0];
                    p.id = 990_004;
                    let _ = std::fs::remove_dir_all(p.files_dir());
                    if let Some(dir) = std::env::var_os("BLOCKWISE_SHOT_FILES") {
                        let mut paths: Vec<_> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
                        paths.sort();
                        for f in paths {
                            let _ = p.add_file(&f);
                        }
                    }
                    let font = p.file_names(FileKind::Font).first().cloned().unwrap_or_default();
                    let data = p.file_names(FileKind::Data).first().cloned().unwrap_or_default();
                    let pic = p.file_names(FileKind::Image).first().cloned().unwrap_or_default();
                    let clip = p.file_names(FileKind::Video).first().cloned().unwrap_or_default();
                    let s = &mut p.screens[0];
                    if let Some(t) = s.widget_mut("textview1") {
                        t.font = font.clone();
                    }
                    if let Some(t) = s.widget_mut("textview2") {
                        t.font = font;
                    }
                    let mut img = Widget::new("imageview1".into(), WKind::ImageView, "linear1".into());
                    img.image = pic;
                    img.width = Size::Px(160);
                    s.widgets.push(img);
                    if !clip.is_empty() {
                        let mut v = Widget::new("videoview1".into(), WKind::VideoView, "linear1".into());
                        v.video = clip;
                        v.radius = 12;
                        s.widgets.push(v);
                    }
                    let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
                    s.events[0].blocks.push(blk("setText", vec![Arg::Text("textview2".into()), Arg::Block(Box::new(blk("assetText", vec![Arg::Text(data)])))]));
                    ed = Editor::new(990_004);
                    // the preview running: the video starts by itself, a sound plays
                    // again and again, and a text shows the sound's length
                    if o == "run-media" {
                        let p = &mut app.store.projects[0];
                        let ding = p.file_names(FileKind::Sound).first().cloned().unwrap_or_default();
                        let s = &mut p.screens[0];
                        if let Some(v) = s.widget_mut("videoview1") {
                            v.autoplay = true;
                        }
                        s.components.push(Component { name: "player".into(), kind: CompKind::Sound });
                        s.components.push(Component { name: "later".into(), kind: CompKind::Timer });
                        let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
                        let t = |x: &str| Arg::Text(x.into());
                        let len = Arg::Block(Box::new(blk("stringJoin", vec![t("sound ms: "), Arg::Block(Box::new(blk("soundDuration", vec![t("player")])))])));
                        let mut later = blk("timerAfter", vec![t("later"), t("1500")]);
                        later.sub1 = vec![blk("setText", vec![t("textview1"), len])];
                        s.events.push(Event { target: "videoview1".into(), name: "onCompletion".into(), blocks: vec![blk("setText", vec![t("textview2"), t("video ended")])], loose: vec![], params: vec![] });
                        s.events[0].blocks.extend([blk("soundLoop", vec![t("player"), Arg::Block(Box::new(blk("true", vec![])))]), blk("soundPlay", vec![t("player"), t(&ding)]), later]);
                        ed.run = Some(Runtime::new(p.clone(), 0.0));
                    }
                    // the viewer open over the Assets page, on a file of that kind
                    if let Some(k) = o.strip_prefix("asset-view-") {
                        ed.tab = Tab::Assets;
                        let p = &app.store.projects[0];
                        if let Some(f) = p.files.iter().find(|f| f.kind.menu() == k || (k == "data" && f.kind == FileKind::Data)) {
                            ed.asset_view = Some(crate::asset_view::AssetView::new(f, p.file_path(f)));
                        }
                    }
                    match o.as_str() {
                        "assets" => ed.tab = Tab::Assets,
                        "design-video" => ed.design.selected = Some("videoview1".into()),
                        _ => ed.design.selected = Some("textview1".into()),
                    }
                }
                // a scrolling column filling the screen, holding a centred
                // column taller than the phone, and a dropdown after it
                "scroll-column" | "scroll-column-play" => {
                    scroll_column_screen(&mut app.store.projects[0]);
                    if o == "scroll-column-play" {
                        ed.run = Some(Runtime::new(app.store.projects[0].clone(), 0.0));
                    }
                }
                // a screen taller than the phone, while designing
                "design-long" => {
                    let s = &mut app.store.projects[0].screens[0];
                    for i in 0..16 {
                        let mut b = Widget::new(format!("button{}", i + 3), WKind::Button, "linear1".into());
                        b.text = format!("Button {}", i + 3);
                        b.width = Size::Match;
                        s.widgets.push(b);
                    }
                }
                // the add-event list, searched
                "event-search" => {
                    ed.tab = Tab::Event;
                    ed.add_event = true;
                    ed.event_search = std::env::var("BLOCKWISE_SHOT_SEARCH").unwrap_or_else(|_| "button".into());
                }
                // text fields of each kind; playing, the password's eye is open
                "input-types" | "input-types-run" => {
                    let s = &mut app.store.projects[0].screens[0];
                    for (id, k, hint, text) in [
                        ("password1", InputKind::Password, "Password", "secret42"),
                        ("email1", InputKind::Email, "Email", ""),
                        ("number1", InputKind::Number, "Age", ""),
                        ("note1", InputKind::Multiline, "A note", ""),
                    ] {
                        let mut w = Widget::new(id.into(), WKind::EditText, "linear1".into());
                        w.input = k;
                        w.password = k == InputKind::Password;
                        w.hint = hint.into();
                        w.text = text.into();
                        w.width = Size::Match;
                        s.widgets.push(w);
                    }
                    ed.design.selected = Some("password1".into());
                    if o == "input-types-run" {
                        let mut rt = Runtime::new(app.store.projects[0].clone(), 0.0);
                        if let Some(st) = rt.widgets.get_mut("password1") {
                            st.reveal = true;
                        }
                        ed.run = Some(rt);
                    }
                }
                // the pencil beside a screen in the picker was pressed
                "screen-rename" => {
                    add_second_screen(&mut app.store.projects[0]);
                    ed.screen_rename = Some((1, "details".into(), String::new()));
                }
                // the x beside a screen in the picker was pressed
                "screen-delete" => {
                    add_second_screen(&mut app.store.projects[0]);
                    ed.screen_delete = Some(1);
                }
                "design-looks" => {
                    let p = &mut app.store.projects[0];
                    p.id = 990_001;
                    let pic = std::env::var("BLOCKWISE_SHOT_PICTURE").ok().and_then(|f| p.add_file(std::path::Path::new(&f)).ok());
                    let s = &mut p.screens[0];
                    if let Some(b) = s.widget_mut("button1") {
                        b.radius = 18;
                        b.elevation = 6;
                        b.bg = 0xFF5B_5BD6;
                        b.text_color = 0xFFFF_FFFF;
                    }
                    if let Some(t) = s.widget_mut("textview1") {
                        t.border = 2;
                        t.border_color = 0xFF5B_5BD6;
                        t.radius = 8;
                    }
                    let mut img = Widget::new("imageview1".into(), WKind::ImageView, "linear1".into());
                    img.image = pic.unwrap_or_default();
                    img.width = Size::Px(200);
                    img.radius = 16;
                    img.elevation = 4;
                    img.margin = 12;
                    s.widgets.push(img);
                    ed = Editor::new(990_001);
                    ed.design.selected = Some("imageview1".into());
                }
                // two blocks picked, as after a ⇧-click
                "logic-select" => {
                    ed.tab = Tab::Event;
                    ed.logic = Some(1);
                    ed.logic_state.cat = 6;
                    ed.logic_state.selected = app.store.projects[0].screens[0].events[1].blocks.iter().map(|b| b.id).collect();
                }
                "source" => ed.source = Some(SrcTab::KtScreen),
                "source-xml" => ed.source = Some(SrcTab::KtLayout),
                "run" => ed.run = Some(Runtime::new(app.store.projects[0].clone(), 0.0)),
                "android-setup" => {
                    ed.device = Some(crate::device::DeviceRun::setup_sample(app.store.projects[0].app_name.clone()));
                    ed.device_shown = true;
                }
                "design-play" => ed.run = Some(Runtime::new(app.store.projects[0].clone(), 0.0)),
                "kotlin" => ed.source = Some(SrcTab::KtScreen),
                "android" | "iphone" => {
                    let d = if o == "android" { crate::device::Device::Android } else { crate::device::Device::Iphone };
                    ed.device = Some(crate::device::DeviceRun::start(d, app.store.projects[0].clone(), cc.egui_ctx.clone()));
                    ed.device_shown = true;
                }
                "crash" => {
                    // a block that reads past the end of a list, then tap it
                    let mut p = app.store.projects[0].clone();
                    let s = &mut p.screens[0];
                    s.lists.push(ListVar { name: "names".into(), ty: ListType::Str });
                    let get = Block { id: new_id(), op: "getAtListStr".into(), args: vec![Arg::Text("2".into()), Arg::Text("names".into())], sub1: vec![], sub2: vec![] };
                    if let Some(e) = s.events.iter_mut().find(|e| e.target == "button1") {
                        e.blocks.insert(0, Block { id: new_id(), op: "setText".into(), args: vec![Arg::Text("textview1".into()), Arg::Block(Box::new(get))], sub1: vec![], sub2: vec![] });
                    }
                    let mut rt = Runtime::new(p, 0.0);
                    rt.fire("button1", "onClick", Default::default());
                    ed.run = Some(rt);
                }
                "swift" => ed.source = Some(SrcTab::SwScreen),
                "debug" | "debug-events" => {
                    // pause on the first block of the button's tap, then tap it
                    let p = app.store.projects[0].clone();
                    let mut bps = std::collections::HashSet::new();
                    if let Some(b) = p.screens[0].events.iter().find(|e| e.name == "onClick").and_then(|e| e.blocks.first()) {
                        bps.insert(b.id);
                    }
                    ed.logic_state.breakpoints = bps.clone();
                    let mut rt = Runtime::with_debug(p, 0.0, bps, false);
                    if let Some(e) = rt.project.screens[0].events.iter().find(|e| e.name == "onClick").cloned() {
                        rt.fire(&e.target, &e.name, Default::default());
                        rt.debug_step();
                    }
                    ed.run_tab = 0;
                    ed.debugging = true;
                    ed.run = Some(rt);
                    // the same, looked at from the blocks of the tapped event
                    if o == "debug-events" {
                        ed.tab = Tab::Event;
                        ed.logic = app.store.projects[0].screens[0].events.iter().position(|e| e.name == "onClick");
                    }
                }
                _ => {}
            }
            if o == "settings" {
                app.settings_open = true;
            }
            if o == "exit-ask" {
                ed.quit_ask = true;
            }
            if o == "projects-list" {
                app.list_view = true;
            }
            if o == "export" {
                app.open_export(pid);
            }
            if o == "new-project" || o == "icon-glyph" {
                app.new_project_dialog();
                if let (Some(d), true) = (app.proj_dialog.as_mut(), o == "icon-glyph") {
                    d.glyph = "rocket".into();
                }
            }
            if !matches!(o.as_str(), "projects" | "projects-list" | "settings" | "export" | "new-project" | "icon-glyph") {
                app.editor = Some(ed);
            }
        }
        app
    }

    fn project_mut(&mut self, pid: u32) -> Option<&mut Project> {
        self.store.projects.iter_mut().find(|p| p.id == pid)
    }

    fn new_project_dialog(&mut self) {
        let n = self.store.next_project;
        let name = if self.store.projects.iter().any(|p| p.name == "NewProject") { format!("NewProject{}", n) } else { "NewProject".to_string() };
        self.proj_dialog = Some(ProjDialog { edit: None, app_name: "New Application".into(), package: format!("com.my.{}", name.to_lowercase()), name, ver_code: 1, ver_name: "1.0".into(), color: ICON_COLORS[(n as usize) % ICON_COLORS.len()], glyph: String::new(), image: None, error: String::new() });
    }

    fn open_export(&mut self, pid: u32) {
        let st = &self.store.settings;
        let dir = st.export_dir.clone().unwrap_or_else(|| tilde(&crate::codegen::default_export_dir()));
        self.export = Some(ExportDialog { pid, kotlin: st.export_kotlin, swift: st.export_swift, dir, error: String::new() });
    }

    /// The "Export source" window: Kotlin, Swift or both, and the folder.
    fn export_dialog(&mut self, ctx: &Context) {
        let Some(d) = &mut self.export else { return };
        let Some(p) = self.store.projects.iter().find(|p| p.id == d.pid).cloned() else {
            self.export = None;
            return;
        };
        let now = ctx.input(|i| i.time);
        let mut close = false;
        let mut go = false;
        Window::new("Export source").order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                ic::paint(ui.painter(), r, &p);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&p.app_name).size(16.0).strong());
                    ui.label(RichText::new("Projects you can open in Android Studio or Xcode").color(crate::theme::gray(120)));
                });
            });
            ui.add_space(12.0);
            ui.label(RichText::new("Export").strong());
            ui.add_space(4.0);
            let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
            let ios = crate::device::ios_tools();
            if !ios {
                d.swift = false;
            }
            for (on, icon, title, what) in [(&mut d.kotlin, ic::SMARTPHONE, "Android  ·  Kotlin", "A Gradle project with the layouts, Kotlin code and app icon"), (&mut d.swift, ic::APPLE, "iOS  ·  Swift", "SwiftUI files and an Assets.xcassets with the app icon")].into_iter().take(if ios { 2 } else { 1 }) {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
                let fill = if *on { ui.visuals().selection.bg_fill.gamma_multiply(0.5) } else if resp.hovered() { crate::theme::surface_hover() } else { crate::theme::surface() };
                ui.painter().rect(r, 8.0, fill, Stroke::new(1.0_f32, if *on { PRIMARY } else { line }));
                let box_r = Rect::from_center_size(r.left_center() + vec2(20.0, 0.0), vec2(18.0, 18.0));
                ui.painter().rect(box_r, 4.0, if *on { PRIMARY } else { crate::theme::surface() }, Stroke::new(1.0_f32, if *on { PRIMARY } else { crate::theme::gray(170) }));
                if *on {
                    ui.painter().text(box_r.center(), Align2::CENTER_CENTER, ic::CHECK, FontId::proportional(13.0), Color32::WHITE);
                }
                ui.painter().text(r.left_center() + vec2(46.0, 0.0), Align2::LEFT_CENTER, icon, FontId::proportional(20.0), crate::theme::ink());
                ui.painter().text(pos2(r.left() + 76.0, r.center().y - 9.0), Align2::LEFT_CENTER, title, FontId::proportional(14.5), crate::theme::ink());
                ui.painter().text(pos2(r.left() + 76.0, r.center().y + 10.0), Align2::LEFT_CENTER, what, FontId::proportional(12.0), crate::theme::gray(130));
                if resp.clicked() {
                    *on = !*on;
                }
                ui.add_space(4.0);
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Save in").strong());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut d.dir).desired_width(310.0).margin(vec2(8.0, 5.0)));
                if ui.button(lbl(ic::FOLDER_OPEN, "Choose…")).on_hover_text("Pick a folder").clicked() {
                    let start = expand_home(&d.dir);
                    let mut fd = rfd::FileDialog::new().set_title("Where should the export go?");
                    if start.is_dir() {
                        fd = fd.set_directory(&start);
                    } else if let Some(h) = dirs::home_dir() {
                        fd = fd.set_directory(h);
                    }
                    if let Some(f) = fd.pick_folder() {
                        d.dir = tilde(&f);
                    }
                }
            });
            let home_default = tilde(&crate::codegen::default_export_dir());
            if d.dir.trim() != home_default && ui.link(RichText::new(format!("Use {home_default}")).size(12.5)).clicked() {
                d.dir = home_default;
            }
            let target = expand_home(&d.dir).join(crate::codegen::sanitize(&p.name));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{}  Files go in {}  (made if it isn't there)", ic::FOLDER, tilde(&target))).size(12.5).color(crate::theme::gray(120)));
            if !d.error.is_empty() {
                ui.add_space(4.0);
                ui.colored_label(Color32::from_rgb(200, 40, 40), &d.error);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                let ok = (d.kotlin || d.swift) && !d.dir.trim().is_empty();
                ui.add_enabled_ui(ok, |ui| {
                    if primary_button(ui, &lbl(ic::FILE_OUTPUT, "Export")).on_disabled_hover_text("Pick Android, iOS or both, and a folder").clicked() {
                        go = true;
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        if go {
            let parent = expand_home(&d.dir);
            match crate::codegen::export(&p, &parent, d.kotlin, d.swift) {
                Ok(path) => {
                    let what = match (d.kotlin, d.swift) {
                        (true, true) => "Android (Kotlin) and iOS (Swift)",
                        (true, false) => "Android (Kotlin)",
                        _ => "iOS (Swift)",
                    };
                    self.notice = Some((format!("Exported {what} to {}", tilde(&path)), now + 6.0));
                    crate::device::reveal_in_file_manager(&path);
                    let st = &mut self.store.settings;
                    st.export_dir = Some(d.dir.trim().to_string()).filter(|x| *x != tilde(&crate::codegen::default_export_dir()));
                    st.export_kotlin = d.kotlin;
                    st.export_swift = d.swift;
                    self.dirty = true;
                    close = true;
                }
                Err(e) => d.error = format!("Couldn't export there: {e}"),
            }
        }
        if close {
            self.export = None;
        }
    }

    /// The left column shared by the projects page and the settings page,
    /// headed by the Blockwise logo and name.
    fn nav_panel(&mut self, ui: &mut Ui) {
        let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
        SidePanel::left("nav").exact_width(232.0).resizable(false).frame(Frame::none().fill(crate::theme::surface()).inner_margin(Margin::symmetric(14.0, 16.0)).stroke(Stroke::new(1.0_f32, line))).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                if let Some(mark) = brand_mark(ui.ctx()) {
                    let h = 26.0;
                    let size = mark.size_vec2();
                    ui.add(Image::new(&mark).fit_to_exact_size(vec2(h * size.x / size.y, h)));
                    ui.add_space(2.0);
                }
                ui.label(RichText::new("Blockwise").size(20.0).strong());
            });
            ui.add_space(16.0);
            if ui.add_sized([ui.available_width(), 34.0], Button::new(RichText::new(lbl(ic::PLUS, "New project")).color(Color32::WHITE).strong()).fill(PRIMARY).rounding(7.0)).on_hover_text("Make a new app").clicked() {
                self.settings_open = false;
                self.new_project_dialog();
            }
            ui.add_space(14.0);
            let count = self.store.projects.len();
            let rows = [(lbl(ic::FOLDER, " Projects"), format!("{count}"), false), (lbl(ic::SETTINGS, " Settings"), "⌘,".to_string(), true)];
            for (label, hint, is_settings) in rows.into_iter().filter(|r| !(r.2 && crate::menu::NATIVE)) {
                let sel = self.settings_open == is_settings;
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                if sel {
                    ui.painter().rect_filled(r, 6.0, ui.visuals().selection.bg_fill);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, crate::theme::surface_hover());
                }
                ui.painter().text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), crate::theme::ink());
                ui.painter().text(r.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, hint, FontId::proportional(12.0), crate::theme::gray(140));
                if resp.clicked() {
                    self.settings_open = is_settings;
                }
                ui.add_space(2.0);
            }
        });
    }

    fn projects_page(&mut self, ui: &mut Ui) {
        let now = ui.input(|i| i.time);
        self.nav_panel(ui);
        #[derive(PartialEq)]
        enum Act {
            Open,
            Run,
            Build,
            Code,
            Settings,
            Backup,
            Duplicate,
            Export,
            Delete,
        }
        let mut act: Option<(u32, Act)> = None;
        let mut want_new = false;
        CentralPanel::default().frame(Frame::none().fill(crate::theme::bg()).inner_margin(Margin::symmetric(32.0, 24.0))).show_inside(ui, |ui| {
            // header: title, search, sort and view
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Projects").size(24.0).strong());
                    let n = self.store.projects.len();
                    ui.label(RichText::new(format!("{n} app{}", if n == 1 { "" } else { "s" })).color(crate::theme::gray(130)));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (grid_l, list_l) = (lbl(ic::GRID, "Grid"), lbl(ic::LIST, "List"));
                    if let Some(i) = designer::segmented(ui, &[grid_l.as_str(), list_l.as_str()], if self.list_view { 1 } else { 0 }) {
                        self.list_view = i == 1;
                    }
                    ui.add_space(8.0);
                    const SORTS: [&str; 3] = ["Newest", "Last edited", "Name"];
                    ComboBox::from_id_salt("proj_sort").selected_text(format!("Sort: {}", SORTS[self.sort.min(2) as usize])).width(150.0).show_ui(ui, |ui| {
                        for (i, s) in SORTS.iter().enumerate() {
                            ui.selectable_value(&mut self.sort, i as u8, *s);
                        }
                    });
                    ui.add_space(8.0);
                    let search = ui.add(TextEdit::singleline(&mut self.search).id(Id::new("proj_search")).hint_text(lbl(ic::SEARCH, "Search projects  ⌘F")).desired_width(240.0).margin(vec2(8.0, 6.0)));
                    if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F)) {
                        search.request_focus();
                    }
                });
            });
            ui.add_space(18.0);
            let q = self.search.trim().to_lowercase();
            let mut projects: Vec<&Project> = self.store.projects.iter().filter(|p| q.is_empty() || p.app_name.to_lowercase().contains(&q) || p.name.to_lowercase().contains(&q) || p.package.to_lowercase().contains(&q)).collect();
            match self.sort {
                2 => projects.sort_by(|a, b| a.app_name.to_lowercase().cmp(&b.app_name.to_lowercase())),
                1 => projects.sort_by(|a, b| b.modified.cmp(&a.modified).then(b.id.cmp(&a.id))),
                _ => projects.sort_by(|a, b| crate::model::newest_first(a, b)),
            }
            let card_menu = |ui: &mut Ui, pid: u32, act: &mut Option<(u32, Act)>| {
                ui.menu_button(RichText::new(ic::ELLIPSIS).size(17.0), |ui| {
                    ui.set_min_width(210.0);
                    // quick actions first, then the project's own
                    let mut quick = vec![(lbl(ic::FOLDER_OPEN, "Open"), Act::Open)];
                    quick.push((lbl(ic::BUG, "Debug in Blockwise engine"), Act::Run));
                    quick.push((lbl(ic::HAMMER, "Build APK"), Act::Build));
                    quick.push((lbl(ic::CODE, "Show code"), Act::Code));
                    for (label, a) in quick {
                        if ui.button(label).clicked() {
                            *act = Some((pid, a));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    for (label, a) in [(lbl(ic::PENCIL, "Project settings…"), Act::Settings), (lbl(ic::ARCHIVE, "Back up"), Act::Backup), (lbl(ic::FILE_OUTPUT, "Export source…"), Act::Export), (lbl(ic::COPY, "Duplicate"), Act::Duplicate)] {
                        if ui.button(label).clicked() {
                            *act = Some((pid, a));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button(RichText::new(lbl(ic::TRASH, "Delete…")).color(Color32::from_rgb(210, 50, 50))).clicked() {
                        *act = Some((pid, Act::Delete));
                        ui.close_menu();
                    }
                });
            };
            let icon = |ui: &Ui, r: Rect, p: &Project| ic::paint(ui.painter(), r, p);
            let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
            let (list_view, none_at_all) = (self.list_view, self.store.projects.is_empty());
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if projects.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        let msg = if none_at_all { "No projects yet. Press “New project” to make your first app." } else { "No project matches the search." };
                        ui.label(RichText::new(msg).size(16.0).color(Color32::GRAY));
                    });
                } else if list_view {
                    // a table: one row per project
                    let w = ui.available_width();
                    let cols = [0.0, w * 0.40, w * 0.58, w * 0.70, w * 0.82];
                    let (hr, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
                    for (x, t) in cols.iter().zip(["Name", "Screens", "Version", "Edited", ""]) {
                        ui.painter().text(pos2(hr.left() + x + if *x == 0.0 { 62.0 } else { 0.0 }, hr.center().y), Align2::LEFT_CENTER, t, FontId::proportional(12.0), crate::theme::gray(130));
                    }
                    Frame::none().fill(crate::theme::surface()).rounding(10.0).stroke(Stroke::new(1.0_f32, line)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, p) in projects.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 60.0), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r.shrink(2.0), 8.0, crate::theme::surface_hover());
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            if i > 0 {
                                ui.painter().line_segment([r.left_top() + vec2(14.0, 0.0), r.right_top() - vec2(14.0, 0.0)], Stroke::new(1.0_f32, line));
                            }
                            icon(ui, Rect::from_center_size(r.left_center() + vec2(36.0, 0.0), vec2(36.0, 36.0)), p);
                            let pt = ui.painter_at(Rect::from_min_max(r.min, pos2(r.left() + cols[1] - 10.0, r.bottom())));
                            pt.text(pos2(r.left() + 62.0, r.center().y - 9.0), Align2::LEFT_CENTER, &p.app_name, FontId::proportional(15.0), crate::theme::ink());
                            pt.text(pos2(r.left() + 62.0, r.center().y + 10.0), Align2::LEFT_CENTER, &p.package, FontId::proportional(12.5), crate::theme::gray(130));
                            let c = |x: f32, t: String| ui.painter().text(pos2(r.left() + x, r.center().y), Align2::LEFT_CENTER, t, FontId::proportional(13.5), crate::theme::gray(90));
                            c(cols[1], format!("{}", p.screens.len()));
                            c(cols[2], format!("{} ({})", p.ver_name, p.ver_code));
                            c(cols[3], ago(p.modified));
                            let mut m = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(r.right_center() - vec2(30.0, 0.0), vec2(36.0, 30.0))).layout(Layout::left_to_right(Align::Center)));
                            card_menu(&mut m, p.id, &mut act);
                            if resp.clicked() && act.is_none() {
                                act = Some((p.id, Act::Open));
                            }
                        }
                    });
                } else {
                    // a grid of cards that fills the window's width
                    let gap = 16.0;
                    let w = ui.available_width();
                    let cols = (((w + gap) / (300.0 + gap)).floor() as usize).max(1);
                    let cw = ((w - gap * (cols as f32 - 1.0)) / cols as f32).floor();
                    let ch = 156.0;
                    let items: Vec<Option<&Project>> = projects.iter().map(|p| Some(*p)).collect();
                    for row in items.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for item in row {
                                let (r, resp) = ui.allocate_exact_size(vec2(cw, ch), Sense::click());
                                let hover = resp.hovered();
                                if hover {
                                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                }
                                match item {
                                    Some(p) => {
                                        ui.painter().add(epaint::Shadow { offset: vec2(0.0, if hover { 4.0 } else { 1.0 }), blur: if hover { 14.0 } else { 4.0 }, spread: 0.0, color: Color32::from_black_alpha(if hover { 40 } else { 18 }) }.as_shape(r, 12.0));
                                        ui.painter().rect(r, 12.0, crate::theme::surface(), Stroke::new(1.0_f32, if hover { PRIMARY } else { line }));
                                        icon(ui, Rect::from_min_size(r.min + vec2(18.0, 18.0), vec2(48.0, 48.0)), p);
                                        let pt = ui.painter_at(r.shrink(14.0));
                                        pt.text(r.min + vec2(18.0, 84.0), Align2::LEFT_CENTER, &p.app_name, FontId::proportional(16.0), crate::theme::ink());
                                        pt.text(r.min + vec2(18.0, 106.0), Align2::LEFT_CENTER, &p.package, FontId::proportional(12.5), crate::theme::gray(130));
                                        let meta = format!("{} screen{}  ·  v{}  ·  {}", p.screens.len(), if p.screens.len() == 1 { "" } else { "s" }, p.ver_name, ago(p.modified));
                                        pt.text(r.min + vec2(18.0, 134.0), Align2::LEFT_CENTER, meta, FontId::proportional(12.0), crate::theme::gray(150));
                                        let mut m = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(r.right_top() + vec2(-48.0, 12.0), vec2(36.0, 30.0))).layout(Layout::left_to_right(Align::Center)));
                                        card_menu(&mut m, p.id, &mut act);
                                        if resp.clicked() && act.is_none() {
                                            act = Some((p.id, Act::Open));
                                        }
                                    }
                                    None => {
                                        let col = if hover { PRIMARY } else { crate::theme::gray(170) };
                                        let dash = egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(1.5_f32, col), 6.0, 4.0);
                                        ui.painter().extend(dash);
                                        ui.painter().text(r.center() - vec2(0.0, 12.0), Align2::CENTER_CENTER, ic::FOLDER_PLUS, FontId::proportional(26.0), col);
                                        ui.painter().text(r.center() + vec2(0.0, 20.0), Align2::CENTER_CENTER, "New project", FontId::proportional(14.0), col);
                                        if resp.clicked() {
                                            want_new = true;
                                        }
                                    }
                                }
                            }
                        });
                        ui.add_space(gap);
                    }
                }
                ui.add_space(24.0);
            });
        });
        if self.proj_dialog.is_none() && self.export.is_none() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::N)) {
            want_new = true;
        }
        if want_new {
            self.new_project_dialog();
        }
        if let Some((id, a)) = act {
            let Some(p) = self.store.projects.iter().find(|p| p.id == id).cloned() else { return };
            match a {
                Act::Open | Act::Run | Act::Build | Act::Code => {
                    if let Some(p) = self.project_mut(id) {
                        p.modified = now_secs();
                    }
                    let mut ed = Editor::new(id);
                    if a == Act::Run {
                        ed.run = Some(Runtime::new(p.clone(), now));
                        ed.debugging = true;
                    } else if a == Act::Build {
                        ed.build_start = Some(crate::device::Device::Android);
                        ed.device_shown = true;
                    } else if a == Act::Code {
                        ed.source = Some(SrcTab::KtScreen);
                    }
                    self.editor = Some(ed);
                    self.dirty = true;
                }
                Act::Settings => {
                    self.proj_dialog = Some(ProjDialog { edit: Some(p.id), app_name: p.app_name.clone(), name: p.name.clone(), package: p.package.clone(), ver_code: p.ver_code, ver_name: p.ver_name.clone(), color: p.icon_color, glyph: p.icon_glyph.clone(), image: p.icon_image.clone(), error: String::new() });
                }
                Act::Duplicate => {
                    let mut c = p.clone();
                    c.id = self.store.next_project;
                    self.store.next_project += 1;
                    let mut k = 1;
                    let taken = |n: &str| self.store.projects.iter().any(|q| q.name == n);
                    let mut name = format!("{}Copy", p.name);
                    while taken(&name) {
                        k += 1;
                        name = format!("{}Copy{}", p.name, k);
                    }
                    c.package = format!("{}{}", p.package, if k == 1 { "copy".to_string() } else { format!("copy{k}") });
                    c.name = name;
                    c.app_name = format!("{} copy", p.app_name);
                    c.modified = now_secs();
                    self.notice = Some((format!("Made a copy: {}", c.app_name), now + 3.0));
                    self.store.projects.push(c);
                    self.dirty = true;
                }
                Act::Backup => {
                    self.notice = Some((
                        match crate::model::backup(&p) {
                            Ok(path) => format!("Backed up to {}", path.display()),
                            Err(e) => format!("Couldn't back up: {e}"),
                        },
                        now + 6.0,
                    ));
                }
                Act::Export => self.open_export(id),
                Act::Delete => {
                    if self.store.settings.confirm_delete {
                        self.confirm_delete = Some(id);
                    } else {
                        self.store.projects.retain(|q| q.id != id);
                        self.dirty = true;
                    }
                }
            }
        }
        self.project_dialog(ui.ctx());
        if let Some(id) = self.confirm_delete {
            let name = self.store.projects.iter().find(|p| p.id == id).map(|p| p.app_name.clone()).unwrap_or_default();
            Window::new("Delete project?").order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
                ui.label("Delete this project? This can't be undone.");
                ui.label(RichText::new(format!("\"{name}\" and all its screens and blocks go. This can't be undone.")).color(crate::theme::gray(120)));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.confirm_delete = None;
                    }
                    if ui.add(Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(Color32::from_rgb(200, 40, 40))).clicked() {
                        self.store.projects.retain(|p| p.id != id);
                        let _ = std::fs::remove_dir_all(files_root().join(id.to_string()));
                        self.confirm_delete = None;
                        self.dirty = true;
                    }
                });
            });
        }
    }

    /// The settings page (⌘, opens and closes it; Esc closes it).
    fn settings_page(&mut self, ui: &mut Ui) {
        let in_editor = self.editor.is_some();
        if in_editor || crate::menu::NATIVE {
            let mut back = false;
            top_bar(ui, |ui| {
                if bar_button(ui, &lbl(ic::ARROW_LEFT, if in_editor { "Back" } else { "Projects" })).on_hover_text("Back (Esc)").clicked() {
                    back = true;
                }
                ui.label(RichText::new("Settings").size(15.5).strong());
            });
            if back {
                self.settings_open = false;
            }
        } else {
            self.nav_panel(ui);
        }
        let mcp_status: Result<String, String> = match (&self.mcp, &self.mcp_err) {
            (Some(srv), _) => Ok(srv.url()),
            (None, Some((_, e, _))) => Err(e.clone()),
            (None, None) => Err("starting…".into()),
        };
        let st = &mut self.store.settings;
        let mut changed = false;
        CentralPanel::default().frame(Frame::none().fill(crate::theme::bg()).inner_margin(Margin::symmetric(32.0, 24.0))).show_inside(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let w = ui.available_width().min(760.0);
                ui.set_max_width(w);
                ui.label(RichText::new("Settings").size(24.0).strong());
                ui.label(RichText::new("Saved with your projects. ⌘, opens this page and Esc closes it.").color(crate::theme::gray(130)));
                ui.add_space(18.0);
                let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
                let card = |ui: &mut Ui, title: &str, add: &mut dyn FnMut(&mut Ui)| {
                    ui.label(RichText::new(title).size(13.0).strong().color(crate::theme::gray(90)));
                    ui.add_space(6.0);
                    Frame::none().fill(crate::theme::surface()).rounding(10.0).stroke(Stroke::new(1.0_f32, line)).inner_margin(Margin::symmetric(18.0, 14.0)).show(ui, |ui| {
                        ui.set_width(w - 36.0);
                        Grid::new(title).num_columns(2).min_col_width(220.0).spacing(vec2(24.0, 14.0)).show(ui, |ui| add(ui));
                    });
                    ui.add_space(20.0);
                };
                card(ui, "APPEARANCE", &mut |ui| {
                    ui.label("Theme");
                    let dark = crate::theme::dark();
                    if let Some(i) = designer::segmented(ui, &[&lbl(ic::SUN, "Light"), &lbl(ic::MOON, "Dark")], if dark { 1 } else { 0 }) {
                        crate::theme::set_dark(i == 1);
                        theme(ui.ctx());
                    }
                    ui.end_row();
                });
                if crate::device::ios_tools() { card(ui, "PHONE PREVIEW", &mut |ui| {
                    ui.label("Phone shown in the editor");
                    let iphone = phone::preview_iphone();
                    if let Some(i) = designer::segmented(ui, &["Android", "iOS"], if iphone { 1 } else { 0 }) {
                        phone::set_preview_iphone(i == 1);
                    }
                    ui.end_row();
                }); }
                card(ui, "AI ASSISTANTS (MCP)", &mut |ui| {
                    ui.label("Let AI assistants build apps").on_hover_text("Claude Code, Claude Desktop and other MCP clients can read and change your projects, look at the preview and run apps, while Blockwise is open. Only programs on this Mac can connect.");
                    ui.vertical(|ui| {
                        changed |= ui.checkbox(&mut st.mcp, "Turn on the MCP server").changed();
                        ui.label(RichText::new("While Blockwise is open, assistants on this Mac can change your projects. Their changes show up live and ⌘Z undoes them.").size(12.0).color(crate::theme::gray(130)));
                    });
                    ui.end_row();
                    ui.label("Port");
                    changed |= ui.add(DragValue::new(&mut st.mcp_port).range(1024..=65535).speed(0.2)).changed();
                    ui.end_row();
                    if st.mcp {
                        ui.label("Status");
                        match &mcp_status {
                            Ok(url) => ui.label(RichText::new(format!("●  Listening at {url}")).color(Color32::from_rgb(0x2E, 0x9E, 0x55))),
                            Err(e) => ui.label(RichText::new(format!("●  Couldn't start: {e}")).color(Color32::from_rgb(200, 40, 40))),
                        };
                        ui.end_row();
                        let url = crate::mcp::url(st.mcp_port);
                        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "Blockwise".into());
                        let snippets = [
                            ("Claude Code", format!("claude mcp add --transport http blockwise {url}")),
                            ("Claude Desktop", format!("{{\n  \"mcpServers\": {{\n    \"blockwise\": {{\n      \"command\": {exe:?},\n      \"args\": [\"--mcp\", \"--port\", \"{}\"]\n    }}\n  }}\n}}", st.mcp_port)),
                        ];
                        for (who, text) in snippets {
                            ui.label(who);
                            ui.vertical(|ui| {
                                if who == "Claude Desktop" {
                                    ui.label(RichText::new("Add to claude_desktop_config.json:").size(12.0).color(crate::theme::gray(130)));
                                }
                                Frame::none().fill(crate::theme::bg()).rounding(6.0).inner_margin(Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
                                    ui.label(RichText::new(&text).monospace().size(12.0));
                                });
                                if ui.button(lbl(ic::COPY, "Copy")).clicked() {
                                    ui.ctx().copy_text(text.clone());
                                }
                            });
                            ui.end_row();
                        }
                    }
                });
                card(ui, "EXPORT", &mut |ui| {
                    ui.label("Export folder");
                    ui.vertical(|ui| {
                        let shown = st.export_dir.clone().unwrap_or_else(|| tilde(&crate::codegen::default_export_dir()));
                        ui.label(RichText::new(&shown).monospace().size(12.0).color(crate::theme::gray(110)));
                        ui.horizontal(|ui| {
                            if ui.button(lbl(ic::FOLDER_OPEN, "Change…")).clicked() {
                                let mut fd = rfd::FileDialog::new().set_title("Where should exports go?");
                                let start = expand_home(&shown);
                                if start.is_dir() {
                                    fd = fd.set_directory(start);
                                }
                                if let Some(f) = fd.pick_folder() {
                                    st.export_dir = Some(tilde(&f));
                                    changed = true;
                                }
                            }
                            if st.export_dir.is_some() && ui.button(lbl(ic::HOUSE, "Use ~/Blockwise")).clicked() {
                                st.export_dir = None;
                                changed = true;
                            }
                        });
                    });
                    ui.end_row();
                });
                card(ui, "CODE", &mut |ui| {
                    ui.label("Code text size");
                    ComboBox::from_id_salt("set_code_size").selected_text(format!("{}", st.code_size)).width(80.0).show_ui(ui, |ui| {
                        // the largest size goes back to 14, as on the code page, so it isn't offered
                        for z in crate::viewer::CODE_SIZES.iter().copied().filter(|z| crate::viewer::code_size(*z) == *z) {
                            if ui.selectable_value(&mut st.code_size, z, format!("{z}")).changed() {
                                changed = true;
                            }
                        }
                    });
                    ui.end_row();
                });
                card(ui, "PROJECTS", &mut |ui| {
                    ui.label("Automatically save").on_hover_text("Keep your changes without asking when you leave a project. Off: Blockwise asks first.");
                    changed |= ui.checkbox(&mut st.auto_save, "").changed();
                    ui.end_row();
                    ui.label("Ask before deleting a project");
                    changed |= ui.checkbox(&mut st.confirm_delete, "").changed();
                    ui.end_row();
                    ui.label("Open the last project at start");
                    changed |= ui.checkbox(&mut st.reopen_last, "").changed();
                    ui.end_row();
                    ui.label("Saved in");
                    ui.label(RichText::new(Store::path().display().to_string()).monospace().size(12.0).color(crate::theme::gray(110)));
                    ui.end_row();
                });
                card(ui, "KEYBOARD", &mut |ui| {
                    let groups: [(&str, &[(&str, &str)]); 4] = [
                        ("Everywhere", &[("⌘ ,", "Open or close settings"), ("⌘ N", "New project"), ("⌘ W", "Close the project"), ("⌘ F", "Search projects")]),
                        (
                            "In a project",
                            &[
                                ("⌘ Z  /  ⇧⌘ Z", "Undo  /  redo"),
                                ("⌘ 1  2  3", "Design, Events, Components"),
                                ("⌘ R", "Run on the emulator or simulator picked last"),
                                ("⌘ B", "Build the app"),
                                ("⌘ J", "Show the code"),
                                ("⌘ E", "Export source"),
                                ("⌘ S", "Save now"),
                                ("Esc", "Drop the selection, then step back (event, tab, project)"),
                            ],
                        ),
                        (
                            "Blocks",
                            &[
                                ("Click  /  ⇧-click", "Select a block  /  add one to the selection"),
                                ("Drag on the canvas", "Select with a box"),
                                ("⌘ A", "Select everything"),
                                ("⌘ D", "Duplicate"),
                                ("⌘ C  X  V", "Copy, cut, paste (also between events and screens)"),
                                ("Delete key", "Delete"),
                                ("Alt-drag", "Drag a copy"),
                                ("Scroll, Space-drag", "Move around the canvas"),
                            ],
                        ),
                        ("Design", &[("↑  ↓", "Pick the widget above or below"), ("Alt ↑  Alt ↓", "Move it up or down"), ("⌘ D", "Duplicate"), ("⌘ C  X  V", "Copy, cut, paste"), ("Delete key", "Delete")]),
                    ];
                    for (title, rows) in groups {
                        ui.label(RichText::new(lbl(ic::KEYBOARD, title)).strong());
                        ui.label("");
                        ui.end_row();
                        for (k, what) in rows {
                            ui.label(RichText::new(*k).monospace().color(crate::theme::gray(90)));
                            ui.label(*what);
                            ui.end_row();
                        }
                    }
                });
            });
        });
        if changed {
            crate::viewer::set_default_code_size(self.store.settings.code_size);
            self.dirty = true;
        }
        self.project_dialog(ui.ctx());
    }

    fn project_dialog(&mut self, ctx: &Context) {
        let Some(d) = &mut self.proj_dialog else { return };
        let mut close = false;
        let mut save = false;
        let title = if d.edit.is_some() { "Project settings" } else { "New project" };
        Window::new(title).order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_width(400.0);
            Grid::new("projfields").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                ui.label("App name");
                ui.text_edit_singleline(&mut d.app_name);
                ui.end_row();
                ui.label("Project name");
                let before = d.name.clone();
                ui.text_edit_singleline(&mut d.name);
                if d.edit.is_none() && d.name != before && d.package == format!("com.my.{}", before.to_lowercase()) {
                    d.package = format!("com.my.{}", d.name.to_lowercase());
                }
                ui.end_row();
                ui.label("App ID").on_hover_text("A unique name for the app, like com.example.app. Both phones use it.");
                ui.text_edit_singleline(&mut d.package);
                ui.end_row();
                ui.label("Version code");
                ui.add(DragValue::new(&mut d.ver_code).range(1..=99999));
                ui.end_row();
                ui.label("Version name");
                ui.text_edit_singleline(&mut d.ver_name);
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            // the app icon: a letter or a symbol on a colour, or a picture
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
                ic::paint_face(ui.painter(), r, d.color, &d.face());
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("App icon").strong());
                    let kind = if d.image.is_some() { 2 } else if d.glyph.is_empty() { 0 } else { 1 };
                    let (a, b, c) = (lbl(ic::TYPE, "Letter"), lbl(ic::BLOCKS, "Symbol"), lbl(ic::IMAGE, "Picture"));
                    if let Some(i) = designer::segmented(ui, &[a.as_str(), b.as_str(), c.as_str()], kind) {
                        match i {
                            0 => {
                                d.glyph.clear();
                                d.image = None;
                            }
                            1 => {
                                d.image = None;
                                if d.glyph.is_empty() {
                                    d.glyph = ic::APP_GLYPHS[0].0.into();
                                }
                            }
                            _ => {
                                if let Some(f) = rfd::FileDialog::new().set_title("Pick a picture for the app icon").add_filter("Pictures", &["png", "jpg", "jpeg"]).pick_file() {
                                    match ic::import_picture(&f) {
                                        Ok(path) => d.image = Some(path.display().to_string()),
                                        Err(e) => d.error = e,
                                    }
                                }
                            }
                        }
                    }
                    ui.add_space(4.0);
                    let hint = match kind {
                        2 => "Square pictures work best. It fills the whole icon.",
                        1 => "Pick a symbol and a colour below.",
                        _ => "The app name's first letter on a colour.",
                    };
                    ui.label(RichText::new(hint).size(12.0).color(crate::theme::gray(130)));
                    if kind == 2 && ui.button(lbl(ic::UPLOAD, "Pick another picture…")).clicked() {
                        if let Some(f) = rfd::FileDialog::new().add_filter("Pictures", &["png", "jpg", "jpeg"]).pick_file() {
                            match ic::import_picture(&f) {
                                Ok(path) => d.image = Some(path.display().to_string()),
                                Err(e) => d.error = e,
                            }
                        }
                    }
                });
            });
            if d.image.is_none() {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    for c in ICON_COLORS {
                        let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                        ui.painter().rect_filled(r, 6.0, c32(c));
                        if d.color == c {
                            ui.painter().rect_stroke(r.expand(2.5), 7.0, Stroke::new(2.0_f32, crate::theme::ink()));
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, ic::CHECK, FontId::proportional(13.0), Color32::WHITE);
                        }
                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            d.color = c;
                        }
                    }
                });
            }
            if !d.glyph.is_empty() && d.image.is_none() {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for (name, g) in ic::APP_GLYPHS {
                        let (r, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                        let on = d.glyph == *name;
                        if on {
                            ui.painter().rect_filled(r, 7.0, c32(d.color));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 7.0, crate::theme::surface_hover());
                        }
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, *g, FontId::proportional(18.0), if on { Color32::WHITE } else { crate::theme::gray(80) });
                        if resp.on_hover_text(name.replace('-', " ")).clicked() {
                            d.glyph = name.to_string();
                        }
                    }
                });
            }
            ui.add_space(6.0);
            if !d.error.is_empty() {
                ui.colored_label(Color32::from_rgb(200, 40, 40), &d.error);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                if ui.add(Button::new(RichText::new(if d.edit.is_some() { "Save" } else { "Create" }).color(Color32::WHITE).strong()).fill(PRIMARY)).clicked() {
                    save = true;
                }
            });
        });
        if save {
            let d = self.proj_dialog.as_mut().unwrap();
            d.name = d.name.trim().to_string();
            d.app_name = d.app_name.trim().to_string();
            d.package = d.package.trim().to_string();
            let dup = self.store.projects.iter().any(|p| p.name == d.name && Some(p.id) != d.edit);
            if d.app_name.is_empty() {
                d.error = "Give the app a name".into();
            } else if d.name.is_empty() || !d.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                d.error = "Project name: letters, digits and _ only".into();
            } else if dup {
                d.error = "Another project already has that name".into();
            } else if !valid_package(&d.package) {
                d.error = "The App ID must look like com.example.app".into();
            } else if d.ver_name.trim().is_empty() {
                d.error = "Give a version name, like 1.0".into();
            } else {
                let d = self.proj_dialog.take().unwrap();
                match d.edit {
                    Some(id) => {
                        if let Some(p) = self.project_mut(id) {
                            p.app_name = d.app_name;
                            p.name = d.name;
                            p.package = d.package;
                            p.ver_code = d.ver_code;
                            p.ver_name = d.ver_name;
                            p.icon_color = d.color;
                            p.icon_glyph = d.glyph;
                            p.icon_image = d.image;
                            p.modified = now_secs();
                        }
                    }
                    None => {
                        let id = self.store.next_project;
                        self.store.next_project += 1;
                        let mut p = Project::new(id, &d.name, &d.app_name, &d.package);
                        p.ver_code = d.ver_code;
                        p.ver_name = d.ver_name;
                        p.icon_color = d.color;
                        p.icon_glyph = d.glyph;
                        p.icon_image = d.image;
                        self.store.projects.push(p);
                        self.editor = Some(Editor::new(id));
                    }
                }
                self.dirty = true;
            }
        }
        if close {
            self.proj_dialog = None;
        }
    }

    /// Save now: write the store and make the open project's current state
    /// the one leaving compares against.
    fn save_project(&mut self, now: f64) {
        let Some(ed) = self.editor.as_mut() else { return };
        if let Some(p) = self.store.projects.iter().find(|p| p.id == ed.pid) {
            ed.opened = Some(crate::model::project_snapshot(p));
        }
        ed.notice = Some((lbl(ic::CHECK, "Saved"), now + 1.2));
        if !self.no_save {
            self.store.save();
            self.dirty = false;
            self.last_save = now;
        }
    }

    /// Do what a menu item asks. Opening or starting another project closes
    /// the open one first, with the usual question about unsaved changes.
    fn menu_cmd(&mut self, ctx: &Context, c: crate::menu::Cmd) {
        use crate::menu::Cmd;
        let now = ctx.input(|i| i.time);
        match c {
            Cmd::Settings => self.settings_open = true,
            Cmd::OpenProject(id) if self.editor.as_ref().is_some_and(|e| e.pid == id) => self.settings_open = false,
            Cmd::NewProject | Cmd::OpenProject(_) if self.editor.is_some() => {
                self.settings_open = false;
                if let Some(ed) = self.editor.as_mut() {
                    ed.close_req = true;
                }
                self.after_close = Some(c);
            }
            Cmd::NewProject => {
                self.settings_open = false;
                self.new_project_dialog();
            }
            Cmd::OpenProject(id) => {
                if self.store.projects.iter().any(|p| p.id == id) {
                    self.settings_open = false;
                    self.editor = Some(Editor::new(id));
                }
            }
            Cmd::CloseProject => {
                if let Some(ed) = self.editor.as_mut() {
                    self.settings_open = false;
                    ed.close_req = true;
                }
            }
            Cmd::Save => self.save_project(now),
            Cmd::ToggleAutoSave => {
                self.store.settings.auto_save = !self.store.settings.auto_save;
                self.dirty = true;
            }
            Cmd::Export => {
                if let Some(pid) = self.editor.as_ref().map(|e| e.pid) {
                    self.open_export(pid);
                }
            }
            Cmd::ToggleDark => {
                crate::theme::set_dark(!crate::theme::dark());
                theme(ctx);
            }
            Cmd::PreviewIphone(on) => phone::set_preview_iphone(on),
        }
    }

    /// Start or stop the MCP server to match the settings.
    fn sync_mcp(&mut self, ctx: &Context, now: f64) {
        // BLOCKWISE_MCP_PORT turns it on regardless (tests use it with the
        // throwaway --screenshot store)
        let forced: Option<u16> = std::env::var("BLOCKWISE_MCP_PORT").ok().and_then(|p| p.parse().ok());
        if self.shot.is_some() && forced.is_none() {
            return;
        }
        let want = forced.or(self.store.settings.mcp.then_some(self.store.settings.mcp_port));
        if want == self.mcp.as_ref().map(|s| s.port) {
            return;
        }
        self.mcp = None;
        let Some(port) = want else {
            self.mcp_err = None;
            return;
        };
        // after a failure, try the same port again only now and then
        if self.mcp_err.as_ref().is_some_and(|(p, _, t)| *p == port && now - t < 3.0) {
            ctx.request_repaint_after(std::time::Duration::from_secs(3));
            return;
        }
        match crate::mcp::Server::start(port, ctx.clone()) {
            Ok(s) => {
                self.mcp = Some(s);
                self.mcp_err = None;
            }
            Err(e) => {
                let why = if e.kind() == std::io::ErrorKind::AddrInUse { format!("port {port} is already in use") } else { e.to_string() };
                self.mcp_err = Some((port, why, now));
            }
        }
    }

    /// Open a project for an MCP tool (on the screen asked for), unless that
    /// would leave another project's unsaved changes behind.
    fn mcp_open(&mut self, a: &serde_json::Value) -> Result<(u32, usize), String> {
        let pid = crate::mcp::project_id(a)?;
        let p = crate::mcp::find_project(&mut self.store, a)?;
        let si = crate::mcp::screen_index(p, a)?;
        if let Some(ed) = &self.editor {
            if ed.pid != pid {
                let cur = self.store.projects.iter().find(|p| p.id == ed.pid);
                let differs = cur.is_some_and(|c| ed.opened.as_deref() != Some(project_snapshot(c).as_str()));
                if differs && !self.store.settings.auto_save {
                    return Err(format!("\"{}\" is open in Blockwise with unsaved changes. Ask the person to save it (⌘S) or close it first.", cur.map(|c| c.app_name.as_str()).unwrap_or("A project")));
                }
                self.editor = None;
                self.dirty = true;
            }
        }
        if self.editor.is_none() {
            self.editor = Some(Editor::new(pid));
        }
        self.settings_open = false;
        let ed = self.editor.as_mut().unwrap();
        if ed.quit_ask {
            return Err("Blockwise is asking the person whether to save; try again once they've answered".into());
        }
        ed.screen = si;
        ed.tab = Tab::View;
        ed.logic = None;
        ed.source = None;
        ed.run = None;
        Ok((pid, si))
    }

    /// One tool call from an AI assistant.
    fn mcp_call(&mut self, ctx: &Context, c: crate::mcp::Call, now: f64) {
        use crate::mcp::Reply;
        use serde_json::json;
        let a = &c.args;
        let reply = match c.tool.as_str() {
            "open_project" => match self.mcp_open(a) {
                Ok((pid, si)) => Reply::Json(json!({"opened": pid, "screen": self.store.projects.iter().find(|p| p.id == pid).map(|p| p.screens[si].name.clone())})),
                Err(e) => Reply::Error(e),
            },
            "preview_screenshot" => match self.mcp_open(a) {
                Ok((pid, si)) => {
                    match a.get("device").and_then(|d| d.as_str()) {
                        Some("android") => phone::set_preview_iphone(false),
                        Some("iphone") => phone::set_preview_iphone(true),
                        _ => {}
                    }
                    if let Some(ed) = self.editor.as_mut() {
                        ed.design.selected = None;
                        ed.design.drag = None;
                    }
                    let name = self.store.projects.iter().find(|p| p.id == pid).map(|p| format!("{} › {}", p.app_name, p.screens[si].display_name())).unwrap_or_default();
                    let note = format!("{name}, {} preview", if phone::preview_iphone() { "iOS" } else { "Android" });
                    ctx.data_mut(|d| d.remove::<Rect>(Id::new("design_phone_on_window")));
                    // no notice over the phone in the picture
                    self.notice = None;
                    self.mcp_shot = Some(McpShot { reply: c.reply, frames: 0, sent: false, note });
                    ctx.request_repaint();
                    return;
                }
                Err(e) => Reply::Error(e),
            },
            "run_on_device" => {
                let device = match a.get("device").and_then(|d| d.as_str()) {
                    Some("android") => Some(crate::device::Device::Android),
                    Some("iphone") | Some("ios") => Some(crate::device::Device::Iphone),
                    _ => None,
                };
                match (device, self.mcp_open(a)) {
                    (None, _) => Reply::Error("\"device\" is android or iphone".into()),
                    (Some(crate::device::Device::Iphone), _) if !crate::device::ios_tools() => Reply::Error("iOS apps can only be built on a Mac with Xcode; use \"android\" here".into()),
                    (_, Err(e)) => Reply::Error(e),
                    (Some(d), Ok(_)) => {
                        let ed = self.editor.as_mut().unwrap();
                        if device_building(ed) {
                            Reply::Error("A build is already under way; follow it with device_status".into())
                        } else {
                            ed.device_start = Some(d);
                            ed.last_device = d;
                            Reply::Json(json!({"started": a["device"], "next": "Building takes from several seconds to a few minutes; call device_status to follow it."}))
                        }
                    }
                }
            }
            "device_status" => match self.editor.as_ref().and_then(|e| e.device.as_ref()) {
                None => Reply::Json(json!({"phase": "none", "note": "Nothing has run on a device since the project was opened."})),
                Some(run) => {
                    let n = a.get("lines").and_then(|v| v.as_u64()).unwrap_or(40).clamp(1, 400) as usize;
                    let st = run.state.lock().unwrap();
                    let from = st.lines.len().saturating_sub(n);
                    Reply::Json(json!({
                        "device": run.device.label(), "app": run.app_name,
                        "phase": format!("{:?}", st.phase).to_lowercase(), "step": st.step,
                        "seconds": st.started.elapsed().as_secs(),
                        "log": st.lines[from..].iter().map(|(_, t)| t.clone()).collect::<Vec<_>>(),
                    }))
                }
            },
            tool => {
                let open = self.editor.as_ref().map(|e| e.pid);
                match crate::mcp::store_tool(&mut self.store, tool, a) {
                    Some(Ok(mut v)) => {
                        if tool == "list_projects" {
                            v["open_project_id"] = json!(open);
                        }
                        if !matches!(tool, "list_projects" | "get_screen" | "list_blocks" | "get_code") {
                            self.dirty = true;
                            // the open project changed under the editor: nothing stale stays picked
                            if open.is_some() && crate::mcp::project_id(a).ok() == open {
                                if let Some(ed) = self.editor.as_mut() {
                                    if tool == "delete_widget" || tool == "update_widget" {
                                        ed.design.selected = None;
                                        ed.logic = None;
                                    }
                                }
                            }
                            self.notice = Some((format!("{}  {}", ic::ZAP, tool.replace('_', " ")), now + 1.6));
                        }
                        Reply::Json(v)
                    }
                    Some(Err(e)) => Reply::Error(e),
                    None => Reply::Error(format!("Unknown tool {tool}")),
                }
            }
        };
        let _ = c.reply.send(reply);
    }

    /// A preview_screenshot in progress: let the design view settle, take a
    /// picture of the window and cut the phone out of it.
    fn mcp_shot_step(&mut self, ctx: &Context) {
        use crate::mcp::Reply;
        let Some(shot) = self.mcp_shot.as_mut() else { return };
        shot.frames += 1;
        ctx.request_repaint();
        if shot.frames >= 6 && !shot.sent {
            shot.sent = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot);
        }
        let img = ctx.input(|i| i.raw.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
        let reply = match img {
            Some(img) => {
                let shot = self.mcp_shot.take().unwrap();
                let phone_rect = ctx.data(|d| d.get_temp::<Rect>(Id::new("design_phone_on_window")));
                match phone_rect.and_then(|r| phone_png(&img, r, ctx.pixels_per_point())) {
                    Some(png) => (shot.reply, Reply::Image { png, note: shot.note }),
                    None => (shot.reply, Reply::Error("The design view wasn't showing, so there was no phone to take a picture of".into())),
                }
            }
            None if shot.frames > 240 => {
                let shot = self.mcp_shot.take().unwrap();
                (shot.reply, Reply::Error("No picture came back from the window (is it minimised?)".into()))
            }
            None => return,
        };
        let _ = reply.0.send(reply.1);
    }

    fn editor_page(&mut self, ui: &mut Ui) {
        let now = ui.input(|i| i.time);
        let Some(mut ed) = self.editor.take() else { return };
        if std::mem::take(&mut ed.want_export) {
            self.open_export(ed.pid);
        }
        let Some(pi) = self.store.projects.iter().position(|p| p.id == ed.pid) else { return };
        let mut close_editor = false;
        let mut want_leave = std::mem::take(&mut ed.close_req);
        let mut changed = false;
        let auto_save = self.store.settings.auto_save;
        let project = &mut self.store.projects[pi];
        if ed.opened.is_none() {
            ed.opened = Some(crate::model::project_snapshot(project));
        }
        if ed.screen >= project.screens.len() {
            ed.screen = 0;
        }
        phone::sync_fonts(ui.ctx(), project);
        if let Some(d) = device_take_start(&mut ed) {
            ed.device = Some(crate::device::DeviceRun::start(d, project.clone(), ui.ctx().clone()));
            ed.device_shown = true;
        }
        if let Some(d) = build_take_start(&mut ed) {
            ed.device = Some(crate::device::DeviceRun::build(d, project.clone(), ui.ctx().clone()));
            ed.device_shown = true;
        }
        device_watch(&mut ed, now);

        if let Some(tab) = ed.source {
            device_dock(ui, &mut ed);
            source_page(ui, &mut ed, project, tab, now);
            self.editor = Some(ed);
            return;
        }

        // toolbar: where you are on the left, the three tabs in the middle, actions on the right
        let mut start_run = false;
        let mut open_settings = false;
        let in_logic = ed.logic.filter(|&i| i < project.screens[ed.screen].events.len());
        // Esc works like a back key: close the open event first, then
        // step back one tab, and only on the first tab leave the project.
        let quiet = !ed.quit_ask && !ed.add_event && ed.add_comp.is_none() && ed.add_screen.is_none() && ed.asset_delete.is_none() && ed.asset_rename.is_none() && ed.screen_delete.is_none() && ed.screen_rename.is_none();
        if quiet && ui.ctx().memory(|m| m.focused().is_none() && !m.any_popup_open()) && ui.ctx().input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if ed.asset_view.is_some() {
                ed.asset_view = None;
            } else if ed.run.is_some() && in_logic.is_none() && ed.tab == Tab::View {
                ed.run = None;
            } else if in_logic.is_some() && !ed.logic_state.selected.is_empty() {
                ed.logic_state.selected.clear();
            } else if in_logic.is_none() && ed.tab == Tab::View && ed.design.selected.is_some() {
                ed.design.selected = None;
            } else if in_logic.is_some() {
                ed.logic = None;
                ed.logic_state.drag = None;
            } else {
                match ed.tab {
                    Tab::Assets => ed.tab = Tab::Component,
                    Tab::Component => ed.tab = Tab::Event,
                    Tab::Event => ed.tab = Tab::View,
                    // leaving the project is File › Close Project (⌘W), never a stray Esc
                    Tab::View => {}
                }
            }
        }
        // undo history: a snapshot each time the project settles after a change
        // (not mid-drag or while typing, so one drag or one edit is one step)
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        let busy = typing || ed.logic_state.drag.is_some() || ed.design.drag.is_some() || ui.input(|i| i.pointer.any_down());
        if !busy {
            let snap = crate::model::project_snapshot(project);
            match ed.base.take() {
                Some(b) if b != snap => {
                    ed.undo.push(b);
                    if ed.undo.len() > 200 {
                        ed.undo.remove(0);
                    }
                    ed.redo.clear();
                    ed.base = Some(snap);
                }
                Some(b) => ed.base = Some(b),
                None => ed.base = Some(snap),
            }
        }
        // the editor's keyboard shortcuts
        let mut save_now = false;
        if !typing && quiet && !ui.ctx().memory(|m| m.any_popup_open()) {
            let (mut undo, mut redo, mut tab_pick, mut run, mut build, mut export, mut code) = (false, false, None, false, false, false, false);
            ui.ctx().input_mut(|i| {
                redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y);
                undo = i.consume_key(Modifiers::COMMAND, Key::Z);
                for (k, t) in [(Key::Num1, Tab::View), (Key::Num2, Tab::Event), (Key::Num3, Tab::Component), (Key::Num4, Tab::Assets)] {
                    if i.consume_key(Modifiers::COMMAND, k) {
                        tab_pick = Some(t);
                    }
                }
                run = i.consume_key(Modifiers::COMMAND, Key::R);
                build = i.consume_key(Modifiers::COMMAND, Key::B);
                export = i.consume_key(Modifiers::COMMAND, Key::E);
                code = i.consume_key(Modifiers::COMMAND, Key::J);
                save_now = i.consume_key(Modifiers::COMMAND, Key::S);
            });
            let step = if redo { ed.redo.pop().map(|t| (t, false)) } else if undo { ed.undo.pop().map(|t| (t, true)) } else { None };
            if let Some((text, was_undo)) = step {
                if let Ok(mut p) = serde_json::from_str::<Project>(&text) {
                    if let Some(cur) = ed.base.replace(text) {
                        if was_undo {
                            ed.redo.push(cur);
                        } else {
                            ed.undo.push(cur);
                        }
                    }
                    p.modified = now_secs();
                    *project = p;
                    if ed.screen >= project.screens.len() {
                        ed.screen = 0;
                    }
                    changed = true;
                    ed.notice = Some((if was_undo { lbl(ic::UNDO, "Undo") } else { lbl(ic::REDO, "Redo") }, now + 1.0));
                }
            } else if (undo || redo) && ed.notice.is_none() {
                ed.notice = Some((if undo { "Nothing to undo".into() } else { "Nothing to redo".into() }, now + 1.0));
            }
            if let Some(t) = tab_pick {
                ed.tab = t;
                ed.logic = None;
                ed.logic_state.drag = None;
            }
            if run {
                ed.device_start = Some(ed.last_device);
                ed.device_shown = true;
            }
            if build {
                ed.build_start = Some(ed.last_device);
                ed.device_shown = true;
            }
            if export {
                ed.want_export = true;
            }
            if code {
                ed.source = Some(if ed.last_device == crate::device::Device::Iphone { SrcTab::SwScreen } else { SrcTab::KtScreen });
            }
        }
        let in_logic = ed.logic.filter(|&i| i < project.screens[ed.screen].events.len());
        top_bar(ui, |ui| {
            let bar = ui.max_rect();
            // on a Mac the project closes from the menu (File › Close Project),
            // so there's no button to hit by accident; inside an event the
            // arrow only goes back to the events
            if in_logic.is_some() || !crate::menu::NATIVE {
                if bar_button(ui, &lbl(ic::ARROW_LEFT, if in_logic.is_some() { "Events" } else { "Projects" })).on_hover_text("Back").clicked() {
                    if in_logic.is_some() {
                        ed.logic = None;
                        ed.logic_state.drag = None;
                    } else {
                        want_leave = true;
                    }
                }
                ui.label(RichText::new("/").color(crate::theme::gray(180)));
            } else {
                ui.add_space(8.0);
            }
            match in_logic {
                Some(ei) => {
                    let e = &project.screens[ed.screen].events[ei];
                    ui.label(RichText::new(&project.app_name).color(crate::theme::gray(120)));
                    ui.label(RichText::new("/").color(crate::theme::gray(180)));
                    ui.label(RichText::new(format!("{}  ›  {}", project.screens[ed.screen].display_name(), e.title())).size(15.5).strong());
                }
                None => {
                    ui.label(RichText::new(&project.app_name).size(15.5).strong());
                    ui.add_space(10.0);
                    let (pick, del, rename) = screen_picker(ui, project, ed.screen);
                    if del.is_some() {
                        ed.screen_delete = del;
                    }
                    if let Some(i) = rename {
                        ed.screen_rename = Some((i, project.screens[i].name.clone(), String::new()));
                    }
                    if bar_button(ui, &lbl(ic::PLUS, "Screen")).on_hover_text("Add a screen").clicked() {
                        ed.add_screen = Some((String::new(), String::new()));
                    }
                    if let Some(i) = pick {
                        ed.screen = i;
                        ed.design.selected = None;
                    }
                    // the tabs, in the middle of the bar
                    let tabs = [(Tab::View, lbl(ic::LAYOUT, "Design")), (Tab::Event, lbl(ic::ZAP, "Events")), (Tab::Component, lbl(ic::PUZZLE, "Components")), (Tab::Assets, lbl(ic::FOLDER_OPEN, "Assets"))];
                    let names: Vec<&str> = tabs.iter().map(|t| t.1.as_str()).collect();
                    let cur = tabs.iter().position(|t| t.0 == ed.tab).unwrap_or(0);
                    let mid = Rect::from_center_size(bar.center(), vec2(470.0, 30.0));
                    if mid.left() > ui.cursor().left() + 8.0 {
                        let mut c = ui.new_child(UiBuilder::new().max_rect(mid).layout(Layout::left_to_right(Align::Center).with_main_align(Align::Center)));
                        if let Some(i) = designer::segmented(&mut c, &names, cur) {
                            ed.tab = tabs[i].0;
                        }
                    } else if let Some(i) = designer::segmented(ui, &names, cur) {
                        ed.tab = tabs[i].0;
                    }
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                device_menu(ui, &mut ed, true);
                start_run |= debug_button(ui, &mut ed);
                build_menu(ui, &mut ed);
                if plain_button(ui, &lbl(ic::CODE, "Code")).on_hover_text("Show the app's code in Kotlin (Android) or Swift (iOS)  ⌘J").clicked() {
                    ed.source = Some(SrcTab::KtScreen);
                }
                if !crate::menu::NATIVE {
                    ui.add_space(4.0);
                    if bar_button(ui, ic::SETTINGS).on_hover_text("Settings (⌘,)").clicked() {
                        open_settings = true;
                    }
                }
            });
        });
        if open_settings {
            self.settings_open = true;
        }
        // Debug › Blockwise engine: the app plays on the design phone with
        // the debugger docked underneath
        if start_run {
            ed.tab = Tab::View;
            ed.logic = None;
            ed.run = Some(Runtime::starting_on(project.clone(), now, ed.screen, ed.logic_state.breakpoints.clone(), ed.step_mode));
            ed.preview_media.clear();
            ed.debugging = true;
            ed.run_tab = 0;
            ed.tools_open = true;
        }

        // status bar
        {
            let s = &project.screens[ed.screen];
            let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
            let saved = !self.dirty || self.no_save;
            TopBottomPanel::bottom("status").exact_height(26.0).frame(Frame::none().fill(crate::theme::surface()).inner_margin(Margin::symmetric(12.0, 0.0)).stroke(Stroke::new(1.0_f32, line))).show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let small = |t: String| RichText::new(t).size(12.0).color(crate::theme::gray(120));
                    ui.label(small(format!("{}  ·  {} widgets  ·  {} events  ·  {} screens", s.display_name(), s.widgets.len(), s.events.len(), project.screens.len())));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(small(if saved { lbl(ic::CHECK, "Saved") } else { lbl(ic::LOADER, "Saving…") }));
                        ui.add_space(14.0);
                        ui.label(small(format!("{}  ·  v{}", project.package, project.ver_name)));
                    });
                });
            });
        }

        // the app running in the Blockwise engine ticks whatever page is open
        if let Some(rt) = ed.run.as_mut() {
            rt.tick(now);
            ed.preview_media.sync(rt, ui.ctx());
            if rt.has_timers() || !rt.toasts.is_empty() {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
            }
            // a breakpoint brings the debugger up
            if rt.pause.is_some() && !ed.pause_seen {
                ed.debugging = true;
                ed.run_tab = 0;
                ed.tools_open = true;
            }
            ed.pause_seen = rt.pause.is_some();
        }
        // a plain play stops when the phone is out of sight; a debug session
        // keeps going, so breakpoints can be set in the blocks
        if (in_logic.is_some() || ed.tab != Tab::View) && !ed.debugging {
            ed.run = None;
        }
        if ed.run.is_none() {
            ed.debugging = false;
        }
        // docked above the status bar: the debugger, or a real device run's log
        if ed.debugging {
            debug_dock(ui, &mut ed, project, now);
        } else {
            device_dock(ui, &mut ed);
        }
        if let Some(ei) = in_logic {
            CentralPanel::default().frame(Frame::none()).show_inside(ui, |ui| {
                changed |= logic::show(ui, &mut ed.logic_state, project, ed.screen, ei);
            });
        } else {
            // the viewer plays only while its page is open
            if ed.tab != Tab::Assets {
                ed.asset_view = None;
            }
            // and the preview's players only while it runs
            if ed.run.is_none() {
                ed.preview_media.clear();
            }
            match ed.tab {
                Tab::View => {
                    let play = ed.run.as_mut().map(|rt| designer::Play { rt });
                    changed |= designer::show(ui, &mut ed.design, project, ed.screen, play);
                    // play or stop
                    if let Some(on) = ed.design.want_play.take() {
                        ed.run = on.then(|| Runtime::starting_on(project.clone(), now, ed.screen, ed.logic_state.breakpoints.clone(), ed.step_mode));
                        ed.preview_media.clear();
                    }
                }
                Tab::Event => {
                    CentralPanel::default().frame(Frame::none().fill(crate::theme::bg()).inner_margin(Margin::symmetric(16.0, 28.0))).show_inside(ui, |ui| {
                        page_column(ui, |ui| changed |= events_tab(ui, &mut ed, project));
                    });
                }
                Tab::Component => {
                    CentralPanel::default().frame(Frame::none().fill(crate::theme::bg()).inner_margin(Margin::symmetric(16.0, 28.0))).show_inside(ui, |ui| {
                        page_column(ui, |ui| changed |= components_tab(ui, &mut ed, project));
                    });
                }
                Tab::Assets => {
                    CentralPanel::default().frame(Frame::none().fill(crate::theme::bg()).inner_margin(Margin::symmetric(16.0, 28.0))).show_inside(ui, |ui| {
                        page_column(ui, |ui| changed |= assets_tab(ui, &mut ed, project));
                    });
                }
            }
        }
        // files dropped on the window go into the app's assets
        let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            let (added, errors) = add_assets(project, &dropped);
            if !added.is_empty() {
                changed = true;
                ed.notice = Some((format!("Added to Assets: {}", added.join(", ")), now + 3.0));
            }
            ed.asset_msg = errors.join("\n");
            if !errors.is_empty() {
                ed.tab = Tab::Assets;
                ed.logic = None;
            }
        }
        if ui.ctx().input(|i| !i.raw.hovered_files.is_empty()) {
            let r = ui.ctx().screen_rect();
            let p = ui.ctx().layer_painter(LayerId::new(Order::Foreground, Id::new("drop_files")));
            p.rect_filled(r, 0.0, Color32::from_black_alpha(110));
            p.text(r.center(), Align2::CENTER_CENTER, lbl(ic::FILE_PLUS, "Drop to add to the app's assets"), FontId::proportional(22.0), Color32::WHITE);
        }
        changed |= add_screen_dialog(ui.ctx(), &mut ed, project);
        changed |= delete_screen_dialog(ui.ctx(), &mut ed, project);
        changed |= rename_screen_dialog(ui.ctx(), &mut ed, project);
        if let Some((msg, until)) = &ed.notice {
            if *until > now {
                notice(ui.ctx(), msg);
            }
        }
        if changed {
            project.modified = now_secs();
            self.dirty = true;
        }
        if want_leave {
            let differs = ed.opened.as_deref() != Some(crate::model::project_snapshot(project).as_str());
            match crate::model::leave_rule(auto_save, differs) {
                crate::model::Leave::Go => close_editor = true,
                crate::model::Leave::Ask => ed.quit_ask = true,
            }
        }
        if ed.quit_ask {
            // the "Leave project?" question: Don't save drops the changes,
            // Save and leave keeps them, Cancel stays in the project
            Window::new("Leave project?").order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
                ui.label("This project has changes that aren't saved yet.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        ed.quit_ask = false;
                    }
                    if ui.button("Don't save").clicked() {
                        if let Some(old) = ed.opened.as_deref().and_then(|s| serde_json::from_str::<Project>(s).ok()) {
                            let modified = project.modified;
                            *project = old;
                            project.modified = modified;
                        }
                        ed.quit_ask = false;
                        close_editor = true;
                    }
                    if primary_button(ui, "Save and leave").clicked() {
                        ed.quit_ask = false;
                        close_editor = true;
                    }
                });
            });
        }
        if !close_editor {
            self.editor = Some(ed);
        } else {
            // files taken out (or renamed away from) while it was open go now
            project.clean_files();
            self.dirty = true;
        }
        if save_now {
            self.save_project(now);
        }
    }
}

fn notice(ctx: &Context, msg: &str) {
    let screen = ctx.screen_rect();
    Area::new(Id::new("notice")).order(Order::Foreground).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -30.0)).show(ctx, |ui| {
        Frame::none().fill(Color32::from_rgb(0x33, 0x33, 0x33)).rounding(6.0).inner_margin(Margin::symmetric(16.0, 10.0)).show(ui, |ui| {
            ui.set_max_width(screen.width() * 0.7);
            ui.label(RichText::new(msg).color(Color32::WHITE));
        });
    });
}

/// The screen picker in the editor's bar: (screen picked, screen whose x
/// was pressed, screen whose pencil was pressed). Every screen has a pencil
/// to rename it; every one but the first, which the app opens on, an x.
fn screen_picker(ui: &mut Ui, project: &Project, current: usize) -> (Option<usize>, Option<usize>, Option<usize>) {
    let (mut pick, mut del, mut rename) = (None, None, None);
    ComboBox::from_id_salt("screen_pick").selected_text(project.screens[current].display_name()).width(150.0).show_ui(ui, |ui| {
        ui.set_min_width(230.0);
        for (i, s) in project.screens.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.selectable_label(i == current, s.display_name()).clicked() {
                    pick = Some(i);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let icon = |t: &str| Button::new(RichText::new(t).color(crate::theme::gray(130))).frame(false);
                    if i != 0 {
                        if ui.add(icon(ic::X)).on_hover_text(format!("Delete {}…", s.display_name())).clicked() {
                            del = Some(i);
                        }
                    } else {
                        // keeps the pencils in one column
                        ui.add_space(ui.spacing().interact_size.y);
                    }
                    if ui.add(icon(ic::PENCIL)).on_hover_text(format!("Rename {}…", s.display_name())).clicked() {
                        rename = Some(i);
                    }
                });
            });
        }
    });
    (pick, del, rename)
}

/// "Rename Second screen": the pencil beside a screen in the screen picker.
/// The blocks that open the screen follow the new name.
fn rename_screen_dialog(ctx: &Context, ed: &mut Editor, project: &mut Project) -> bool {
    let Some((i, name, err)) = &mut ed.screen_rename else { return false };
    let i = *i;
    if i >= project.screens.len() {
        ed.screen_rename = None;
        return false;
    }
    let (mut close, mut go) = (false, false);
    Window::new(format!("Rename {}", project.screens[i].display_name())).id(Id::new("rename_screen")).order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.label("New name (small letters, digits and _):");
        let field = ui.add(TextEdit::singleline(name).desired_width(240.0));
        if ui.memory(|m| m.focused().is_none()) {
            field.request_focus();
        }
        if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            go = true;
        }
        if !name.trim().is_empty() {
            ui.label(RichText::new(format!("Shown as \"{}\"", Screen::new(name.trim()).display_name())).small().color(Color32::GRAY));
        }
        let opens = project.screen_uses(i).len();
        if opens > 0 {
            ui.label(RichText::new(format!("The blocks that open it ({opens} event{}) follow the new name.", if opens == 1 { "" } else { "s" })).small().color(Color32::GRAY));
        }
        if !err.is_empty() {
            ui.colored_label(Color32::from_rgb(200, 40, 40), err.as_str());
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                close = true;
            }
            if ui.add(Button::new(RichText::new("Rename").color(Color32::WHITE)).fill(PRIMARY)).clicked() {
                go = true;
            }
        });
    });
    if go {
        let wanted = name.clone();
        match project.rename_screen(i, &wanted) {
            Ok(()) => {
                ed.screen_rename = None;
                return true;
            }
            Err(e) => *err = e,
        }
    }
    if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
        ed.screen_rename = None;
    }
    false
}

/// "Delete Second screen?": asked after the x beside a screen in the screen
/// picker. Says what goes with it and which blocks open it.
fn delete_screen_dialog(ctx: &Context, ed: &mut Editor, project: &mut Project) -> bool {
    let Some(i) = ed.screen_delete.filter(|&i| i != 0 && i < project.screens.len()) else {
        ed.screen_delete = None;
        return false;
    };
    let s = &project.screens[i];
    let (name, widgets, events) = (s.display_name(), s.widgets.len(), s.events.len());
    let uses = project.screen_uses(i);
    let mut close = false;
    let mut delete = false;
    Window::new(format!("Delete {name}?")).id(Id::new("delete_screen")).order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        ui.set_max_width(400.0);
        let n = |k: usize, one: &str| format!("{k} {one}{}", if k == 1 { "" } else { "s" });
        ui.label(format!("The screen and everything on it go: {} and {}.", n(widgets, "widget"), n(events, "event")));
        if !uses.is_empty() {
            ui.add_space(4.0);
            ui.label("These blocks open it and will have nowhere to go:");
            for u in &uses {
                ui.label(RichText::new(format!("•  {u}")).color(crate::theme::gray(100)));
            }
        }
        ui.label(RichText::new("⌘Z brings it back.").small().color(Color32::GRAY));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                close = true;
            }
            if ui.add(Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(Color32::from_rgb(200, 40, 40))).clicked() {
                delete = true;
            }
        });
    });
    if delete {
        project.screens.remove(i);
        // stay on the screen that was open, unless it's the one that went
        if ed.screen == i {
            ed.screen = 0;
            ed.logic = None;
            ed.design.selected = None;
        } else if ed.screen > i {
            ed.screen -= 1;
        }
        ed.screen_delete = None;
        return true;
    }
    if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
        ed.screen_delete = None;
    }
    false
}

fn add_screen_dialog(ctx: &Context, ed: &mut Editor, project: &mut Project) -> bool {
    let Some((name, err)) = &mut ed.add_screen else { return false };
    let mut close = false;
    let mut changed = false;
    Window::new("Add a screen").order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.label(RichText::new("Start from").strong());
        ui.add_space(4.0);
        // blank, or a ready-made screen
        let choices: Vec<(Option<&'static str>, &str, &str, &str)> = std::iter::once((None, "Blank", "An empty screen", ic::LAYOUT))
            .chain(crate::templates::TEMPLATES.iter().map(|t| (Some(t.id), t.name, t.description, t.icon)))
            .collect();
        let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
        for row in choices.chunks(2) {
            ui.horizontal(|ui| {
                for (id, title, desc, icon) in row {
                    let picked = ed.add_template == *id;
                    let (r, resp) = ui.allocate_exact_size(vec2(224.0, 64.0), Sense::click());
                    let fill = if picked { ui.visuals().selection.bg_fill } else if resp.hovered() { crate::theme::surface_hover() } else { crate::theme::surface() };
                    ui.painter().rect(r, 8.0, fill, Stroke::new(if picked { 1.5_f32 } else { 1.0 }, if picked { PRIMARY } else { line }));
                    ui.painter().text(r.left_center() + vec2(20.0, 0.0), Align2::CENTER_CENTER, *icon, FontId::proportional(18.0), PRIMARY);
                    ui.painter().text(r.left_top() + vec2(40.0, 14.0), Align2::LEFT_CENTER, *title, FontId::proportional(14.0), crate::theme::ink());
                    let p = ui.painter_at(r.shrink(4.0));
                    let g = p.layout(desc.to_string(), FontId::proportional(11.5), crate::theme::gray(130), r.width() - 50.0);
                    p.galley(r.left_top() + vec2(40.0, 24.0), g, crate::theme::gray(130));
                    if resp.clicked() {
                        // suggest the template's name unless one was typed
                        let old = ed.add_template.and_then(crate::templates::find).map(|t| t.suggested).unwrap_or("");
                        if name.trim().is_empty() || name.trim() == old {
                            *name = id.and_then(crate::templates::find).map(|t| t.suggested.to_string()).unwrap_or_default();
                        }
                        ed.add_template = *id;
                    }
                }
            });
        }
        ui.add_space(8.0);
        ui.label("Name of the screen (small letters, digits and _):");
        ui.add(TextEdit::singleline(name).hint_text("second").desired_width(220.0));
        if !name.trim().is_empty() {
            let preview = Screen::new(name.trim());
            ui.label(RichText::new(format!("Shown as \"{}\"", preview.display_name())).small().color(Color32::GRAY));
        }
        if !err.is_empty() {
            ui.colored_label(Color32::from_rgb(200, 40, 40), err.as_str());
        }
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                close = true;
            }
            if ui.add(Button::new(RichText::new("Add").color(Color32::WHITE)).fill(PRIMARY)).clicked() {
                let n = name.trim().to_string();
                if let Err(e) = check_screen_name(&n) {
                    *err = e;
                } else if project.screens.iter().any(|s| s.name == n || s.activity_name() == activity_name(&n)) {
                    *err = "There is already a screen with that name".into();
                } else {
                    let screens = crate::blocks::screen_choices(project);
                    let made = match ed.add_template.and_then(crate::templates::find) {
                        Some(t) => crate::templates::screen(t, &n, &screens),
                        None => Ok(Screen::new(&n)),
                    };
                    match made {
                        Ok(s) => project.screens.push(s),
                        Err(e) => {
                            *err = format!("Couldn't make that screen: {e}");
                            return;
                        }
                    }
                    ed.screen = project.screens.len() - 1;
                    ed.design.selected = None;
                    changed = true;
                    close = true;
                }
            }
        });
    });
    if close {
        ed.add_screen = None;
        ed.add_template = None;
    }
    changed
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(title).strong().color(crate::theme::gray(90)));
    ui.add_space(4.0);
}

/// A list page's content in a column of reading width, in the middle of the
/// window.
fn page_column(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let avail = ui.available_rect_before_wrap();
    let w = avail.width().min(720.0);
    let col = Rect::from_min_size(pos2(avail.center().x - w / 2.0, avail.top()), vec2(w, avail.height()));
    ui.allocate_new_ui(UiBuilder::new().max_rect(col), add);
}

fn events_tab(ui: &mut Ui, ed: &mut Editor, project: &mut Project) -> bool {
    let mut changed = false;
    let s = &mut project.screens[ed.screen];
    let mut open = None;
    let mut remove = None;
    let row = |ui: &mut Ui, i: usize, e: &Event, color: u32, removable: bool, open: &mut Option<usize>, remove: &mut Option<usize>| {
        let w = ui.available_width().min(720.0);
        let (r, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
        ui.painter().rect_filled(r, 6.0, if resp.hovered() { crate::theme::surface_hover() } else { crate::theme::surface() });
        ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(6.0, r.height())), Rounding { nw: 6.0, sw: 6.0, ne: 0.0, se: 0.0 }, c32(0xFF00_0000 | color));
        ui.painter().text(r.left_center() + vec2(20.0, 0.0), Align2::LEFT_CENTER, e.title(), FontId::proportional(15.0), crate::theme::ink());
        let n = count_blocks(&e.blocks);
        ui.painter().text(r.right_center() - vec2(if removable { 50.0 } else { 16.0 }, 0.0), Align2::RIGHT_CENTER, format!("{n} block{}", if n == 1 { "" } else { "s" }), FontId::proportional(12.0), Color32::GRAY);
        if removable {
            let xr = Rect::from_center_size(r.right_center() - vec2(24.0, 0.0), vec2(24.0, 24.0));
            let xresp = ui.interact(xr, Id::new(("evdel", i)), Sense::click());
            ui.painter().text(xr.center(), Align2::CENTER_CENTER, ic::TRASH, FontId::proportional(15.0), if xresp.hovered() { Color32::from_rgb(200, 40, 40) } else { Color32::GRAY });
            if xresp.on_hover_text("Delete this event").clicked() {
                *remove = Some(i);
                return;
            }
        }
        if resp.clicked() {
            *open = Some(i);
        }
        ui.add_space(6.0);
    };
    ui.horizontal(|ui| {
        ui.heading(format!("Events of the {}", s.display_name().to_lowercase()));
        ui.add_space(12.0);
        if ui.add(Button::new(RichText::new(lbl(ic::PLUS, "Add event")).color(Color32::WHITE)).fill(PRIMARY).rounding(6.0)).clicked() {
            ed.add_event = !ed.add_event;
            ed.event_search.clear();
        }
    });
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        section(ui, "Screen");
        for (i, e) in s.events.iter().enumerate().filter(|(_, e)| e.target.is_empty()) {
            row(ui, i, e, EVENT_COLOR, e.name != "onCreate", &mut open, &mut remove);
        }
        let widget_events: Vec<(usize, &Event)> = s.events.iter().enumerate().filter(|(_, e)| !e.target.is_empty() && e.target != "moreBlock").collect();
        if !widget_events.is_empty() {
            section(ui, "Widgets");
            for (i, e) in widget_events {
                row(ui, i, e, CATEGORIES[6].color, true, &mut open, &mut remove);
            }
        }
        let more: Vec<(usize, &Event)> = s.events.iter().enumerate().filter(|(_, e)| e.target == "moreBlock").collect();
        if !more.is_empty() {
            section(ui, "More Blocks");
            for (i, e) in more {
                row(ui, i, e, CATEGORIES[8].color, true, &mut open, &mut remove);
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Click an event to put blocks in it. Blocks run when the event happens in the app.").color(Color32::GRAY));
    });
    if let Some(i) = remove {
        s.events.remove(i);
        changed = true;
    } else if let Some(i) = open {
        ed.logic = Some(i);
        ed.logic_state = LogicState::with_cat(ed.logic_state.cat);
    }
    if ed.add_event {
        let mut choices: Vec<(String, String, String)> = vec![];
        if !s.events.iter().any(|e| e.target.is_empty() && e.name == "onBackPressed") {
            choices.push((String::new(), "onBackPressed".into(), "When back is pressed".into()));
        }
        // the widget's kind too, so "button" or "text field" finds its events
        let mut kinds: Vec<&str> = vec![""; choices.len()];
        for a in &s.app_bar.actions {
            if !s.app_bar.hidden && !s.events.iter().any(|e| e.target == a.id && e.name == "onClick") {
                choices.push((a.id.clone(), "onClick".into(), format!("When {} {}", a.id, event_words("onClick"))));
                kinds.push("app bar button");
            }
        }
        for w in &s.widgets {
            for ev in w.kind.events() {
                if !s.events.iter().any(|e| e.target == w.id && e.name == *ev) {
                    choices.push((w.id.clone(), ev.to_string(), format!("When {} {}", w.id, event_words(ev))));
                    kinds.push(w.kind.palette_name());
                }
            }
        }
        // every word typed has to be somewhere in the event's words, its
        // widget's name or kind, or the event's own name
        let words: Vec<String> = ed.event_search.to_lowercase().split_whitespace().map(String::from).collect();
        let shown: Vec<usize> = (0..choices.len())
            .filter(|&i| {
                let (target, name, label) = &choices[i];
                let hay = format!("{label} {target} {name} {}", kinds.get(i).copied().unwrap_or("")).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect();
        let mut pick = None;
        let mut open = true;
        Window::new("Add event").order(Order::Foreground).open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
            ui.set_min_width(340.0);
            if choices.is_empty() {
                ui.label("Every event is already added. Add widgets in the View tab to get more.");
                return;
            }
            let search = ui.add(TextEdit::singleline(&mut ed.event_search).id(Id::new("event_search")).hint_text(lbl(ic::SEARCH, "Search events: button, click, text…")).desired_width(f32::INFINITY).margin(vec2(8.0, 6.0)));
            if ui.memory(|m| m.focused().is_none()) {
                search.request_focus();
            }
            // Enter adds the first one found
            if search.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                pick = shown.first().copied();
            }
            ui.add_space(4.0);
            ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                if shown.is_empty() {
                    ui.label(RichText::new("No event matches.").color(Color32::GRAY));
                }
                for (n, &i) in shown.iter().enumerate() {
                    let r = ui.selectable_label(n == 0 && !words.is_empty(), &choices[i].2);
                    if r.clicked() {
                        pick = Some(i);
                    }
                }
            });
        });
        if !open {
            ed.add_event = false;
        }
        if let Some(i) = pick {
            let (target, name, _) = choices[i].clone();
            s.events.push(Event { target, name, blocks: vec![], loose: vec![], params: vec![] });
            ed.add_event = false;
            ed.logic = Some(s.events.len() - 1);
            ed.logic_state = LogicState::with_cat(ed.logic_state.cat);
            changed = true;
        }
    }
    changed
}

fn count_blocks(v: &[Block]) -> usize {
    v.iter()
        .map(|b| 1 + b.args.iter().map(|a| if let Arg::Block(c) = a { count_blocks(std::slice::from_ref(c.as_ref())) } else { 0 }).sum::<usize>() + count_blocks(&b.sub1) + count_blocks(&b.sub2))
        .sum()
}

fn comp_icon(k: CompKind) -> &'static str {
    match k {
        CompKind::Intent => ic::ARROW_LEFT_RIGHT,
        CompKind::SharedPref => ic::DATABASE,
        CompKind::Timer => ic::TIMER,
        CompKind::Dialog => ic::MESSAGE,
        CompKind::Vibrator => ic::VIBRATE,
        CompKind::Calendar => ic::CALENDAR,
        CompKind::Web => ic::GLOBE,
        CompKind::Sound => ic::MUSIC,
        CompKind::Camera => ic::CAMERA,
    }
}

pub(crate) fn file_icon(k: FileKind) -> &'static str {
    match k {
        FileKind::Image => ic::IMAGE,
        FileKind::Sound => ic::MUSIC,
        FileKind::Font => ic::TYPE,
        FileKind::Data => ic::FILE_TEXT,
        FileKind::Video => ic::FILM,
    }
}

/// What each kind of asset is for, shown while there are none.
fn file_kind_help(k: FileKind) -> &'static str {
    match k {
        FileKind::Image => "PNG or JPEG. Show one in an Image widget (its Picture), or change it with the \"set picture\" block.",
        FileKind::Sound => "MP3, WAV or M4A. Add a Sound player component, then play them with its blocks.",
        FileKind::Font => "TTF or OTF. Pick one as the Font of a text, button or text field, or change it with the \"set font\" block.",
        FileKind::Data => "TXT, JSON, CSV, XML, HTML or MD. Read one with the \"text of data file\" block (in Text).",
        FileKind::Video => "MP4 (H.264) plays on every phone; M4V and MOV work too. Show one in a Video widget, and play it with the Video blocks (in View).",
    }
}

/// Copies files into the project: the names they got, and what went wrong.
fn add_assets(project: &mut Project, paths: &[std::path::PathBuf]) -> (Vec<String>, Vec<String>) {
    let (mut added, mut errors) = (vec![], vec![]);
    for p in paths {
        match project.add_file(p) {
            Ok(n) => added.push(n),
            Err(e) => errors.push(e),
        }
    }
    (added, errors)
}

pub(crate) fn human_size(n: u64) -> String {
    match n {
        0..=1023 => format!("{n} bytes"),
        1024..=1_048_575 => format!("{:.0} KB", n as f64 / 1024.0),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.0),
    }
}

/// The Assets page: every file the app carries, by kind, with where each is used.
fn assets_tab(ui: &mut Ui, ed: &mut Editor, project: &mut Project) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.heading("Assets");
        ui.add_space(12.0);
        if ui.add(Button::new(RichText::new(lbl(ic::PLUS, "Add files…")).color(Color32::WHITE)).fill(PRIMARY).rounding(6.0)).clicked() {
            ed.asset_pick = true;
        }
    });
    ui.add_space(2.0);
    ui.label(RichText::new("Pictures, sounds, fonts and data files the whole app carries. You can also drop files on the window.").color(Color32::GRAY));
    if !ed.asset_msg.is_empty() {
        ui.add_space(4.0);
        ui.colored_label(Color32::from_rgb(200, 40, 40), &ed.asset_msg);
    }
    let mut rename_to: Option<(String, String)> = None;
    let mut open: Option<ProjFile> = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for kind in FileKind::ALL {
            let files: Vec<ProjFile> = project.files.iter().filter(|f| f.kind == kind).cloned().collect();
            section(ui, &format!("{}  {}{}", file_icon(kind), kind.label(), if files.is_empty() { String::new() } else { format!("  ·  {}", files.len()) }));
            if files.is_empty() {
                ui.label(RichText::new(file_kind_help(kind)).small().color(Color32::GRAY));
                ui.add_space(4.0);
            }
            for f in files {
                let path = project.file_path(&f);
                let size = std::fs::metadata(&path).map(|m| m.len()).ok();
                let uses = project.file_uses(&f.name);
                let w = ui.available_width().min(720.0);
                Frame::none().fill(crate::theme::surface()).rounding(6.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                    ui.set_width(w - 20.0);
                    ui.horizontal(|ui| {
                        // a look at the file: the picture, the font's letters, or the kind's icon
                        let (r, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
                        let tile = c32(0xFF00_0000 | CATEGORIES[if kind == FileKind::Image { 6 } else { 7 }].color);
                        let mut dims = None;
                        match (kind, if kind == FileKind::Image { phone::picture(ui.ctx(), &path) } else { None }) {
                            (FileKind::Image, Some(t)) => {
                                let s = t.size_vec2();
                                dims = Some((s.x as u32, s.y as u32));
                                let k = (r.width() / s.x).min(r.height() / s.y);
                                let img = Rect::from_center_size(r.center(), s * k);
                                ui.painter().rect_filled(r, 8.0, crate::theme::bg());
                                ui.painter().image(t.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            }
                            (FileKind::Font, _) => {
                                ui.painter().rect_filled(r, 8.0, crate::theme::bg());
                                ui.painter().text(r.center(), Align2::CENTER_CENTER, "Aa", FontId::new(24.0, phone::project_font(ui.ctx(), &f.name)), crate::theme::ink());
                            }
                            _ => {
                                ui.painter().rect_filled(r, 8.0, tile);
                                ui.painter().text(r.center(), Align2::CENTER_CENTER, file_icon(kind), FontId::proportional(22.0), Color32::WHITE);
                            }
                        }
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.set_max_width(w - 360.0);
                            let editing = ed.asset_rename.as_ref().is_some_and(|(n, _)| *n == f.name);
                            if editing {
                                let (_, text) = ed.asset_rename.as_mut().unwrap();
                                let id = Id::new(("asset_rename", &f.name));
                                let resp = ui.add(TextEdit::singleline(text).id(id).desired_width(220.0));
                                if !resp.has_focus() && !resp.lost_focus() {
                                    resp.request_focus();
                                }
                                if resp.lost_focus() {
                                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                                        ed.asset_rename = None;
                                    } else {
                                        rename_to = Some((f.name.clone(), text.clone()));
                                    }
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&f.name).strong());
                                    ui.label(RichText::new(format!(".{}", f.ext)).color(Color32::GRAY));
                                });
                            }
                            let mut facts = vec![f.ext.to_uppercase()];
                            if let Some(n) = size {
                                facts.push(human_size(n));
                            } else {
                                facts.push("the file is missing".into());
                            }
                            if let Some((x, y)) = dims {
                                facts.push(format!("{x} × {y} dp"));
                            }
                            if kind == FileKind::Video {
                                if let Some(ms) = phone::video_len(ui.ctx(), &path) {
                                    facts.push(crate::model::clock(ms));
                                }
                            }
                            ui.label(RichText::new(facts.join("  ·  ")).small().color(Color32::GRAY));
                            match kind {
                                FileKind::Font => {
                                    ui.label(RichText::new("The quick brown fox jumps over 0123456789").family(phone::project_font(ui.ctx(), &f.name)).size(15.0));
                                }
                                FileKind::Data => {
                                    let first = std::fs::read(&path).ok().map(|b| String::from_utf8_lossy(&b[..b.len().min(200)]).lines().next().unwrap_or("").to_string()).unwrap_or_default();
                                    if !first.trim().is_empty() {
                                        ui.add(Label::new(RichText::new(first).monospace().size(11.5).color(crate::theme::gray(110))).truncate());
                                    }
                                }
                                _ => {}
                            }
                            let used = match uses.len() {
                                0 => RichText::new("Not used yet").small().color(Color32::GRAY),
                                1 => RichText::new(format!("Used by {}", uses[0])).small().color(PRIMARY),
                                n => RichText::new(format!("Used by {} and {} more", uses[0], n - 1)).small().color(PRIMARY),
                            };
                            let resp = ui.label(used);
                            if uses.len() > 1 {
                                resp.on_hover_text(uses.join("\n"));
                            }
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(lbl(ic::TRASH, "Delete")).clicked() {
                                ed.asset_delete = Some(f.name.clone());
                            }
                            let open_label = if matches!(kind, FileKind::Sound | FileKind::Video) { lbl(ic::PLAY, "Play") } else { lbl(ic::SEARCH, "View") };
                            if ui.add_enabled(size.is_some(), Button::new(open_label)).on_hover_text("Shows it here, in Blockwise").clicked() {
                                open = Some(f.clone());
                            }
                            if ui.button(lbl(ic::PENCIL, "Rename")).on_hover_text("Blocks and widgets that use it follow the new name").clicked() {
                                ed.asset_rename = Some((f.name.clone(), f.name.clone()));
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
        }
    });
    if let Some(f) = open {
        let path = project.file_path(&f);
        ed.asset_view = Some(crate::asset_view::AssetView::new(&f, path));
    }
    if let Some(v) = &mut ed.asset_view {
        // a file renamed or deleted under it: nothing left to show
        if project.file(&v.name).is_none() || !crate::asset_view::show(ui.ctx(), v) {
            ed.asset_view = None;
        }
    }
    if let Some((old, new)) = rename_to {
        ed.asset_rename = None;
        match project.rename_file(&old, &new) {
            Ok(n) => {
                ed.asset_msg.clear();
                if n != old {
                    changed = true;
                }
                if n != new.trim() {
                    ed.asset_msg = format!("Named {n}: names are small letters, digits and _, as the phones' code needs");
                }
            }
            Err(e) => ed.asset_msg = e,
        }
    }
    if let Some(name) = ed.asset_delete.clone() {
        let uses = project.file_uses(&name);
        let kind = project.file(&name).map(|f| f.kind).unwrap_or(FileKind::Data);
        let without = match kind {
            FileKind::Image => "picture",
            FileKind::Font => "font (they go back to the phone's own)",
            FileKind::Video => "video",
            _ => "file",
        };
        let mut close = false;
        Window::new(format!("Delete {name}?")).order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
            ui.set_max_width(380.0);
            if uses.is_empty() {
                ui.label(format!("The {} is taken out of the app. Nothing uses it.", kind.one()));
            } else {
                ui.label(format!("The {} is taken out of the app. These use it and will be left without a {without}:", kind.one()));
                for u in &uses {
                    ui.label(RichText::new(format!("•  {u}")).color(crate::theme::gray(100)));
                }
            }
            ui.label(RichText::new("⌘Z brings it back.").small().color(Color32::GRAY));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                if ui.add(Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(Color32::from_rgb(200, 40, 40))).clicked() {
                    project.remove_file(&name);
                    changed = true;
                    close = true;
                }
            });
        });
        if close || ui.input(|i| i.key_pressed(Key::Escape)) {
            ed.asset_delete = None;
        }
    }
    if std::mem::take(&mut ed.asset_pick) {
        let mut d = rfd::FileDialog::new().set_title("Add files to the app").add_filter("Pictures, sounds, fonts and data files", &asset_exts());
        for k in FileKind::ALL {
            d = d.add_filter(k.label(), k.exts());
        }
        if let Some(paths) = d.pick_files() {
            let (added, errors) = add_assets(project, &paths);
            changed |= !added.is_empty();
            ed.asset_msg = errors.join("\n");
        }
    }
    changed
}

fn components_tab(ui: &mut Ui, ed: &mut Editor, project: &mut Project) -> bool {
    let mut changed = false;
    let s = &mut project.screens[ed.screen];
    ui.horizontal(|ui| {
        ui.heading(format!("Components of the {}", s.display_name().to_lowercase()));
        ui.add_space(12.0);
        if ui.add(Button::new(RichText::new(lbl(ic::PLUS, "Add component")).color(Color32::WHITE)).fill(PRIMARY).rounding(6.0)).clicked() {
            ed.add_comp = Some((CompKind::Intent, String::new(), String::new()));
        }
    });
    ui.add_space(8.0);
    if s.components.is_empty() {
        ui.label(RichText::new("Components are helpers without a look: timers, pop-up dialogs, saved data, moving to another screen. Add one, then use its blocks in the Component category.").color(Color32::GRAY));
    }
    let mut remove = None;
    for (i, c) in s.components.iter().enumerate() {
        let w = ui.available_width().min(720.0);
        Frame::none().fill(crate::theme::surface()).rounding(6.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(w - 20.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                ui.painter().rect_filled(r, 8.0, c32(0xFF00_0000 | CATEGORIES[7].color));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, comp_icon(c.kind), FontId::proportional(18.0), Color32::WHITE);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&c.name).strong());
                    ui.label(RichText::new(format!("{}  ·  {}", c.kind.label(), c.kind.description())).small().color(Color32::GRAY));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(lbl(ic::TRASH, "Delete")).clicked() {
                        remove = Some(i);
                    }
                });
            });
        });
        ui.add_space(6.0);
    }
    if let Some(i) = remove {
        s.components.remove(i);
        changed = true;
    }
    if let Some((kind, name, err)) = &mut ed.add_comp {
        let mut close = false;
        Window::new("Add component").order(Order::Foreground).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ui.ctx(), |ui| {
            ui.set_min_width(360.0);
            for k in CompKind::ALL {
                ui.horizontal(|ui| {
                    if ui.radio(*kind == k, format!("{} {}", comp_icon(k), k.label())).clicked() {
                        *kind = k;
                    }
                    ui.label(RichText::new(k.description()).small().color(Color32::GRAY));
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(TextEdit::singleline(name).hint_text(kind.default_name()));
            });
            if !err.is_empty() {
                ui.colored_label(Color32::from_rgb(200, 40, 40), err.as_str());
            }
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                if ui.add(Button::new(RichText::new("Add").color(Color32::WHITE)).fill(PRIMARY)).clicked() {
                    let mut n = name.trim().to_string();
                    if n.is_empty() {
                        n = kind.default_name().to_string();
                        let mut k = 2;
                        while s.name_taken(&n) {
                            n = format!("{}{}", kind.default_name(), k);
                            k += 1;
                        }
                    }
                    match logic::name_problem(s, &n) {
                        Some(e) => *err = e,
                        None => {
                            s.components.push(Component { name: n, kind: *kind });
                            changed = true;
                            close = true;
                        }
                    }
                }
            });
        });
        if close {
            ed.add_comp = None;
        }
    }
    changed
}

fn src_name(t: SrcTab, s: &Screen) -> (String, String) {
    match t {
        SrcTab::KtScreen => ("Screen code".into(), format!("{}.kt", s.activity_name())),
        SrcTab::KtLayout => ("Screen layout".into(), format!("{}.xml", s.name)),
        SrcTab::KtSettings => ("App settings".into(), "AndroidManifest.xml".into()),
        SrcTab::KtHelpers => ("Helpers".into(), "AppUtil.kt".into()),
        SrcTab::SwScreen => ("Screen code".into(), format!("{}Screen.swift", crate::native::swift_name(s))),
        SrcTab::SwApp => ("App start".into(), "App.swift".into()),
        SrcTab::SwHelpers => ("Helpers".into(), "Helpers.swift".into()),
    }
}

fn source_page(ui: &mut Ui, ed: &mut Editor, project: &Project, tab: SrcTab, now: f64) {
    let s = &project.screens[ed.screen];
    let text = match tab {
        SrcTab::KtScreen => crate::native::kotlin(project, s),
        SrcTab::KtLayout => crate::codegen::xml(s),
        SrcTab::KtSettings => crate::native::kotlin_manifest(project),
        SrcTab::KtHelpers => crate::native::kotlin_util(project),
        SrcTab::SwScreen => crate::native::swift_screen(project, s),
        SrcTab::SwApp => crate::native::swift_app(project),
        SrcTab::SwHelpers => crate::native::swift_helpers(project),
    };
    top_bar(ui, |ui| {
        if bar_button(ui, ic::ARROW_LEFT).on_hover_text("Back").clicked() {
            ed.source = None;
        }
        ui.label(RichText::new(format!("Code  ·  {}", s.display_name())).size(15.5).strong());
        ui.add_space(16.0);
        // the language switch
        ui.visuals_mut().override_text_color = Some(crate::theme::ink());
        let lang = |sw: bool| if sw { "Swift  ·  iOS" } else { "Kotlin  ·  Android" };
        let mut pick = None;
        if !crate::device::ios_tools() {
            ui.label(RichText::new(lang(false)).strong());
        } else { ComboBox::from_id_salt("code_lang").selected_text(RichText::new(lang(tab.swift())).strong()).width(170.0).show_ui(ui, |ui| {
            for sw in [false, true] {
                if ui.selectable_label(tab.swift() == sw, lang(sw)).clicked() && tab.swift() != sw {
                    pick = Some(tab.other_lang());
                }
            }
        }); }
        if let Some(t) = pick {
            ed.source = Some(t);
        }
        ui.visuals_mut().override_text_color = Some(crate::theme::ink());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if primary_button(ui, &lbl(ic::FILE_OUTPUT, "Export…")).on_hover_text("Write the Android (Kotlin) and/or iOS (Swift) project to a folder").clicked() {
                ed.want_export = true;
            }
            if bar_button(ui, &lbl(ic::COPY, "Copy")).clicked() {
                ui.ctx().copy_text(text.clone());
                ed.notice = Some(("Copied".into(), now + 2.0));
            }
            device_menu(ui, ed, false);
            build_menu(ui, ed);
        });
    });
    TopBottomPanel::top("srctabs").exact_height(40.0).frame(Frame::none().fill(crate::theme::surface())).show_inside(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            let files: &[SrcTab] = if tab.swift() { &[SrcTab::SwScreen, SrcTab::SwApp, SrcTab::SwHelpers] } else { &[SrcTab::KtScreen, SrcTab::KtLayout, SrcTab::KtSettings, SrcTab::KtHelpers] };
            for &t in files {
                let (label, file) = src_name(t, s);
                if ui.selectable_label(tab == t, RichText::new(label).size(14.0)).on_hover_text(file).clicked() {
                    ed.source = Some(t);
                }
                ui.add_space(4.0);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                // text size
                ed.code_size = crate::viewer::code_size(ed.code_size);
                ComboBox::from_id_salt("code_size").selected_text(format!("Aa  {}", ed.code_size)).width(70.0).show_ui(ui, |ui| {
                    for z in crate::viewer::CODE_SIZES {
                        ui.selectable_value(&mut ed.code_size, z, format!("{z}"));
                    }
                }).response.on_hover_text("Text size");
                ui.add_space(10.0);
                ui.label(RichText::new(src_name(tab, s).1).size(12.5).monospace().color(Color32::GRAY));
            });
        });
    });
    // code colours and rules, light or dark with Blockwise's theme
    let ct = crate::highlight::theme(crate::theme::dark());
    let xml = tab == SrcTab::KtLayout;
    CentralPanel::default().frame(Frame::none().fill(ct.background).inner_margin(Margin::same(12.0))).show_inside(ui, |ui| {
        let size = ed.code_size as f32;
        let text = text.replace('\t', "    ");
        let cols = crate::highlight::colours(&text, xml, &ct);
        let font = FontId::monospace(size);
        ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let mut at = 0usize;
            for (n, line) in text.split('\n').enumerate() {
                let len = line.chars().count();
                let mut job = text::LayoutJob::default();
                let mut run = String::new();
                let mut run_col = None;
                for (k, ch) in line.chars().enumerate() {
                    let col = cols.get(at + k).copied().flatten();
                    if col != run_col && !run.is_empty() {
                        job.append(&run, 0.0, TextFormat::simple(font.clone(), run_col.unwrap_or(ct.text)));
                        run.clear();
                    }
                    run_col = col;
                    run.push(ch);
                }
                if !run.is_empty() {
                    job.append(&run, 0.0, TextFormat::simple(font.clone(), run_col.unwrap_or(ct.text)));
                }
                at += len + 1;
                let back = ui.painter().add(egui::Shape::Noop);
                let row = ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:>4} ", n + 1)).monospace().size(size).color(ct.line_numbers));
                    ui.label(job);
                });
                // the line under the pointer gets the editor's line-highlight colour
                if row.response.contains_pointer() {
                    let r = Rect::from_min_max(pos2(ui.max_rect().left(), row.response.rect.top()), pos2(ui.max_rect().right(), row.response.rect.bottom()));
                    ui.painter().set(back, egui::Shape::rect_filled(r, 0.0, ct.line_highlight));
                }
            }
        });
    });
    if let Some((msg, until)) = &ed.notice {
        if *until > now {
            notice(ui.ctx(), msg);
        }
    }
}

/// The Build menu: compile an installable Android APK or iPhone app bundle, or export sources.
fn build_menu(ui: &mut Ui, ed: &mut Editor) {
    let building = device_building(ed);
    let build_only = ed.device.as_ref().is_some_and(|d| d.build_only);
    let built_path = ed.device.as_ref().and_then(|d| d.state.lock().unwrap().built_path.clone());
    let has_log = ed.device.is_some();
    let (build_dev, export_src, show_log) = build_menu_ui(ui, building, build_only, built_path.as_deref(), has_log);
    if let Some(d) = build_dev {
        ed.last_device = d;
        ed.build_start = Some(d);
        ed.device_shown = true;
    }
    if export_src {
        ed.want_export = true;
    }
    if show_log {
        ed.device_shown = true;
    }
}

/// The "🔨 Build" menu itself: (device to build, export source files, show the build log).
fn build_menu_ui(
    ui: &mut Ui,
    building: bool,
    build_only: bool,
    built_path: Option<&std::path::Path>,
    has_log: bool,
) -> (Option<crate::device::Device>, bool, bool) {
    use crate::device::Device;
    let mut build_dev = None;
    let mut export_src = false;
    let mut show_log = false;
    let label = if building && build_only { lbl(ic::LOADER, "Building…") } else { lbl(ic::HAMMER, "Build") };
    let w = &mut ui.visuals_mut().widgets;
    for v in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        v.weak_bg_fill = crate::theme::surface();
        v.bg_fill = crate::theme::surface();
        v.bg_stroke = Stroke::new(1.0_f32, crate::theme::gray(205));
        v.rounding = Rounding::same(6.0);
    }
    ui.spacing_mut().button_padding = vec2(10.0, 6.0);
    ui.menu_button(RichText::new(label), |ui| {
        ui.style_mut().visuals = ui.ctx().style().visuals.clone();
        ui.visuals_mut().override_text_color = Some(crate::theme::ink());
        ui.set_min_width(240.0);
        ui.label(RichText::new("Compile an installable app package").small().color(Color32::GRAY));
        for (d, item_label) in [(Device::Android, lbl(ic::PACKAGE, "Android APK (.apk)")), (Device::Iphone, lbl(ic::APPLE, "iOS app (.app)"))].into_iter().filter(|(d, _)| *d == Device::Android || crate::device::ios_tools()) {
            let b = ui.add_enabled(!building, Button::new(item_label)).on_disabled_hover_text("Wait for the current build to finish, or stop it");
            if b.clicked() {
                build_dev = Some(d);
                ui.close_menu();
            }
        }
        ui.separator();
        if ui.button(lbl(ic::FILE_OUTPUT, "Export source…")).on_hover_text("Write the Kotlin (Android) and/or Swift (iOS) project to a folder").clicked() {
            export_src = true;
            ui.close_menu();
        }
        if let Some(p) = built_path {
            ui.separator();
            if ui.button(lbl(ic::FOLDER_OPEN, "Reveal built app in Finder")).on_hover_text(p.display().to_string()).clicked() {
                crate::device::reveal_in_file_manager(p);
                ui.close_menu();
            }
        }
        if has_log {
            ui.separator();
            if ui.button(lbl(ic::TERMINAL, "Show the build log")).clicked() {
                show_log = true;
                ui.close_menu();
            }
        }
    });
    (build_dev, export_src, show_log)
}

/// The Run menu: build the app and run it for real on an emulator or simulator.
/// The Run menu of the editor's bar: a plain run on an emulator or simulator.
fn device_menu(ui: &mut Ui, ed: &mut Editor, primary: bool) {
    let running = ed.device.as_ref().filter(|d| !d.build_only).map(|d| d.device);
    let building = device_building(ed) && ed.device.as_ref().is_some_and(|d| !d.build_only);
    let (start, show) = device_menu_ui(ui, running, building, primary);
    if let Some(d) = start {
        ed.last_device = d;
    }
    if start.is_some() {
        ed.device_start = start;
        ed.device_shown = true;
    }
    if show {
        ed.device_shown = true;
    }
}

/// The Debug button beside Run: opens or closes the debug bar along the
/// bottom. With the app playing on the design phone that's the debugger; with
/// an emulator run, its live log; with nothing running, it starts the app on
/// the design phone (true is returned) with the debugger open.
fn debug_button(ui: &mut Ui, ed: &mut Editor) -> bool {
    let open = if ed.run.is_some() { ed.debugging } else { ed.device.is_some() && ed.device_shown };
    let green = Color32::from_rgb(0x2E, 0x9E, 0x45);
    let bug = RichText::new(lbl(ic::BUG, "Debug")).color(if open { Color32::WHITE } else { green });
    let b = Button::new(bug).fill(if open { green } else { crate::theme::surface() }).stroke(Stroke::new(1.0_f32, if open { green } else { crate::theme::gray(205) })).min_size(vec2(0.0, 30.0)).rounding(6.0);
    let tip = if open { "Close the debug bar" } else { "Open the debug bar: what each block does, the values, breakpoints and stepping. With nothing running, the app starts on the design phone." };
    if !ui.add(b).on_hover_text(tip).clicked() {
        return false;
    }
    if ed.run.is_some() {
        ed.debugging = !ed.debugging;
        ed.run_tab = 0;
        ed.tools_open = true;
        false
    } else if ed.device.is_some() {
        ed.device_shown = !ed.device_shown;
        false
    } else {
        true
    }
}

/// The menu itself: (device to start, show the log).
fn device_menu_ui(ui: &mut Ui, running: Option<crate::device::Device>, building: bool, primary: bool) -> (Option<crate::device::Device>, bool) {
    use crate::device::Device;
    let mut start = None;
    let mut show = false;
    // the Run button reads "Building…" and can't be pressed while a build runs
    let label = match running {
        _ if building => lbl(ic::LOADER, "Building…"),
        Some(d) => lbl(ic::CIRCLE_PLAY, if d == Device::Android { "Running · Android" } else { "Running · iOS" }),
        None => lbl(ic::PLAY, "Run"),
    };
    let label = format!("{label}  {}", ic::CHEVRON_DOWN);
    let (fill, hover, stroke) = if primary { (PRIMARY, PRIMARY_DARK, Stroke::NONE) } else { (crate::theme::surface(), crate::theme::surface_hover(), Stroke::new(1.0_f32, crate::theme::gray(205))) };
    let w = &mut ui.visuals_mut().widgets;
    for v in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        v.weak_bg_fill = fill;
        v.bg_fill = fill;
        v.bg_stroke = stroke;
        v.rounding = Rounding::same(6.0);
    }
    w.hovered.weak_bg_fill = hover;
    w.open.weak_bg_fill = hover;
    ui.spacing_mut().button_padding = vec2(12.0, 6.0);
    let text = if primary { RichText::new(label).color(Color32::WHITE).strong() } else { RichText::new(label) };
    let r = ui.menu_button(text, |ui| {
        // the white button look is for the top bar only, not the menu's items
        ui.style_mut().visuals = ui.ctx().style().visuals.clone();
        ui.visuals_mut().override_text_color = Some(crate::theme::ink());
        ui.set_min_width(230.0);
        ui.label(RichText::new("Build the app and open it").small().color(Color32::GRAY));
        for (d, icon) in [(Device::Android, ic::SMARTPHONE), (Device::Iphone, ic::APPLE)].into_iter().filter(|(d, _)| *d == Device::Android || crate::device::ios_tools()) {
            let b = ui.add_enabled(!building, Button::new(lbl(icon, d.label()))).on_disabled_hover_text("Wait for the build to finish, or stop it");
            if b.clicked() {
                start = Some(d);
                ui.close_menu();
            }
        }
        if running.is_some() {
            ui.separator();
            if ui.button(lbl(ic::TERMINAL, "Show the live log")).clicked() {
                show = true;
                ui.close_menu();
            }
        }
    });
    r.response.on_hover_text("Build the app and open it on an emulator or simulator  ⌘R (the last one picked)");
    (start, show)
}

/// Lets go of the old device/build run when a new one is requested. The app
/// it opened stays open until the new build is installed over it.
fn take_start(slot: &mut Option<crate::device::Device>, device: &mut Option<crate::device::DeviceRun>) -> Option<crate::device::Device> {
    let start = slot.take();
    if start.is_some() {
        if let Some(old) = device.take() {
            old.detach();
        }
    }
    start
}

/// A real device run asked for in a menu: stops the old run and returns the
/// device to start.
/// The first-time Android setup offer: what is missing, where it comes from,
/// how big it is, and the license. True when "Download and set up" is pressed.
fn android_setup_card(ui: &mut Ui, need: &[crate::android_setup::Need]) -> bool {
    use crate::android_setup::{root, size, LICENSE_URL};
    let mut go = false;
    let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
    Frame::none().fill(crate::theme::surface()).stroke(Stroke::new(1.0_f32, line)).rounding(10.0).inner_margin(Margin::same(14.0)).show(ui, |ui| {
        ui.set_max_width(640.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(ic::DOWNLOAD).size(20.0).color(PRIMARY));
            ui.label(RichText::new("Set up Android on this computer").size(15.5).strong());
        });
        ui.add_space(4.0);
        let total: u32 = need.iter().map(|n| n.mb).sum();
        if need.is_empty() {
            ui.label("There is no Android emulator yet. Blockwise can make one from the system image already on this computer.");
        } else {
            ui.label("Building an Android app needs a few tools that aren't on this computer. Blockwise can download them once; after that, apps build and open right away.");
            ui.add_space(6.0);
            Grid::new("android_needs").num_columns(3).spacing(vec2(18.0, 4.0)).show(ui, |ui| {
                for n in need {
                    ui.label(format!("•  {}", n.what));
                    ui.label(RichText::new(n.from).color(crate::theme::gray(130)));
                    ui.label(RichText::new(format!("about {}", size(n.mb))).color(crate::theme::gray(130)));
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new(format!("About {} in all. The Android SDK goes in {}, Java and Gradle in {}. The first build also fetches the Android build libraries.", size(total), crate::device::android_sdk().unwrap_or_else(crate::android_setup::managed_sdk).display(), root().display())).size(12.5).color(crate::theme::gray(120)));
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(RichText::new("Downloading means you accept the ").size(12.5));
            ui.hyperlink_to(RichText::new("Android SDK License Agreement").size(12.5).color(PRIMARY).underline(), LICENSE_URL);
            ui.label(RichText::new(".").size(12.5));
        });
        ui.add_space(8.0);
        let text = if need.is_empty() { lbl(ic::SMARTPHONE, "Make the emulator") } else { lbl(ic::DOWNLOAD, &format!("Accept and download ({})", size(total))) };
        if primary_button(ui, &text).clicked() {
            go = true;
        }
    });
    go
}

/// The phone part of a window picture, as PNG (at most 1100 pixels tall).
fn phone_png(img: &ColorImage, r: Rect, ppp: f32) -> Option<Vec<u8>> {
    let [w, h] = img.size;
    let rgba = image::RgbaImage::from_raw(w as u32, h as u32, img.as_raw().to_vec())?;
    let x0 = ((r.left() * ppp).floor().max(0.0) as u32).min(w as u32);
    let y0 = ((r.top() * ppp).floor().max(0.0) as u32).min(h as u32);
    let x1 = ((r.right() * ppp).ceil().max(0.0) as u32).min(w as u32);
    let y1 = ((r.bottom() * ppp).ceil().max(0.0) as u32).min(h as u32);
    if x1 <= x0 + 10 || y1 <= y0 + 10 {
        return None;
    }
    let mut cut = image::imageops::crop_imm(&rgba, x0, y0, x1 - x0, y1 - y0).to_image();
    if cut.height() > 1100 {
        let k = 1100.0 / cut.height() as f32;
        cut = image::imageops::resize(&cut, (cut.width() as f32 * k).round() as u32, 1100, image::imageops::FilterType::Triangle);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(cut).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
    Some(out)
}

fn device_take_start(ed: &mut Editor) -> Option<crate::device::Device> {
    take_start(&mut ed.device_start, &mut ed.device)
}

/// A standalone build asked for in the Build menu: stops any previous run and
/// returns the device target to build for.
fn build_take_start(ed: &mut Editor) -> Option<crate::device::Device> {
    take_start(&mut ed.build_start, &mut ed.device)
}

/// A real device build is under way (the Run button is locked meanwhile).
fn device_building(ed: &Editor) -> bool {
    ed.device.as_ref().is_some_and(|d| d.state.lock().unwrap().phase == crate::device::Phase::Working)
}

/// Says "Build failed" (or announces a finished standalone build) when a
/// device build changes phase, and brings the tool area up on failure.
fn device_watch(ed: &mut Editor, now: f64) {
    let (phase, built, setup) = match &ed.device {
        Some(d) => {
            let st = d.state.lock().unwrap();
            (Some(st.phase), d.build_only && st.built_path.is_some(), st.setup.is_some())
        }
        None => (None, false, false),
    };
    if phase == Some(crate::device::Phase::Failed) && ed.device_phase != phase {
        // missing tools aren't a failed build: the log offers to set them up
        ed.notice = Some((if setup { "Android needs setting up first" } else { "Build failed" }.into(), now + 3.0));
        ed.device_shown = true;
        ed.run_tab = 2;
        ed.tools_open = true;
    } else if ed.device_phase == Some(crate::device::Phase::Working) && phase == Some(crate::device::Phase::Stopped) && built {
        let label = match ed.device.as_ref().map(|d| d.device) {
            Some(crate::device::Device::Android) => "Built APK",
            Some(crate::device::Device::Iphone) => "Built iOS app",
            None => "Build complete",
        };
        ed.notice = Some((label.into(), now + 4.0));
    }
    ed.device_phase = phase;
}

/// The live log of a real device run or build (status, Stop / Run again,
/// filters, text options and the lines). Returns (device to run/build again,
/// stop pressed).
fn device_log(ui: &mut Ui, run: &crate::device::DeviceRun, filter: &mut [bool; 4], view: &mut crate::viewer::LogView) -> (Option<crate::device::Device>, bool) {
    use crate::device::{LineKind, Phase};
    let mut again = None;
    let mut stop = false;
    let st = run.state.lock().unwrap();
    let (dot, color) = match st.phase {
        Phase::Working => (ic::LOADER, Color32::from_rgb(0xE0, 0x8A, 0x00)),
        Phase::Running => (ic::CIRCLE_PLAY, Color32::from_rgb(0x2E, 0x9E, 0x45)),
        Phase::Stopped if run.build_only && st.built_path.is_some() => (ic::CIRCLE_CHECK, Color32::from_rgb(0x2E, 0x9E, 0x45)),
        Phase::Stopped => (ic::STOP, Color32::GRAY),
        Phase::Failed => (ic::CIRCLE_X, Color32::from_rgb(200, 40, 40)),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(dot).color(color).size(15.0));
        ui.label(RichText::new(format!("{}  ·  {}", run.app_name, run.device.label())).strong());
        ui.label(RichText::new(st.step.lines().next().unwrap_or("")).color(crate::theme::gray(110)));
        if st.phase == Phase::Working {
            ui.spinner();
            ui.label(RichText::new(format!("{}s", st.started.elapsed().as_secs())).color(Color32::GRAY));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // the build log's menu: wrap, fixed-width letters, size
            ui.menu_button(RichText::new(lbl(ic::TYPE, "Text")), |ui| {
                ui.set_min_width(190.0);
                ui.checkbox(&mut view.wrap, "Wrap text");
                ui.checkbox(&mut view.monospace, "Monospaced font");
                ui.horizontal(|ui| {
                    ui.label("Font size");
                    ui.add(egui::DragValue::new(&mut view.size).range(crate::viewer::LOG_SIZE_MIN..=crate::viewer::LOG_SIZE_MAX));
                });
            })
            .response
            .on_hover_text("Log text options");
            if !run.build_only {
                for (i, n) in ["Events", "Blocks", "Values", "Other"].iter().enumerate().rev() {
                    ui.checkbox(&mut filter[i], *n);
                }
                ui.separator();
            }
            if let Some(path) = &st.built_path {
                if ui.button(lbl(ic::FOLDER_OPEN, "Reveal")).on_hover_text(path.display().to_string()).clicked() {
                    crate::device::reveal_in_file_manager(path);
                }
            }
            let busy = matches!(st.phase, Phase::Working | Phase::Running);
            // while building the button reads "Building…" and is locked
            let building = st.phase == Phase::Working;
            let text = if building {
                lbl(ic::LOADER, "Building…")
            } else if run.build_only {
                lbl(ic::HAMMER, "Build again")
            } else if busy {
                lbl(ic::REFRESH, "Build and run again")
            } else {
                lbl(ic::PLAY, "Run again")
            };
            if ui.add_enabled(!building, Button::new(text)).on_disabled_hover_text("Wait for the build to finish, or stop it").clicked() {
                again = Some(run.device);
            }
            if busy && ui.button(lbl(ic::STOP, "Stop")).clicked() {
                stop = true;
            }
        });
    });
    ui.separator();
    // Android tools missing: what Blockwise would download, to agree to
    if let (Phase::Failed, Some(need)) = (st.phase, &st.setup) {
        if android_setup_card(ui, need) {
            crate::android_setup::allow();
            again = Some(run.device);
        }
        ui.add_space(6.0);
    }
    let view = *view;
    let mut area = if view.wrap { ScrollArea::vertical() } else { ScrollArea::both() };
    area = area.id_salt("device_log_lines").stick_to_bottom(true).auto_shrink([false, false]);
    area.show(ui, |ui| {
        if st.lines.is_empty() {
            ui.label(RichText::new("The log shows here: each event that starts, each block that runs and each value that changes.").color(Color32::GRAY));
        }
        for (k, t) in &st.lines {
            let (show, icon, c) = match k {
                LineKind::Step => (true, "›", PRIMARY_DARK),
                LineKind::Event => (filter[0], "▶", Color32::from_rgb(0x4A, 0x6C, 0xD4)),
                LineKind::Block => (filter[1], "·", crate::theme::gray(70)),
                LineKind::Value => (filter[2], "=", Color32::from_rgb(0xCC, 0x6A, 0x10)),
                LineKind::Output => (filter[3], " ", crate::theme::gray(120)),
                LineKind::Problem => (true, "!", Color32::from_rgb(200, 40, 40)),
            };
            if !show {
                continue;
            }
            let indent = if *k == LineKind::Block || *k == LineKind::Value { "    " } else { "" };
            let marked = crate::viewer::mark(t);
            let mut text = RichText::new(format!("{indent}{icon} {t}")).size(view.size as f32).color(marked.unwrap_or(c));
            if view.monospace {
                text = text.monospace();
            }
            if marked.is_some() || *k == LineKind::Step || *k == LineKind::Event {
                text = text.strong();
            }
            let label = Label::new(text).selectable(true);
            ui.add(if view.wrap { label.wrap() } else { label.extend() });
        }
    });
    (again, stop)
}

/// The frame of a tool area docked along the bottom (like an IDE's Run and
/// Debug windows).
fn dock_frame() -> Frame {
    let line = if crate::theme::dark() { Color32::from_rgb(0x33, 0x39, 0x44) } else { Color32::from_rgb(0xDD, 0xE2, 0xEA) };
    Frame::none().fill(crate::theme::surface()).inner_margin(Margin::symmetric(12.0, 6.0)).stroke(Stroke::new(1.0_f32, line))
}

/// A tab of a docked tool area; true when clicked.
fn dock_tab(ui: &mut Ui, on: bool, text: &str) -> bool {
    let t = RichText::new(text).size(13.5);
    let r = ui.add(Button::new(if on { t.strong().color(crate::theme::ink()) } else { t.color(crate::theme::gray(120)) }).frame(false).min_size(vec2(0.0, 26.0)));
    if on {
        let b = r.rect;
        ui.painter().line_segment([pos2(b.left(), b.bottom() + 1.0), pos2(b.right(), b.bottom() + 1.0)], Stroke::new(2.0_f32, PRIMARY));
    }
    r.clicked()
}

/// The editor's "Run" tool area for a real device run, docked at the bottom
/// instead of a floating window. Hidden with ▾; "Show the live log" brings it
/// back.
fn device_dock(ui: &mut Ui, ed: &mut Editor) {
    let Some(run) = &ed.device else { return };
    if !ed.device_shown {
        return;
    }
    let build_only = run.build_only;
    let mut hide = false;
    let mut out = (None, false);
    TopBottomPanel::bottom("device_dock").resizable(true).default_height(260.0).height_range(120.0..=720.0).frame(dock_frame()).show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            dock_tab(ui, true, &if build_only { lbl(ic::HAMMER, "Build") } else { lbl(ic::SMARTPHONE, "Run") });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(Button::new(RichText::new(lbl(ic::CHEVRON_DOWN, "Hide")).color(crate::theme::gray(120))).frame(false)).on_hover_text("Build / Run on device › Show the log brings it back").clicked() {
                    hide = true;
                }
            });
        });
        ui.add_space(2.0);
        out = device_log(ui, run, &mut ed.device_filter, &mut ed.log_view);
    });
    if out.1 {
        run.stop();
    }
    if out.0.is_some() {
        if build_only {
            ed.build_start = out.0;
        } else {
            ed.device_start = out.0;
        }
        ui.ctx().request_repaint();
    }
    if hide {
        ed.device_shown = false;
    }
}

/// The debugger, docked under the editor while the app runs in the Blockwise
/// engine, as in an IDE: Debugger (step controls, what ran, the values), App
/// state and, during a real device run, its log. The app itself plays on the
/// design phone.
fn debug_dock(ui: &mut Ui, ed: &mut Editor, project: &Project, now: f64) {
    let rt = ed.run.as_mut().unwrap();
    // the breakpoints set in the blocks reach the running app at once
    rt.breakpoints = ed.logic_state.breakpoints.clone();
    rt.step_mode = ed.step_mode;
    if ed.run_tab == 2 && ed.device.is_none() {
        ed.run_tab = 0;
    }
    let mut dbg = DebugAction::None;
    let (mut stop, mut restart, mut hide, mut goto) = (false, false, false, None);
    // on the Design tab the Play/Stop button over the phone stops it
    let phone_shown = ed.tab == Tab::View && ed.logic.is_none();
    let mut dev_out = (None, false);
    let panel = TopBottomPanel::bottom("run_tools").frame(dock_frame());
    let panel = if ed.tools_open { panel.resizable(true).default_height(300.0).height_range(150.0..=760.0) } else { panel.resizable(false).exact_height(40.0) };
    panel.show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            let dlabel = if rt.pause.is_some() { format!("{}  {}", lbl(ic::BUG, "Debugger"), ic::CIRCLE_PAUSE) } else { lbl(ic::BUG, "Debugger") };
            if dock_tab(ui, ed.run_tab == 0, &dlabel) {
                ed.run_tab = 0;
                ed.tools_open = true;
            }
            if dock_tab(ui, ed.run_tab == 1, &lbl(ic::CLIPBOARD_LIST, "App state")) {
                ed.run_tab = 1;
                ed.tools_open = true;
            }
            if let Some(d) = &ed.device {
                let label = if d.build_only { lbl(ic::HAMMER, "Build log") } else { lbl(ic::SMARTPHONE, "Device log") };
                if dock_tab(ui, ed.run_tab == 2, &label) {
                    ed.run_tab = 2;
                    ed.tools_open = true;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(Button::new(RichText::new(lbl(ic::CHEVRON_DOWN, "Hide")).color(crate::theme::gray(120))).frame(false)).on_hover_text("Close the debugger (the app keeps playing on the design phone)").clicked() {
                    hide = true;
                }
                let (icon, tip) = if ed.tools_open { ("Fold", "Fold the debugger to its tabs") } else { ("Unfold", "Open the debugger") };
                if ui.add(Button::new(RichText::new(icon).color(crate::theme::gray(120))).frame(false)).on_hover_text(tip).clicked() {
                    ed.tools_open = !ed.tools_open;
                }
                ui.separator();
                if !phone_shown && ui.button(lbl(ic::STOP, "Stop")).on_hover_text("Stop the app").clicked() {
                    stop = true;
                }
                if ui.button(ic::ROTATE_CCW).on_hover_text("Start the app again, with the latest blocks and design").clicked() {
                    restart = true;
                }
                // jump between screens while testing (Back returns to the one before)
                if rt.project.screens.len() > 1 {
                    let names: Vec<String> = rt.project.screens.iter().map(|s| s.display_name()).collect();
                    let cur = rt.screen_idx;
                    egui::ComboBox::from_id_salt("run_screen").selected_text(format!("Screen: {}", names[cur])).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            if ui.selectable_label(i == cur, n).clicked() {
                                goto = Some(i);
                            }
                        }
                    })
                    .response
                    .on_hover_text("Open another screen, as if the app went there");
                }
                ui.label(RichText::new(format!("{}  {}", ic::CIRCLE_PLAY, rt.project.app_name)).color(crate::theme::gray(120)));
            });
        });
        if !ed.tools_open {
            return;
        }
        ui.add_space(2.0);
        match ed.run_tab {
            0 => {
                SidePanel::left("dbg_controls").exact_width(360.0).resizable(false).frame(Frame::none().inner_margin(Margin { left: 0.0, right: 12.0, top: 6.0, bottom: 0.0 })).show_inside(ui, |ui| {
                    ScrollArea::vertical().id_salt("dbg_controls_scroll").auto_shrink([false, false]).show(ui, |ui| {
                        dbg = debug_controls(ui, rt);
                    });
                });
                SidePanel::right("dbg_values").exact_width(300.0).resizable(false).frame(Frame::none().inner_margin(Margin { left: 12.0, right: 0.0, top: 6.0, bottom: 0.0 })).show_inside(ui, |ui| {
                    ui.label(RichText::new("Values right now").strong());
                    ui.add_space(4.0);
                    ScrollArea::vertical().id_salt("dbg_values_scroll").auto_shrink([false, false]).show(ui, |ui| {
                        vars_view(ui, rt);
                    });
                });
                CentralPanel::default().frame(Frame::none().inner_margin(Margin::symmetric(0.0, 6.0))).show_inside(ui, |ui| {
                    trace_view(ui, rt);
                });
            }
            1 => {
                ScrollArea::vertical().id_salt("app_state_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    inside_panel(ui, rt);
                });
            }
            _ => {
                if let Some(run) = &ed.device {
                    dev_out = device_log(ui, run, &mut ed.device_filter, &mut ed.log_view);
                    if dev_out.1 {
                        run.stop();
                    }
                }
            }
        }
    });
    if dev_out.0.is_some() {
        if ed.device.as_ref().is_some_and(|d| d.build_only) {
            ed.build_start = dev_out.0;
        } else {
            ed.device_start = dev_out.0;
        }
    }
    match dbg {
        DebugAction::Step => rt.debug_step(),
        DebugAction::Continue => rt.debug_continue(),
        DebugAction::Finish => rt.debug_finish(),
        DebugAction::None => {}
    }
    if let Some(i) = goto {
        rt.go_to_screen(i);
    }
    if restart {
        let p = project.clone();
        let (bps, step, screen) = (rt.breakpoints.clone(), rt.step_mode, rt.screen_idx);
        *rt = Runtime::starting_on(p, now, screen, bps, step);
        ed.preview_media.clear();
    }
    // the debugger's own changes (breakpoints cleared, step mode) carry back
    ed.logic_state.breakpoints = rt.breakpoints.clone();
    ed.step_mode = rt.step_mode;
    if hide {
        ed.debugging = false;
    }
    if stop {
        ed.run = None;
        ed.debugging = false;
    }
}

/// The running app on its phone, filling the space left: its screen, toasts,
/// dialogs, crash screen and the back button under it. Returns the taps to
/// hand to the app and whether back was pressed; `run_phone_after` does that.
pub(crate) fn run_phone(ui: &mut Ui, rt: &mut Runtime, now: f64, area: &str) -> (PhoneOut, bool) {
    let mut out = PhoneOut::default();
    let mut back = false;
    let total = phone::shell_size(ui.ctx()) + vec2(0.0, 40.0);
    let shake = if rt.vibrating_until > now { ((now * 90.0).sin() * 3.0) as f32 } else { 0.0 };
    let canvas = ui.available_rect_before_wrap().translate(vec2(shake, 0.0));
    let (_, t) = phone::fit_area(ui.ctx(), area, canvas, total, |ui| {
        ui.vertical(|ui| {
            let title = rt.title.clone();
            let look = rt.screen().app_bar.clone();
            let sheet = rt.project.sheet_of(rt.screen());
            let below = match (sheet, rt.back_stack.last()) {
                (Sheet::No, _) | (_, None) => None,
                (_, Some(&i)) => rt.project.screens.get(i).cloned(),
            };
            let mut shell = match &below {
                // a bottom sheet over the screen that opened it, dimmed
                Some(b) => {
                    let t = if b.title.is_empty() { rt.project.app_name.clone() } else { b.title.clone() };
                    let mut sh = phone::shell_begin(ui, &t, &b.app_bar, rt.back_stack.len() > 1);
                    let files: std::collections::HashMap<String, std::path::PathBuf> = rt.project.file_paths().into_iter().map(|(f, p)| (f.name.clone(), p)).collect();
                    phone::draw_still(ui, sh.content, b, &files);
                    let dim = phone::sheet_begin(ui, &mut sh, sheet);
                    if ui.interact(dim, Id::new("sheet_dim"), Sense::click()).clicked() {
                        back = true;
                    }
                    sh
                }
                None => phone::shell_begin(ui, &title, &look, !rt.back_stack.is_empty()),
            };
            let content = shell.content;
            if rt.finished {
                ui.painter().rect_filled(content, 0.0, Color32::from_gray(245));
                ui.painter().text(content.center(), Align2::CENTER_CENTER, "The app has closed", FontId::proportional(16.0), Color32::GRAY);
            } else if let Some(c) = rt.crash_screen().cloned() {
                // the crash screen every generated app restarts into
                ui.painter().rect_filled(content, 0.0, Color32::from_gray(250));
                ui.painter().rect_filled(content, 0.0, Color32::from_black_alpha(110));
                let dr = Rect::from_center_size(content.center(), vec2(content.width() - 40.0, (content.height() * 0.62).min(420.0)));
                ui.painter().rect_filled(dr, 4.0, Color32::WHITE);
                let _ = ui.interact(content, Id::new("crash_block"), Sense::click());
                let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(dr.min + vec2(18.0, 16.0), dr.max - vec2(18.0, 52.0))).layout(Layout::top_down(Align::Min)));
                child.label(RichText::new(crate::runtime::CRASH_TITLE).size(18.0).strong().color(INK));
                child.add_space(8.0);
                ScrollArea::vertical().id_salt("crash_text").auto_shrink([false, false]).show(&mut child, |ui| {
                    ui.add(Label::new(RichText::new(&c.text).size(12.5).monospace().color(Color32::from_gray(70))).selectable(true).wrap());
                });
                let btn = Rect::from_min_max(pos2(dr.left() + 10.0, dr.bottom() - 44.0), dr.max - vec2(10.0, 8.0));
                let mut row = ui.new_child(UiBuilder::new().max_rect(btn).layout(Layout::right_to_left(Align::Center)));
                if row.add(Button::new(RichText::new(crate::runtime::CRASH_BUTTON.to_uppercase()).color(PRIMARY).strong()).frame(false)).clicked() {
                    rt.end_after_crash();
                }
            } else {
                let scr = rt.screen().clone();
                shell.bottom = phone::bottom_color(&scr, &rt.widgets);
                {
                    let files: std::collections::HashMap<String, std::path::PathBuf> = rt.project.file_paths().into_iter().map(|(f, p)| (f.name.clone(), p)).collect();
                    let mut cx = phone::Ctx { screen: &scr, states: &mut rt.widgets, mode: Mode::Run, selected: None, dragging: None, out: &mut out, files: &files };
                    phone::draw_screen(ui, content, &mut cx);
                }
                // toasts
                if let Some((msg, _)) = rt.toasts.last() {
                    let p = ui.painter_at(content);
                    let g = p.layout(msg.clone(), FontId::proportional(14.0), Color32::WHITE, content.width() - 80.0);
                    let r = Rect::from_center_size(pos2(content.center().x, content.bottom() - 60.0), g.size() + vec2(28.0, 16.0));
                    p.rect_filled(r, r.height() / 2.0, Color32::from_rgba_unmultiplied(50, 50, 50, 230));
                    p.galley(r.min + vec2(14.0, 8.0), g, Color32::WHITE);
                }
                // dialog
                if let Some(d) = rt.dialog.clone() {
                    ui.painter().rect_filled(content, 0.0, Color32::from_black_alpha(110));
                    let dr = Rect::from_center_size(content.center(), vec2(content.width() - 50.0, 170.0));
                    let mut child = ui.new_child(UiBuilder::new().max_rect(dr.shrink(18.0)).layout(Layout::top_down(Align::Min)));
                    ui.painter().rect_filled(dr, 4.0, Color32::WHITE);
                    let blocker = ui.interact(content, Id::new("dlg_block"), Sense::click());
                    let _ = blocker;
                    child.label(RichText::new(if d.title.is_empty() { " " } else { &d.title }).size(18.0).strong());
                    child.add_space(8.0);
                    child.label(RichText::new(&d.message).size(14.0).color(Color32::from_gray(80)));
                    let mut press = None;
                    let btn = Rect::from_min_max(pos2(dr.left() + 10.0, dr.bottom() - 44.0), dr.max - vec2(10.0, 8.0));
                    let mut row = ui.new_child(UiBuilder::new().max_rect(btn).layout(Layout::right_to_left(Align::Center)));
                    if let Some((l, _)) = &d.ok {
                        if row.add(Button::new(RichText::new(l.to_uppercase()).color(PRIMARY).strong()).frame(false)).clicked() {
                            press = Some(true);
                        }
                    }
                    if let Some((l, _)) = &d.cancel {
                        if row.add(Button::new(RichText::new(l.to_uppercase()).color(PRIMARY).strong()).frame(false)).clicked() {
                            press = Some(false);
                        }
                    }
                    if d.ok.is_none() && d.cancel.is_none() && blocker.clicked() && rt.pause.is_none() {
                        rt.dialog = None;
                    }
                    if let Some(ok) = press {
                        if rt.pause.is_none() {
                            rt.dialog_button(ok);
                        }
                    }
                }
            }
            if let Some(p) = &rt.pause {
                let r = Rect::from_min_size(content.min, vec2(content.width(), 30.0));
                ui.painter().rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(255, 196, 0, 235));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("{}  Paused before: {}", ic::CIRCLE_PAUSE, short(&p.text, 34)), FontId::proportional(13.0), INK);
                // taps wait while paused
                let _ = ui.interact(content, Id::new("paused_block"), Sense::click());
            }
            // the app bar's buttons and back arrow
            if !rt.finished && rt.crash_screen().is_none() {
                for (id, r) in &shell.actions {
                    if ui.interact(*r, Id::new(("bar_action", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        out.events.push((id.clone(), "onClick".into(), Default::default()));
                    }
                }
                if let Some(r) = shell.back {
                    if ui.interact(r, Id::new("bar_back"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        back = true;
                    }
                }
            }
            phone::shell_end(ui, &shell);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(total.x / 2.0 - 60.0);
                if ui.button(lbl(ic::ARROW_LEFT, "Back button")).on_hover_text("Press the phone's back button").clicked() {
                    back = true;
                }
            });
        });
    });
    // where each widget sits in the window, for anything that taps by name
    let on_window: Vec<(String, Rect)> = out.rects.iter().map(|(id, r, _)| (id.clone(), t.mul_rect(*r))).collect();
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(area).with("rects"), on_window));
    (out, back)
}

/// Gives the app what was done on its phone this frame, and keeps it ticking.
pub(crate) fn run_phone_after(ctx: &egui::Context, rt: &mut Runtime, (out, back): (PhoneOut, bool), now: f64) {
    let paused = rt.pause.is_some();
    for (target, name, locals) in out.events {
        if rt.dialog.is_none() && !paused {
            rt.fire(&target, &name, locals);
        }
    }
    if back && !paused {
        if rt.dialog.is_some() {
            rt.dialog = None;
        } else {
            rt.press_back();
        }
    }
    if rt.has_timers() || !rt.toasts.is_empty() || rt.vibrating_until > now || (rt.crash.is_some() && rt.crash_screen().is_none()) {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

enum DebugAction {
    None,
    Step,
    Continue,
    Finish,
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// Variables, lists, saved data and problems of the running app.
fn inside_panel(ui: &mut Ui, rt: &Runtime) {
    ui.label(RichText::new(format!("Screen: {}", rt.screen().display_name())).color(Color32::GRAY));
    ui.add_space(8.0);
    vars_view(ui, rt);
    if !rt.prefs.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("Saved data").strong());
        for (f, m) in &rt.prefs {
            for (k, v) in m {
                ui.label(RichText::new(format!("{f}: {k} = {v:?}")).monospace());
            }
        }
    }
    if let Some(c) = &rt.clipboard {
        ui.add_space(8.0);
        ui.label(RichText::new(format!("Clipboard: {c:?}")).monospace());
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Problems").strong());
    if rt.log.is_empty() {
        ui.label(RichText::new("none so far").italics().color(Color32::GRAY));
    }
    for l in rt.log.iter().rev().take(40) {
        ui.colored_label(Color32::from_rgb(190, 60, 40), l);
    }
    ui.add_space(12.0);
    ui.label(RichText::new("This is the Blockwise engine: the blocks run on a simulated phone, not a real emulator. Use Run to try the real app.").small().color(Color32::GRAY));
}

fn vars_view(ui: &mut Ui, rt: &Runtime) {
    ui.label(RichText::new("Variables").strong());
    let mut vars: Vec<(String, String)> = rt.vars.iter().map(|(k, v)| (k.clone(), format!("{:?}", v.text()))).collect();
    for (k, m) in &rt.maps {
        vars.push((k.clone(), format!("{} entries", m.len())));
    }
    vars.sort();
    if vars.is_empty() {
        ui.label(RichText::new("none").italics().color(Color32::GRAY));
    }
    for (k, v) in vars {
        ui.horizontal(|ui| {
            ui.label(RichText::new(k).monospace());
            ui.label(RichText::new(v).monospace().color(PRIMARY_DARK));
        });
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Lists").strong());
    let mut lists: Vec<(String, String)> = rt.lists.iter().map(|(k, v)| (k.clone(), format!("[{}]", v.iter().map(|x| x.text()).collect::<Vec<_>>().join(", ")))).collect();
    for (k, v) in &rt.list_maps {
        lists.push((k.clone(), format!("{} maps", v.len())));
    }
    lists.sort();
    if lists.is_empty() {
        ui.label(RichText::new("none").italics().color(Color32::GRAY));
    }
    for (k, v) in lists {
        ui.label(RichText::new(format!("{k} = {v}")).monospace());
    }
}

/// The debugger's left column: pause settings, and where it stopped with the
/// step buttons.
fn debug_controls(ui: &mut Ui, rt: &mut Runtime) -> DebugAction {
    let mut act = DebugAction::None;
    ui.checkbox(&mut rt.step_mode, "Pause at the start of every event");
    ui.horizontal(|ui| {
        let n = rt.breakpoints.len();
        ui.label(RichText::new(format!("{n} breakpoint{}", if n == 1 { "" } else { "s" })).color(crate::theme::gray(90)));
        if n > 0 && ui.small_button("Clear").on_hover_text("Remove every breakpoint").clicked() {
            rt.breakpoints.clear();
        }
    });
    ui.label(RichText::new("Right-click a block in the blocks editor to pause there (red dot).").small().color(Color32::GRAY));
    ui.add_space(6.0);
    if let Some(p) = rt.pause.clone() {
        Frame::none().fill(Color32::from_rgb(0xFF, 0xF4, 0xCC)).stroke(Stroke::new(1.0_f32, Color32::from_rgb(0xE8, 0xB8, 0x2E))).rounding(6.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.visuals_mut().override_text_color = Some(Color32::from_rgb(0x1D, 0x21, 0x29));
            ui.label(RichText::new(lbl(ic::CIRCLE_PAUSE, "Paused")).size(16.0).strong());
            ui.label(RichText::new(&p.label).color(crate::theme::gray(70)));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add(Button::new(RichText::new("Step").strong().color(Color32::WHITE)).fill(PRIMARY)).on_hover_text("Run the next block, then pause again").clicked() {
                    act = DebugAction::Step;
                }
                if ui.button("Continue").on_hover_text("Run on to the next breakpoint").clicked() {
                    act = DebugAction::Continue;
                }
                if ui.button("Run to end").on_hover_text("Finish this event without pausing again").clicked() {
                    act = DebugAction::Finish;
                }
            });
            ui.add_space(6.0);
            ScrollArea::vertical().id_salt("pause_listing").max_height(150.0).show(ui, |ui| {
                for (depth, id, text) in &p.listing {
                    let here = *id == p.block;
                    let bp = rt.breakpoints.contains(id);
                    let mark = if here { "➜ " } else if bp { "● " } else { "   " };
                    let line = format!("{mark}{}{}", "    ".repeat(*depth), text);
                    let mut t = RichText::new(line).monospace().size(12.0);
                    t = if here { t.strong().background_color(Color32::from_rgb(0xFF, 0xE0, 0x7A)).color(INK) } else if bp { t.color(Color32::from_rgb(200, 40, 40)) } else { t.color(crate::theme::gray(60)) };
                    ui.label(t);
                }
            });
        });
        ui.add_space(6.0);
    } else {
        ui.label(RichText::new("Running. Tap things on the phone; what runs shows in the console.").color(crate::theme::gray(90)));
        ui.add_space(6.0);
    }
    act
}

/// The debugger's console: every event, block and change, newest at the bottom.
fn trace_view(ui: &mut Ui, rt: &mut Runtime) {
    use crate::runtime::TraceKind;
    ui.horizontal(|ui| {
        ui.label(RichText::new("What happened").strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("Clear").clicked() {
                rt.clear_trace();
            }
        });
    });
    Frame::none().fill(Color32::from_rgb(0x26, 0x2A, 0x33)).rounding(4.0).inner_margin(Margin::same(8.0)).show(ui, |ui| {
        ScrollArea::vertical().id_salt("trace").auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            if rt.trace.is_empty() {
                ui.label(RichText::new("Nothing yet").italics().color(Color32::GRAY));
            }
            let start = rt.trace.len().saturating_sub(400);
            for l in &rt.trace[start..] {
                let color = match l.kind {
                    TraceKind::Event => Color32::from_rgb(0x7F, 0xC6, 0xF0),
                    TraceKind::Block => Color32::from_rgb(0xE6, 0xE8, 0xEC),
                    TraceKind::Change => Color32::from_rgb(0x9C, 0xE0, 0x8A),
                    TraceKind::Note => Color32::from_rgb(0xFF, 0xD0, 0x5A),
                    TraceKind::Problem => Color32::from_rgb(0xFF, 0x7A, 0x6A),
                };
                let mut t = RichText::new(format!("{:>6.1}s  {}{}", l.time, "  ".repeat(l.depth.saturating_sub(1)), l.text)).monospace().size(11.5).color(color);
                if l.kind == TraceKind::Event {
                    t = t.strong();
                }
                ui.label(t);
            }
        });
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        if self.store.dark != crate::theme::dark() {
            self.store.dark = crate::theme::dark();
            self.dirty = true;
        }
        let preview = if phone::preview_iphone() { "iphone" } else { "android" };
        if self.store.settings.preview != preview {
            self.store.settings.preview = preview.into();
            self.dirty = true;
        }
        let last = self.editor.as_ref().map(|e| e.pid);
        if last.is_some() && self.store.settings.last_project != last {
            self.store.settings.last_project = last;
            self.dirty = true;
        }
        // the menu bar (macOS); its shortcuts arrive here as menu commands
        let cmds = self.menu.as_ref().map(|m| m.poll()).unwrap_or_default();
        for c in cmds {
            self.menu_cmd(ctx, c);
        }
        // AI assistants' tool calls (MCP), run here so they see and change
        // exactly what the window shows
        self.sync_mcp(ctx, now);
        let calls = self.mcp.as_ref().map(|s| s.poll()).unwrap_or_default();
        for c in calls {
            self.mcp_call(ctx, c, now);
        }
        // ⌘, opens and closes the settings page (the menu takes it on macOS); Esc closes it
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Comma)) {
            self.settings_open = !self.settings_open;
        }
        if self.settings_open && ctx.memory(|m| m.focused().is_none()) && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.settings_open = false;
        }
        CentralPanel::default().frame(Frame::none().fill(crate::theme::bg())).show(ctx, |ui| {
            if self.settings_open {
                self.settings_page(ui);
            } else if self.editor.is_some() {
                self.editor_page(ui);
            } else {
                self.projects_page(ui);
            }
        });
        // a menu command that waited for the project to close: run it now, or
        // drop it when the person stayed in the project
        if let Some(c) = self.after_close {
            match &self.editor {
                None => {
                    self.after_close = None;
                    self.menu_cmd(ctx, c);
                }
                Some(ed) if !ed.quit_ask && !ed.close_req => self.after_close = None,
                Some(_) => {}
            }
        }
        if let Some(m) = self.menu.as_mut() {
            let mut list: Vec<&Project> = self.store.projects.iter().collect();
            list.sort_by(|a, b| b.modified.cmp(&a.modified));
            let projects: Vec<(u32, String)> = list.iter().map(|p| (p.id, p.app_name.clone())).collect();
            m.sync(&crate::menu::State { in_project: self.editor.is_some(), auto_save: self.store.settings.auto_save, dark: crate::theme::dark(), iphone: phone::preview_iphone(), projects: &projects });
        }
        self.export_dialog(ctx);
        self.mcp_shot_step(ctx);
        if let Some((msg, until)) = &self.notice {
            if *until > now {
                notice(ctx, msg);
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
        if self.dirty && !self.no_save && now - self.last_save > 1.0 {
            self.store.save();
            self.dirty = false;
            self.last_save = now;
        }
        if self.dirty {
            ctx.request_repaint_after(std::time::Duration::from_millis(1100));
        }
        // --screenshot: draw ~120 frames, save a picture of the window, quit
        if let Some(path) = self.shot.clone() {
            self.frames += 1;
            ctx.request_repaint();
            let want = std::env::var("BLOCKWISE_SHOT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(120u32);
            if self.frames >= want && !self.shot_sent {
                self.shot_sent = true;
                ctx.send_viewport_cmd(ViewportCommand::Screenshot);
            }
            let img = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
            });
            // only the picture asked for above (an MCP preview takes its own)
            if let Some(img) = img.filter(|_| self.shot_sent && self.mcp_shot.is_none()) {
                let p = std::path::Path::new(&path);
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let [w, h] = img.size;
                match image::save_buffer(p, img.as_raw(), w as u32, h as u32, image::ColorType::Rgba8) {
                    Ok(()) => eprintln!("saved screenshot to {}", p.display()),
                    Err(e) => eprintln!("couldn't save screenshot: {e}"),
                }
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if self.frames > want + 480 {
                eprintln!("no screenshot arrived; giving up");
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.no_save {
            self.store.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(shape: &epaint::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            epaint::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }

    /// Opens a menu button drawn by `draw`, then clicks the item whose text
    /// contains `item`; returns the menu's item texts from top to bottom.
    fn open_and_pick(mut draw: impl FnMut(&mut Ui), item: &str) -> Vec<String> {
        let ctx = Context::default();
        let button = std::cell::Cell::new(Rect::NOTHING);
        let mut frame = |events: Vec<egui::Event>| -> Vec<(String, Rect)> {
            let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 700.0))), events, ..Default::default() };
            let out = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    button.set(ui.scope(|ui| draw(ui)).response.rect);
                });
            });
            let mut found = vec![];
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut found));
            found
        };
        let press = |at: Pos2, pressed| egui::Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let click = |frame: &mut dyn FnMut(Vec<egui::Event>) -> Vec<(String, Rect)>, at: Pos2| {
            frame(vec![egui::Event::PointerMoved(at)]);
            frame(vec![press(at, true)]);
            frame(vec![press(at, false)]);
        };
        frame(vec![]);
        click(&mut frame, button.get().center());
        let mut items = frame(vec![]);
        items.retain(|(_, r)| r.top() > button.get().bottom());
        items.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
        if let Some((_, r)) = items.iter().find(|(t, _)| t.contains(item)) {
            click(&mut frame, r.center());
        }
        items.into_iter().map(|(t, _)| t).collect()
    }

    /// Deleting a screen says which blocks open it; the first screen has none.
    #[test]
    fn deleting_a_screen_lists_the_blocks_that_open_it() {
        let mut p = sample_project(601);
        add_second_screen(&mut p);
        let uses = p.screen_uses(1);
        assert_eq!(uses.len(), 1, "{uses:?}");
        assert!(uses[0].starts_with("1 block in main screen"), "{uses:?}");
        assert!(p.screen_uses(0).is_empty());
    }

    /// The screen picker has an x on every screen but the first; pressing it
    /// asks to delete that screen, it doesn't open it.
    #[test]
    fn screen_picker_has_an_x_to_delete() {
        let mut p = sample_project(601);
        add_second_screen(&mut p);
        let mut got = (None, None, None);
        let items = open_and_pick(|ui| { let r = screen_picker(ui, &p, 0); if r != (None, None, None) { got = r } }, ic::X);
        assert_eq!(items.iter().filter(|t| t.as_str() == ic::X).count(), 1, "{items:?}");
        assert_eq!(items.iter().filter(|t| t.as_str() == ic::PENCIL).count(), 2, "{items:?}");
        assert_eq!(got, (None, Some(1), None));
        // the main screen's pencil (the first one) renames it
        let mut got = (None, None, None);
        open_and_pick(|ui| { let r = screen_picker(ui, &p, 0); if r != (None, None, None) { got = r } }, ic::PENCIL);
        assert_eq!(got, (None, None, Some(0)));
    }

    /// A renamed screen keeps working: the blocks that open it follow.
    #[test]
    fn renaming_a_screen_updates_the_blocks_that_open_it() {
        let mut p = sample_project(601);
        add_second_screen(&mut p);
        p.rename_screen(1, " details ").unwrap();
        assert_eq!(p.screens[1].name, "details");
        assert_eq!(p.screens[1].display_name(), "Details screen");
        let opens = |p: &Project| p.screens[0].events[0].blocks.iter().find(|b| b.op == "timerAfter").map(|b| b.sub1[0].args[0].clone());
        assert_eq!(opens(&p), Some(Arg::Text("DetailsActivity".into())));
        assert_eq!(p.screen_uses(1).len(), 1);
        // bad and taken names are refused and change nothing
        assert!(p.rename_screen(1, "Details").is_err());
        assert!(p.rename_screen(1, "main").is_err());
        assert_eq!(p.screens[1].name, "details");
        // renaming to the same name is fine
        p.rename_screen(1, "details").unwrap();
    }

    /// Run is a plain run on an emulator: no engine in its menu.
    #[test]
    fn run_menu_lists_only_the_emulators() {
        let mut picked = (None, false);
        let items = open_and_pick(|ui| picked = device_menu_ui(ui, None, false, true), "Android emulator");
        assert!(items.iter().any(|t| t.contains("Android emulator")), "{items:?}");
        assert!(!items.iter().any(|t| t.contains("engine") || t.contains("preview")), "{items:?}");
        assert_eq!(picked.0, Some(crate::device::Device::Android));
    }
}
