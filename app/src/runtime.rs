//! The offline preview: runs a project's blocks the way the generated Android code would.

use crate::blocks::*;
use crate::model::*;
use std::collections::{HashMap, HashSet};

/// Most block steps one event may run (an estimated safety limit).
/// How long one event may run before it is stopped as an endless loop: after
/// 5 seconds Android says the app isn't responding, so a phone freezes too.
#[cfg(not(test))]
pub const EVENT_SECONDS: f64 = 5.0;
#[cfg(test)]
pub const EVENT_SECONDS: f64 = 0.5;

// When an app crashes: the app restarts into a screen showing the error.
/// The crash screen's dialog title.
pub const CRASH_TITLE: &str = "The app stopped";
/// Its only button, which closes the app.
pub const CRASH_BUTTON: &str = "Close app";
/// Intent extra carrying the stack trace to the crash screen.
pub const CRASH_EXTRA: &str = "error";
/// Time the pretend phone takes to "restart" into the crash screen (estimate).
pub const CRASH_RESTART_S: f64 = 0.3;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Num(f64),
    Str(String),
    Bool(bool),
}

impl Val {
    pub fn num(&self) -> f64 {
        match self {
            Val::Num(n) => *n,
            Val::Str(s) => s.trim().parse().unwrap_or(0.0),
            Val::Bool(b) => *b as i32 as f64,
        }
    }
    pub fn text(&self) -> String {
        match self {
            Val::Num(n) => fmt_num(*n),
            Val::Str(s) => s.clone(),
            Val::Bool(b) => b.to_string(),
        }
    }
    pub fn truth(&self) -> bool {
        match self {
            Val::Bool(b) => *b,
            Val::Num(n) => *n != 0.0,
            Val::Str(s) => s == "true",
        }
    }
}

/// Pixels per dp of the pretend phone (estimated: a common 1080 x 1920 phone).
pub const PHONE_DENSITY: f64 = 3.0;

/// A number as text the way Java's Double.toString writes it (and so the
/// Android app): 7.0, 0.001, 1234567.5, 1.0E7, 1.5E-6, NaN, Infinity.
/// A number with so many decimals, as Android's
/// BigDecimal(Double.toString(x)).setScale(d, HALF_UP) writes it.
pub fn fixed_text(x: f64, decimals: f64) -> String {
    let t = fmt_num(x);
    if !x.is_finite() {
        return t;
    }
    let d = if decimals.is_nan() { 0 } else { decimals.clamp(0.0, 10.0) as usize };
    let (neg, s) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.as_str()),
    };
    let (mant, exp) = match s.split_once('E') {
        Some((m, e)) => (m, e.parse::<i64>().unwrap_or(0)),
        None => (s, 0),
    };
    let dot = mant.find('.').unwrap_or(mant.len()) as i64;
    let mut digits: Vec<u8> = mant.bytes().filter(|c| *c != b'.').map(|c| c - b'0').collect();
    let mut point = dot + exp;
    if point < 0 {
        let mut z = vec![0; (-point) as usize];
        z.extend(digits);
        digits = z;
        point = 0;
    }
    let mut point = point as usize;
    if point > digits.len() {
        digits.resize(point, 0);
    }
    let keep = point + d;
    if digits.len() > keep {
        let up = digits[keep] >= 5;
        digits.truncate(keep);
        if up {
            let mut i = keep;
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    } else {
        digits.resize(keep, 0);
    }
    let text = |v: &[u8]| v.iter().map(|c| (b'0' + c) as char).collect::<String>();
    let mut whole = text(&digits[..point]);
    while whole.len() > 1 && whole.starts_with('0') {
        whole.remove(0);
    }
    if whole.is_empty() {
        whole = "0".into();
    }
    let zero = digits.iter().all(|c| *c == 0);
    format!("{}{}{}", if neg && !zero { "-" } else { "" }, whole, if d > 0 { format!(".{}", text(&digits[point..])) } else { String::new() })
}

pub fn fmt_num(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n < 0.0 { "-Infinity".into() } else { "Infinity".into() };
    }
    if n == 0.0 {
        return if n.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let sign = if n < 0.0 { "-" } else { "" };
    let a = n.abs();
    // the shortest digits that read back as this number, and where the point goes
    let e = format!("{a:e}");
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    if (1e-3..1e7).contains(&a) {
        if exp >= 0 {
            let cut = (exp + 1) as usize;
            let (ip, fp) = if digits.len() > cut { (digits[..cut].to_string(), digits[cut..].to_string()) } else { (format!("{digits}{}", "0".repeat(cut - digits.len())), "0".to_string()) };
            format!("{sign}{ip}.{fp}")
        } else {
            format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
        }
    } else {
        let rest = if digits.len() > 1 { &digits[1..] } else { "0" };
        format!("{sign}{}.{rest}E{exp}", &digits[..1])
    }
}

/// Text positions count UTF-16 units, as on Android (an emoji is 2).
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[derive(Clone, Debug)]
pub struct WState {
    pub text: String,
    pub hint: String,
    pub checked: bool,
    pub progress: i32,
    pub max: i32,
    pub enabled: bool,
    /// 0 visible, 1 invisible, 2 gone
    pub visibility: u8,
    pub bg: u32,
    pub text_color: u32,
    pub text_size: f32,
    pub alpha: f32,
    pub items: Vec<String>,
    pub selected: usize,
    pub url: String,
    /// An image widget's picture (a project file's name).
    pub image: String,
    pub radius: f32,
    pub elevation: f32,
    pub border: f32,
    pub border_color: u32,
    /// The letters' font (a project font file's name, "" for the phone's own).
    pub font: String,
    /// A video widget: its video, whether it plays, plays again and again,
    /// and where it is (ms; the preview shows no moving picture).
    pub video: String,
    pub playing: bool,
    pub looping: bool,
    pub video_pos: f64,
    /// A password field's eye is open: what's typed shows.
    pub reveal: bool,
}

#[derive(Clone, Debug)]
pub struct Dialog {
    pub title: String,
    pub message: String,
    pub ok: Option<(String, Vec<Block>)>,
    pub cancel: Option<(String, Vec<Block>)>,
}

#[derive(Clone, Debug)]
struct TimerJob {
    name: String,
    due: f64,
    every: Option<f64>,
    blocks: Vec<Block>,
}

/// A web request on its way: curl runs on a thread and leaves the answer
/// (answered with a 2xx code, the code or 0, the text) in `result`.
#[derive(Clone)]
struct WebJob {
    name: String,
    ok: Vec<Block>,
    fail: Vec<Block>,
    locals: HashMap<String, Val>,
    result: std::sync::Arc<std::sync::Mutex<Option<WebDone>>>,
}

/// A finished request: answered with a 2xx code, the code or 0, the text,
/// and what it does to the Supabase sign-in.
type WebDone = (bool, f64, String, Option<Session>);

/// What a finished request does to the Supabase sign-in kept by the app.
#[derive(Clone, Debug)]
enum Session {
    /// Keep the session in this sign-in or refresh answer.
    Keep(String),
    /// The sign-in can't be renewed: signed out.
    Forget,
}

const SUPA_NOT_LINKED: &str = "Supabase isn't linked yet: open the Supabase blocks and add your project's URL and key";

/// Percent-encoding for one part of a web address.
fn pct(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// The readable part of a Supabase error answer.
fn supa_error(text: &str) -> String {
    if let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(text) {
        for k in ["msg", "error_description", "message", "error"] {
            if let Some(v) = o.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                return v.to_string();
            }
        }
    }
    text.to_string()
}

/// Send one request with curl, the way the phone's code does: a JSON
/// content type when there is a body and none was set, 30 seconds at most.
fn web_fetch(method: &str, url: &str, headers: &HashMap<String, String>, body: &str) -> (bool, f64, String) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return (false, 0.0, format!("Not a web address: {url}"));
    }
    let mut c = Command::new("curl");
    c.args(["-sS", "-L", "--max-time", "30", "-X", method, "-w", "\n%{http_code}"]);
    for (k, v) in headers {
        c.arg("-H").arg(format!("{k}: {v}"));
    }
    let send = !body.is_empty() && method != "GET";
    if send {
        if !headers.keys().any(|k| k.eq_ignore_ascii_case("content-type")) {
            c.args(["-H", "Content-Type: application/json"]);
        }
        c.args(["--data-binary", "@-"]);
    }
    c.arg("--").arg(url);
    c.stdin(if send { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return (false, 0.0, format!("Could not run curl: {e}")),
    };
    if send {
        if let Some(mut i) = child.stdin.take() {
            let _ = i.write_all(body.as_bytes());
        }
    }
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => return (false, 0.0, e.to_string()),
    };
    if !out.status.success() {
        let e = String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("curl: ").to_string();
        return (false, 0.0, if e.is_empty() { "The request failed".into() } else { e });
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = match text.rfind('\n') {
        Some(i) => (text[..i].to_string(), text[i + 1..].trim().parse::<f64>().unwrap_or(0.0)),
        None => (String::new(), text.trim().parse::<f64>().unwrap_or(0.0)),
    };
    ((200.0..300.0).contains(&code), code, body)
}

/// A JSON value as the text blocks show it: strings as they are, numbers and
/// true/false as text, null as nothing, nested objects and lists as JSON.
fn json_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.is_finite() => format!("{}", f as i64),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn json_object(o: &serde_json::Map<String, serde_json::Value>) -> HashMap<String, String> {
    o.iter().map(|(k, v)| (k.clone(), json_text(v))).collect()
}

fn json_to_map(text: &str) -> HashMap<String, String> {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(serde_json::Value::Object(o)) => json_object(&o),
        _ => HashMap::new(),
    }
}

fn json_to_list_map(text: &str) -> Vec<HashMap<String, String>> {
    let items = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(serde_json::Value::Array(a)) => a,
        Ok(o @ serde_json::Value::Object(_)) => vec![o],
        _ => return vec![],
    };
    items
        .iter()
        .map(|it| match it {
            serde_json::Value::Object(o) => json_object(o),
            other => HashMap::from([("value".to_string(), json_text(other))]),
        })
        .collect()
}

/// A map as JSON, its keys in order so the same map always reads the same.
fn map_json(m: &HashMap<String, String>) -> serde_json::Value {
    let sorted: std::collections::BTreeMap<_, _> = m.iter().collect();
    serde_json::json!(sorted)
}

enum Flow {
    Normal,
    Break,
    Stop,
}

/// What a line of the debug log is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceKind {
    /// An event started (a tap, a timer, the screen opening...).
    Event,
    /// A block ran.
    Block,
    /// A variable got a new value.
    Change,
    /// Paused, resumed, skipped lines.
    Note,
    /// Something went wrong.
    Problem,
}

#[derive(Clone, Debug)]
pub struct TraceLine {
    pub time: f64,
    pub kind: TraceKind,
    pub depth: usize,
    pub text: String,
}

/// Most lines the debug log keeps, and most block lines one event adds.
const TRACE_KEEP: usize = 1500;
const TRACE_PER_EVENT: usize = 300;

/// An event stopped part-way by the debugger. `pre` is the app just before the
/// event started: stepping replays the event from there one block further.
#[derive(Clone)]
pub struct Pause {
    pre: Box<Runtime>,
    blocks: Vec<Block>,
    locals: HashMap<String, Val>,
    pub label: String,
    /// Blocks that already ran in this event.
    pub at: u64,
    /// The block that runs next.
    pub block: u64,
    pub text: String,
    /// The whole event, for showing where it stopped.
    pub listing: Vec<(usize, u64, String)>,
}

#[derive(Clone, Default)]
struct RunCtl {
    stmt: u64,
    stop_at: Option<u64>,
    skip_until: u64,
    hit: Option<(u64, u64)>,
    lines: usize,
}

/// The app crashed: the error text and when the crash screen shows.
#[derive(Clone, Debug)]
pub struct Crash {
    pub text: String,
    pub show_at: f64,
}

/// What a sound player was asked to play: the sound (a project sound, a file
/// path or a web address), a count that goes up at each "play" (the same sound
/// again starts over), again and again, volume (0 to 1), a "go to" (ms) not
/// done yet; and, written back by the real player, where it is and how long it is (ms).
#[derive(Clone, Debug)]
pub struct SoundJob {
    pub source: String,
    pub gen: u64,
    pub looping: bool,
    pub volume: f32,
    pub seek: Option<f64>,
    pub pos: f64,
    pub len: f64,
}

impl Default for SoundJob {
    fn default() -> Self {
        SoundJob { source: String::new(), gen: 0, looping: false, volume: 1.0, seek: None, pos: 0.0, len: 0.0 }
    }
}

#[derive(Clone)]
pub struct Runtime {
    pub project: Project,
    pub screen_idx: usize,
    pub widgets: HashMap<String, WState>,
    pub vars: HashMap<String, Val>,
    pub maps: HashMap<String, HashMap<String, String>>,
    pub lists: HashMap<String, Vec<Val>>,
    pub list_maps: HashMap<String, Vec<HashMap<String, String>>>,
    pub prefs: HashMap<String, HashMap<String, String>>,
    pub intent_target: HashMap<String, String>,
    pub extras: HashMap<String, String>,
    pub pending_extras: HashMap<String, HashMap<String, String>>,
    pub toasts: Vec<(String, f64)>,
    /// Sound players that were started (the preview plays no sound).
    pub playing: HashSet<String>,
    /// Each sound player's sound, for the preview's real player (player::Preview).
    pub sounds: HashMap<String, SoundJob>,
    /// Different for each run (a restart is a new one).
    pub run_id: u64,
    pub dialog: Option<Dialog>,
    pub building_dialogs: HashMap<String, Dialog>,
    pub title: String,
    pub log: Vec<String>,
    pub clipboard: Option<String>,
    pub finished: bool,
    timers: Vec<TimerJob>,
    /// When the running event started, and whether it ran out of time.
    event_start: Option<std::time::Instant>,
    overtime: bool,
    web: Vec<WebJob>,
    /// The headers each web request component sends.
    pub web_headers: HashMap<String, HashMap<String, String>>,
    /// Every value a variable was set to, in order: the same values a debug
    /// build on a phone logs, so the two can be compared.
    pub values: Vec<(String, Val)>,
    /// The Supabase sign-in: access, refresh, email, id.
    pub supa: HashMap<String, String>,
    steps: u64,
    pub now: f64,
    event_locals: HashMap<String, Val>,
    switch_to: Option<usize>,
    /// Screens under the current one, newest last: closing a screen goes back
    /// to the one that opened it, as on a phone.
    pub back_stack: Vec<usize>,
    pub vibrating_until: f64,
    // ---- debugging
    pub trace: Vec<TraceLine>,
    /// Blocks to pause before (set with a right-click in the logic editor).
    pub breakpoints: HashSet<u64>,
    /// Pause before the first block of every event.
    pub step_mode: bool,
    pub pause: Option<Box<Pause>>,
    /// Set when a block throws the way the real app would; the app is dead.
    pub crash: Option<Crash>,
    ctl: RunCtl,
    depth: usize,
    rng: u64,
}

impl Runtime {
    pub fn new(project: Project, now: f64) -> Self {
        Self::with_debug(project, now, HashSet::new(), false)
    }

    /// Start the app on one of its screens, as if it opened there: the
    /// design phone's play button runs the screen being designed.
    pub fn starting_on(project: Project, now: f64, screen: usize, breakpoints: HashSet<u64>, step_mode: bool) -> Self {
        let mut rt = Self::with_debug(project, now, breakpoints, step_mode);
        if screen != 0 && screen < rt.project.screens.len() {
            rt.open_screen(screen);
        }
        rt
    }

    /// Start the app with breakpoints and step mode already set, so the
    /// screen's first event can be paused too.
    pub fn with_debug(project: Project, now: f64, breakpoints: HashSet<u64>, step_mode: bool) -> Self {
        let mut rt = Runtime {
            project,
            screen_idx: 0,
            widgets: HashMap::new(),
            vars: HashMap::new(),
            maps: HashMap::new(),
            lists: HashMap::new(),
            list_maps: HashMap::new(),
            prefs: HashMap::new(),
            intent_target: HashMap::new(),
            extras: HashMap::new(),
            pending_extras: HashMap::new(),
            toasts: vec![],
            playing: HashSet::new(),
            sounds: HashMap::new(),
            run_id: {
                static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
            dialog: None,
            building_dialogs: HashMap::new(),
            title: String::new(),
            log: vec![],
            clipboard: None,
            finished: false,
            timers: vec![],
            event_start: None,
            overtime: false,
            web: vec![],
            web_headers: HashMap::new(),
            supa: HashMap::new(),
            values: vec![],
            steps: 0,
            now,
            event_locals: HashMap::new(),
            switch_to: None,
            back_stack: vec![],
            vibrating_until: 0.0,
            trace: vec![],
            breakpoints,
            step_mode,
            pause: None,
            crash: None,
            ctl: RunCtl::default(),
            depth: 0,
            rng: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(88172645463325252) | 1,
        };
        rt.open_screen(0);
        rt
    }

    pub fn screen(&self) -> &Screen {
        &self.project.screens[self.screen_idx]
    }

    /// Open another screen from the run page's screen menu, on top of this
    /// one (Back returns here).
    pub fn go_to_screen(&mut self, idx: usize) {
        if idx < self.project.screens.len() && idx != self.screen_idx && self.pause.is_none() && self.crash.is_none() {
            self.back_stack.push(self.screen_idx);
            self.extras.clear();
            self.open_screen(idx);
        }
    }

    fn open_screen(&mut self, idx: usize) {
        self.screen_idx = idx.min(self.project.screens.len().saturating_sub(1));
        self.widgets.clear();
        self.vars.clear();
        self.maps.clear();
        self.lists.clear();
        self.list_maps.clear();
        self.timers.clear();
        // a new screen has its own components: old answers have nowhere to go
        self.web.clear();
        self.web_headers.clear();
        self.dialog = None;
        let s = self.project.screens[self.screen_idx].clone();
        self.title = if s.title.is_empty() { self.project.app_name.clone() } else { s.title.clone() };
        for w in &s.widgets {
            self.widgets.insert(w.id.clone(), initial_state(w));
        }
        for v in &s.vars {
            match v.ty {
                VarType::Bool => {
                    self.vars.insert(v.name.clone(), Val::Bool(false));
                }
                VarType::Num => {
                    self.vars.insert(v.name.clone(), Val::Num(0.0));
                }
                VarType::Str => {
                    self.vars.insert(v.name.clone(), Val::Str(String::new()));
                }
                VarType::Map => {
                    self.maps.insert(v.name.clone(), HashMap::new());
                }
            }
        }
        for l in &s.lists {
            match l.ty {
                ListType::Map => {
                    self.list_maps.insert(l.name.clone(), vec![]);
                }
                _ => {
                    self.lists.insert(l.name.clone(), vec![]);
                }
            }
        }
        self.fire("", "onCreate", HashMap::new());
    }

    /// Run the blocks of one event.
    pub fn fire(&mut self, target: &str, name: &str, locals: HashMap<String, Val>) {
        if self.pause.is_some() || self.crash.is_some() {
            return;
        }
        let blocks = self.screen().events.iter().find(|e| e.target == target && e.name == name).map(|e| e.blocks.clone());
        if let Some(blocks) = blocks {
            let mut label = Event { target: target.to_string(), name: name.to_string(), blocks: vec![], loose: vec![], params: vec![] }.title();
            for (k, v) in &locals {
                label.push_str(&format!("  ({k} = {})", v.text()));
            }
            self.run_blocks(&blocks, locals, label);
        }
    }

    fn debugging(&self) -> bool {
        self.step_mode || !self.breakpoints.is_empty()
    }

    fn run_blocks(&mut self, blocks: &[Block], locals: HashMap<String, Val>, label: String) {
        let stop_at = if self.step_mode { Some(0) } else { None };
        self.run_blocks_ctl(blocks, locals, label, stop_at, 0);
    }

    fn run_blocks_ctl(&mut self, blocks: &[Block], locals: HashMap<String, Val>, label: String, stop_at: Option<u64>, skip_until: u64) {
        let pre = if self.debugging() { Some(Box::new(self.clone())) } else { None };
        self.trace_push(TraceKind::Event, 0, format!("▶ {label}"));
        self.steps = 0;
        self.overtime = false;
        self.event_start = Some(std::time::Instant::now());
        self.depth = 0;
        self.ctl = RunCtl { stmt: 0, stop_at, skip_until, hit: None, lines: 0 };
        let saved = std::mem::replace(&mut self.event_locals, locals.clone());
        let _ = self.exec_stack(blocks);
        self.event_locals = saved;
        if let (Some((at, id)), Some(pre)) = (self.ctl.hit.take(), pre) {
            let mut listing = vec![];
            flatten(blocks, 0, &mut listing);
            let text = find_text(blocks, id).unwrap_or_default();
            self.trace_push(TraceKind::Note, 0, format!("⏸ Paused before: {text}"));
            self.pause = Some(Box::new(Pause { pre, blocks: blocks.to_vec(), locals, label, at, block: id, text, listing }));
            return;
        }
        if self.crash.is_some() {
            // the process is gone: nothing else runs
            self.timers.clear();
            self.web.clear();
            self.dialog = None;
            self.switch_to = None;
            return;
        }
        if let Some(idx) = self.switch_to.take() {
            self.open_screen(idx);
        }
    }

    /// A block failed the way Java would throw: the real app dies and
    /// restarts into the crash screen.
    fn crash(&mut self, exception: &str, detail: String, problem: String) {
        self.err(problem);
        if self.crash.is_some() {
            return;
        }
        let who = self.screen().activity_name();
        let text = format!("{exception}: {detail}\n\tat {}.{who}\n\tin {}", self.project.package, self.ctl_label());
        self.trace_push(TraceKind::Problem, 0, format!("💥 The app crashed: {exception}"));
        self.crash = Some(Crash { text, show_at: self.now + CRASH_RESTART_S });
    }

    fn ctl_label(&self) -> String {
        self.trace.iter().rev().find(|l| l.kind == TraceKind::Event).map(|l| l.text.trim_start_matches("▶ ").to_string()).unwrap_or_default()
    }

    /// Is the crash screen up?
    pub fn crash_screen(&self) -> Option<&Crash> {
        self.crash.as_ref().filter(|c| self.now >= c.show_at)
    }

    /// "Close app" on the crash screen.
    pub fn end_after_crash(&mut self) {
        self.finished = true;
    }

    /// Run the paused event again from its start, this time one block further.
    pub fn debug_step(&mut self) {
        self.replay(|at| (Some(at + 1), 0), "Step");
    }

    /// Run the paused event on to the next breakpoint (or its end).
    pub fn debug_continue(&mut self) {
        self.replay(|at| (None, at + 1), "Continue");
    }

    /// Finish the paused event without stopping again, and stop pausing.
    pub fn debug_finish(&mut self) {
        self.step_mode = false;
        let saved = std::mem::take(&mut self.breakpoints);
        self.replay(|_| (None, u64::MAX), "Run to the end");
        self.breakpoints = saved;
    }

    fn replay(&mut self, how: impl Fn(u64) -> (Option<u64>, u64), what: &str) {
        let Some(p) = self.pause.take() else { return };
        let p = *p;
        let (bps, step) = (std::mem::take(&mut self.breakpoints), self.step_mode);
        let mut pre = *p.pre;
        // time kept moving while paused
        let dt = self.now - pre.now;
        pre.now = self.now;
        pre.vibrating_until += dt;
        for t in &mut pre.timers {
            t.due += dt;
        }
        for (_, until) in &mut pre.toasts {
            *until += dt;
        }
        *self = pre;
        self.breakpoints = bps;
        self.step_mode = step;
        self.trace_push(TraceKind::Note, 0, format!("⏵ {what}"));
        let (stop_at, skip) = how(p.at);
        // the replay keeps going even if step mode would stop at the start
        let keep_step = self.step_mode;
        self.step_mode = false;
        let always_debug = self.breakpoints.is_empty() && stop_at.is_some();
        if always_debug {
            // make sure a snapshot is taken so the next step can replay again
            self.step_mode = true;
        }
        self.run_blocks_ctl(&p.blocks, p.locals, p.label, stop_at, skip);
        self.step_mode = keep_step;
    }

    fn trace_push(&mut self, kind: TraceKind, depth: usize, text: String) {
        self.trace.push(TraceLine { time: self.now, kind, depth, text });
        if self.trace.len() > TRACE_KEEP {
            let cut = self.trace.len() - TRACE_KEEP;
            self.trace.drain(..cut);
        }
    }

    pub fn clear_trace(&mut self) {
        self.trace.clear();
    }

    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// Advance timers; call every frame.
    pub fn tick(&mut self, now: f64) {
        if self.crash.is_some() {
            self.now = now;
            self.toasts.clear();
            return;
        }
        if self.pause.is_some() {
            // frozen: timers and toasts wait for the debugger
            let dt = now - self.now;
            for t in &mut self.timers {
                t.due += dt;
            }
            for (_, until) in &mut self.toasts {
                *until += dt;
            }
            self.now = now;
            return;
        }
        self.now = now;
        self.toasts.retain(|(_, until)| *until > now);
        let mut due = vec![];
        for (i, t) in self.timers.iter().enumerate() {
            if t.due <= now {
                due.push(i);
            }
        }
        for i in due.into_iter().rev() {
            let job = self.timers[i].clone();
            match job.every {
                Some(e) => self.timers[i].due = now + e.max(0.016),
                None => {
                    self.timers.remove(i);
                }
            }
            let label = format!("Timer {} fires", job.name);
            self.run_blocks(&job.blocks, HashMap::new(), label);
            if self.pause.is_some() {
                return;
            }
        }
        let mut i = 0;
        while i < self.web.len() {
            let got = self.web[i].result.lock().ok().and_then(|mut r| r.take());
            let Some((ok, code, text, session)) = got else {
                i += 1;
                continue;
            };
            let job = self.web.remove(i);
            match session {
                Some(Session::Keep(t)) => self.supa_keep(&t),
                Some(Session::Forget) => self.supa.clear(),
                None => {}
            }
            let mut locals = job.locals;
            locals.insert("_ok".into(), Val::Bool(ok));
            locals.insert("_code".into(), Val::Num(code));
            locals.insert("_response".into(), Val::Str(text));
            let label = format!("{} {} ({})", job.name, if ok { "answered" } else { "failed" }, code);
            self.run_blocks(if ok { &job.ok } else { &job.fail }, locals, label);
            if self.pause.is_some() || self.crash.is_some() {
                return;
            }
        }
    }

    /// Start a request on a thread; once it answers, `tick` runs the block's
    /// first stack (2xx) or its second.
    fn web_job(&mut self, name: String, b: &Block, work: impl FnOnce() -> WebDone + Send + 'static) {
        let result = std::sync::Arc::new(std::sync::Mutex::new(None));
        let slot = result.clone();
        std::thread::spawn(move || {
            let r = work();
            if let Ok(mut s) = slot.lock() {
                *s = Some(r);
            }
        });
        self.web.push(WebJob { name, ok: b.sub1.clone(), fail: b.sub2.clone(), locals: self.event_locals.clone(), result });
    }

    /// The linked Supabase project's address and key.
    fn supa_link(&self) -> Option<(String, String)> {
        self.project.supabase.as_ref().filter(|s| s.is_set()).map(|s| (s.base(), s.key.trim().to_string()))
    }

    fn supa_headers(&self, key: &str, signed_in: bool) -> HashMap<String, String> {
        let access = self.supa.get("access").cloned().unwrap_or_default();
        let bearer = if signed_in && !access.is_empty() { access } else { key.to_string() };
        HashMap::from([("apikey".to_string(), key.to_string()), ("Authorization".to_string(), format!("Bearer {bearer}"))])
    }

    /// Keep the session in a sign-in, sign-up or refresh answer (a sign-up
    /// that waits for an email confirmation has none).
    fn supa_keep(&mut self, text: &str) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
        let Some(access) = v["access_token"].as_str().filter(|a| !a.is_empty()) else { return };
        let s = |x: &serde_json::Value| x.as_str().unwrap_or("").to_string();
        self.supa = HashMap::from([
            ("access".to_string(), access.to_string()),
            ("refresh".to_string(), s(&v["refresh_token"])),
            ("email".to_string(), s(&v["user"]["email"])),
            ("id".to_string(), s(&v["user"]["id"])),
        ]);
    }

    /// Something will happen later on its own: a timer, or a web answer.
    pub fn has_timers(&self) -> bool {
        !self.timers.is_empty() || !self.web.is_empty()
    }

    pub fn press_back(&mut self) {
        let has = self.screen().events.iter().any(|e| e.target.is_empty() && e.name == "onBackPressed");
        if has {
            self.fire("", "onBackPressed", HashMap::new());
        } else if let Some(prev) = self.back_stack.pop() {
            // back to the screen that opened this one, as on a phone
            self.open_screen(prev);
        } else {
            self.finished = true;
        }
    }

    pub fn dialog_button(&mut self, ok: bool) {
        if let Some(d) = self.dialog.take() {
            let b = if ok { d.ok } else { d.cancel };
            if let Some((l, blocks)) = b {
                self.run_blocks(&blocks, HashMap::new(), format!("Dialog button {l:?} pressed"));
            }
        }
    }

    fn exec_stack(&mut self, blocks: &[Block]) -> Flow {
        self.depth += 1;
        let mut out = Flow::Normal;
        for b in blocks {
            match self.exec(b) {
                Flow::Normal => {}
                f => {
                    out = f;
                    break;
                }
            }
        }
        self.depth -= 1;
        out
    }

    fn step(&mut self) -> bool {
        self.steps += 1;
        if self.overtime {
            return false;
        }
        // the clock is cheap but not free: look every few thousand blocks
        if self.steps % 4096 == 0 && self.event_start.is_some_and(|t| t.elapsed().as_secs_f64() > EVENT_SECONDS) {
            self.overtime = true;
            let m = format!("Stopped: this event ran for more than {EVENT_SECONDS} seconds (an endless loop?). A phone would freeze here too.");
            self.log.push(m.clone());
            self.trace_push(TraceKind::Problem, self.depth, m);
            return false;
        }
        true
    }

    fn txt(&mut self, b: &Block, i: usize) -> String {
        match b.args.get(i) {
            Some(Arg::Text(t)) => t.clone(),
            Some(Arg::Block(r)) => self.eval(r).text(),
            None => String::new(),
        }
    }
    fn num(&mut self, b: &Block, i: usize) -> f64 {
        match b.args.get(i) {
            Some(Arg::Text(t)) => t.trim().parse().unwrap_or(0.0),
            Some(Arg::Block(r)) => self.eval(r).num(),
            None => 0.0,
        }
    }
    fn boolean(&mut self, b: &Block, i: usize) -> bool {
        match b.args.get(i) {
            Some(Arg::Block(r)) => self.eval(r).truth(),
            _ => false,
        }
    }
    /// A player starts a sound from its start ("" stops it).
    fn start_sound(&mut self, player: String, source: String) {
        let j = self.sounds.entry(player.clone()).or_default();
        j.gen += 1;
        j.seek = None;
        j.pos = 0.0;
        j.len = 0.0;
        if source.is_empty() {
            j.source.clear();
            self.playing.remove(&player);
        } else {
            j.source = source;
            self.playing.insert(player);
        }
    }

    /// The length (ms) of the video a video widget shows, from the file.
    fn video_length(&self, widget: &str) -> f64 {
        let Some(name) = self.widgets.get(widget).map(|w| w.video.clone()) else { return 0.0 };
        self.project.file(&name).filter(|f| f.kind == FileKind::Video).and_then(|f| video_duration_ms(&self.project.file_path(f))).unwrap_or(0.0)
    }

    fn name(b: &Block, i: usize) -> String {
        match b.args.get(i) {
            Some(Arg::Text(t)) => t.clone(),
            _ => String::new(),
        }
    }

    fn err(&mut self, m: String) {
        self.trace_push(TraceKind::Problem, self.depth, m.clone());
        if self.log.len() < 200 {
            self.log.push(m);
        }
    }

    fn exec(&mut self, b: &Block) -> Flow {
        if self.crash.is_some() {
            return Flow::Stop;
        }
        // debugger: stop before this block?
        let idx = self.ctl.stmt;
        self.ctl.stmt += 1;
        let stop = match self.ctl.stop_at {
            Some(s) => idx >= s,
            None => idx >= self.ctl.skip_until && self.breakpoints.contains(&b.id),
        };
        if stop && self.ctl.hit.is_none() {
            self.ctl.hit = Some((idx, b.id));
            return Flow::Stop;
        }
        if !self.step() {
            return Flow::Stop;
        }
        if self.ctl.lines < TRACE_PER_EVENT {
            self.trace_push(TraceKind::Block, self.depth, describe(b));
        } else if self.ctl.lines == TRACE_PER_EVENT {
            self.trace_push(TraceKind::Note, self.depth, "… more blocks ran (not listed)".into());
        }
        self.ctl.lines += 1;
        // Work out the plugged-in values first, left to right, as Java does
        // before calling: a value that throws stops the block before it acts.
        // (A while loop's condition is checked again each time round.)
        let resolved;
        let b = if b.op != "whileLoop" && b.args.iter().any(|a| matches!(a, Arg::Block(_))) {
            let mut args = Vec::with_capacity(b.args.len());
            for a in &b.args {
                args.push(match a {
                    Arg::Block(r) => match self.eval(r) {
                        Val::Bool(v) => Arg::Block(Box::new(Block { id: r.id, op: if v { "true" } else { "false" }.into(), args: vec![], sub1: vec![], sub2: vec![] })),
                        v => Arg::Text(v.text()),
                    },
                    t => t.clone(),
                });
                if self.crash.is_some() {
                    return Flow::Stop;
                }
            }
            resolved = Block { id: b.id, op: b.op.clone(), args, sub1: b.sub1.clone(), sub2: b.sub2.clone() };
            &resolved
        } else {
            b
        };
        let flow = self.exec_inner(b);
        if self.crash.is_some() {
            return Flow::Stop;
        }
        if matches!(b.op.as_str(), "setVarBoolean" | "setVarInt" | "setVarString" | "increaseInt" | "decreaseInt") {
            let n = Self::name(b, 0);
            let v = self.vars.get(&n).cloned().unwrap_or(Val::Str(String::new()));
            if self.values.len() < 20_000 {
                self.values.push((n.clone(), v.clone()));
            }
            if self.ctl.lines <= TRACE_PER_EVENT {
                self.trace_push(TraceKind::Change, self.depth, format!("{n} is now {:?}", v.text()));
            }
        }
        flow
    }

    fn exec_inner(&mut self, b: &Block) -> Flow {
        let op = b.op.as_str();
        match op {
            "setVarBoolean" => {
                let n = Self::name(b, 0);
                let v = self.boolean(b, 1);
                self.vars.insert(n, Val::Bool(v));
            }
            "setVarInt" => {
                let n = Self::name(b, 0);
                let v = self.num(b, 1);
                self.vars.insert(n, Val::Num(v));
            }
            "setVarString" => {
                let n = Self::name(b, 0);
                let v = self.txt(b, 1);
                self.vars.insert(n, Val::Str(v));
            }
            "increaseInt" | "decreaseInt" => {
                let n = Self::name(b, 0);
                let cur = self.vars.get(&n).map(|v| v.num()).unwrap_or(0.0);
                self.vars.insert(n, Val::Num(if op == "increaseInt" { cur + 1.0 } else { cur - 1.0 }));
            }
            "mapCreateNew" => {
                self.maps.insert(Self::name(b, 0), HashMap::new());
            }
            "mapPut" => {
                let n = Self::name(b, 0);
                let k = self.txt(b, 1);
                let v = self.txt(b, 2);
                self.maps.entry(n).or_default().insert(k, v);
            }
            "mapRemoveKey" => {
                let n = Self::name(b, 0);
                let k = self.txt(b, 1);
                self.maps.entry(n).or_default().remove(&k);
            }
            "mapClear" => {
                self.maps.entry(Self::name(b, 0)).or_default().clear();
            }
            "mapFromJson" => {
                let t = self.txt(b, 1);
                self.maps.insert(Self::name(b, 0), json_to_map(&t));
            }
            "listMapFromJson" => {
                let t = self.txt(b, 1);
                self.list_maps.insert(Self::name(b, 0), json_to_list_map(&t));
            }
            "webSetHeader" => {
                let n = Self::name(b, 0);
                let k = self.txt(b, 1);
                let v = self.txt(b, 2);
                self.web_headers.entry(n).or_default().insert(k, v);
            }
            "webClearHeaders" => {
                self.web_headers.remove(&Self::name(b, 0));
            }
            "webRequest" => {
                let name = Self::name(b, 0);
                let method = Self::name(b, 1);
                let method = if method.is_empty() { "GET".to_string() } else { method.to_uppercase() };
                let url = self.txt(b, 2);
                let body = self.txt(b, 3);
                let headers = self.web_headers.get(&name).cloned().unwrap_or_default();
                self.web_job(name, b, move || {
                    let (ok, code, text) = web_fetch(&method, &url, &headers, &body);
                    (ok, code, text, None)
                });
            }
            "supaSignUp" | "supaSignIn" => {
                let email = self.txt(b, 0);
                let password = self.txt(b, 1);
                let up = op == "supaSignUp";
                let name = if up { "Supabase sign up" } else { "Supabase sign in" }.to_string();
                match self.supa_link() {
                    None => self.web_job(name, b, || (false, 0.0, SUPA_NOT_LINKED.into(), None)),
                    Some((base, key)) => {
                        let url = format!("{base}/auth/v1/{}", if up { "signup" } else { "token?grant_type=password" });
                        let headers = self.supa_headers(&key, false);
                        let body = serde_json::json!({"email": email.trim(), "password": password}).to_string();
                        self.web_job(name, b, move || {
                            let (ok, code, text) = web_fetch("POST", &url, &headers, &body);
                            let keep = if ok { Some(Session::Keep(text.clone())) } else { None };
                            (ok, code, text, keep)
                        });
                    }
                }
            }
            "supaSignOut" => {
                let signed_in = self.supa.get("access").is_some_and(|a| !a.is_empty());
                if let (Some((base, key)), true) = (self.supa_link(), signed_in) {
                    let headers = self.supa_headers(&key, true);
                    std::thread::spawn(move || {
                        let _ = web_fetch("POST", &format!("{base}/auth/v1/logout"), &headers, "");
                    });
                }
                self.supa.clear();
            }
            "supaSelect" | "supaSelectWhere" | "supaInsert" | "supaUpdate" | "supaDelete" => {
                let map_text = |rt: &Self, n: String| rt.maps.get(&n).map(|m| map_json(m).to_string()).unwrap_or_else(|| "{}".into());
                let (method, table, col, val, body) = match op {
                    "supaSelect" => ("GET", self.txt(b, 0), String::new(), String::new(), String::new()),
                    "supaSelectWhere" => ("GET", self.txt(b, 0), self.txt(b, 1), self.txt(b, 2), String::new()),
                    "supaInsert" => ("POST", self.txt(b, 1), String::new(), String::new(), map_text(self, Self::name(b, 0))),
                    "supaUpdate" => ("PATCH", self.txt(b, 0), self.txt(b, 1), self.txt(b, 2), map_text(self, Self::name(b, 3))),
                    _ => ("DELETE", self.txt(b, 0), self.txt(b, 1), self.txt(b, 2), String::new()),
                };
                let name = format!("Supabase {} {}", match method { "GET" => "get rows of", "POST" => "add to", "PATCH" => "change", _ => "delete from" }, table.trim());
                match self.supa_link() {
                    None => self.web_job(name, b, || (false, 0.0, SUPA_NOT_LINKED.into(), None)),
                    Some((base, key)) => {
                        let mut url = format!("{base}/rest/v1/{}", pct(table.trim()));
                        let mut query = vec![];
                        if method == "GET" {
                            query.push("select=*".to_string());
                        }
                        if !col.trim().is_empty() {
                            query.push(format!("{}=eq.{}", pct(col.trim()), pct(&val)));
                        }
                        if !query.is_empty() {
                            url = format!("{url}?{}", query.join("&"));
                        }
                        let mut headers = self.supa_headers(&key, true);
                        headers.insert("Prefer".into(), "return=representation".into());
                        let anon = self.supa_headers(&key, false);
                        let refresh = self.supa.get("refresh").cloned().unwrap_or_default();
                        self.web_job(name, b, move || {
                            let (ok, code, text) = web_fetch(method, &url, &headers, &body);
                            if code != 401.0 || refresh.is_empty() {
                                return (ok, code, text, None);
                            }
                            // the sign-in expired: renew it and send again, as the phone does
                            let rb = serde_json::json!({"refresh_token": refresh}).to_string();
                            let (rok, _, rtext) = web_fetch("POST", &format!("{base}/auth/v1/token?grant_type=refresh_token"), &anon, &rb);
                            if !rok {
                                return (ok, code, text, Some(Session::Forget));
                            }
                            let access = serde_json::from_str::<serde_json::Value>(&rtext).ok().and_then(|v| v["access_token"].as_str().map(String::from)).unwrap_or_default();
                            headers.insert("Authorization".into(), format!("Bearer {access}"));
                            let (ok, code, text) = web_fetch(method, &url, &headers, &body);
                            (ok, code, text, Some(Session::Keep(rtext)))
                        });
                    }
                }
            }
            "addListInt" | "addListStr" => {
                let v = if op == "addListInt" { Val::Num(self.num(b, 0)) } else { Val::Str(self.txt(b, 0)) };
                let n = Self::name(b, 1);
                self.lists.entry(n).or_default().push(v);
            }
            "insertListInt" | "insertListStr" => {
                let v = if op == "insertListInt" { Val::Num(self.num(b, 0)) } else { Val::Str(self.txt(b, 0)) };
                let at = self.num(b, 1) as i64;
                let n = Self::name(b, 2);
                let l = self.lists.entry(n.clone()).or_default();
                let len = l.len();
                if at >= 0 && (at as usize) <= len {
                    l.insert(at as usize, v);
                } else {
                    self.crash("java.lang.IndexOutOfBoundsException", format!("Index: {at}, Size: {len}"), format!("insert at {at}: outside list {n}"));
                }
            }
            "addMapToList" => {
                let m = self.maps.get(&Self::name(b, 0)).cloned().unwrap_or_default();
                self.list_maps.entry(Self::name(b, 1)).or_default().push(m);
            }
            "getMapInList" => {
                let at = self.num(b, 0) as i64;
                let l = Self::name(b, 1);
                let target = Self::name(b, 2);
                let got = self.list_maps.get(&l).and_then(|v| if at >= 0 { v.get(at as usize).cloned() } else { None });
                match got {
                    Some(m) => {
                        self.maps.insert(target, m);
                    }
                    None => {
                        let len = self.list_maps.get(&l).map(|v| v.len()).unwrap_or(0);
                        self.crash("java.lang.IndexOutOfBoundsException", out_of_bounds(at, len), format!("get at {at}: outside list {l}"));
                    }
                }
            }
            "deleteList" => {
                let at = self.num(b, 0) as i64;
                let n = Self::name(b, 1);
                let ok = if let Some(l) = self.lists.get_mut(&n) {
                    if at >= 0 && (at as usize) < l.len() {
                        l.remove(at as usize);
                        true
                    } else {
                        false
                    }
                } else if let Some(l) = self.list_maps.get_mut(&n) {
                    if at >= 0 && (at as usize) < l.len() {
                        l.remove(at as usize);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !ok {
                    let len = self.lists.get(&n).map(|l| l.len()).or_else(|| self.list_maps.get(&n).map(|l| l.len())).unwrap_or(0);
                    self.crash("java.lang.IndexOutOfBoundsException", out_of_bounds(at, len), format!("delete at {at}: outside list {n}"));
                }
            }
            "reverseList" => {
                let n = Self::name(b, 0);
                if let Some(l) = self.lists.get_mut(&n) {
                    l.reverse();
                }
                if let Some(l) = self.list_maps.get_mut(&n) {
                    l.reverse();
                }
            }
            "sortListInt" => {
                let n = Self::name(b, 0);
                if let Some(l) = self.lists.get_mut(&n) {
                    // Java's order: -0.0 before 0.0, NaN last
                    l.sort_by(|a, b| a.num().total_cmp(&b.num()));
                }
            }
            "sortListStr" => {
                let n = Self::name(b, 0);
                if let Some(l) = self.lists.get_mut(&n) {
                    // Java's compareTo: UTF-16 letter by letter
                    l.sort_by(|a, b| a.text().encode_utf16().cmp(b.text().encode_utf16()));
                }
            }
            "splitToList" => {
                let t = self.txt(b, 0);
                let sep = self.txt(b, 1);
                let n = Self::name(b, 2);
                let parts: Vec<Val> = if sep.is_empty() { vec![Val::Str(t)] } else { t.split(sep.as_str()).map(|p| Val::Str(p.to_string())).collect() };
                self.lists.insert(n, parts);
            }
            "hideKeyboard" | "openLink" | "shareText" => {
                // phone things the preview leaves to the phone
                let _ = if b.args.is_empty() { String::new() } else { self.txt(b, 0) };
            }
            "clearList" => {
                let n = Self::name(b, 0);
                if let Some(l) = self.lists.get_mut(&n) {
                    l.clear();
                }
                if let Some(l) = self.list_maps.get_mut(&n) {
                    l.clear();
                }
            }
            "repeat" => {
                let n = self.num(b, 0).max(0.0) as i64;
                for _ in 0..n {
                    match self.exec_stack(&b.sub1) {
                        Flow::Break => break,
                        Flow::Stop => return Flow::Stop,
                        Flow::Normal => {}
                    }
                    if !self.step() {
                        return Flow::Stop;
                    }
                }
            }
            "forever" => loop {
                match self.exec_stack(&b.sub1) {
                    Flow::Break => break,
                    Flow::Stop => return Flow::Stop,
                    Flow::Normal => {}
                }
                if !self.step() {
                    return Flow::Stop;
                }
            },
            "whileLoop" => loop {
                if !self.boolean(b, 0) {
                    break;
                }
                match self.exec_stack(&b.sub1) {
                    Flow::Break => break,
                    Flow::Stop => return Flow::Stop,
                    Flow::Normal => {}
                }
                if !self.step() {
                    return Flow::Stop;
                }
            },
            "if" => {
                if self.boolean(b, 0) {
                    return self.exec_stack(&b.sub1);
                }
            }
            "ifElse" => {
                return if self.boolean(b, 0) { self.exec_stack(&b.sub1) } else { self.exec_stack(&b.sub2) };
            }
            "break" => return Flow::Break,
            "fileSetData" => {
                let f = Self::name(b, 0);
                let k = self.txt(b, 1);
                let v = self.txt(b, 2);
                self.prefs.entry(f).or_default().insert(k, v);
            }
            "fileRemoveData" => {
                let f = Self::name(b, 0);
                let k = self.txt(b, 1);
                self.prefs.entry(f).or_default().remove(&k);
            }
            "setEnable" => {
                let v = self.boolean(b, 1);
                self.with_w(b, |w| w.enabled = v);
            }
            "setVisible" => {
                let v = match Self::name(b, 1).as_str() {
                    "INVISIBLE" => 1,
                    "GONE" => 2,
                    _ => 0,
                };
                self.with_w(b, |w| w.visibility = v);
            }
            "setBgColor" => {
                let c = color_value(&Self::name(b, 1));
                self.with_w(b, |w| w.bg = c);
            }
            "setAlpha" => {
                let a = self.num(b, 1).clamp(0.0, 1.0) as f32;
                self.with_w(b, |w| w.alpha = a);
            }
            "setRadius" => {
                let r = self.num(b, 1).max(0.0) as f32;
                self.with_w(b, |w| w.radius = r);
            }
            "setElevation" => {
                let e = self.num(b, 1).max(0.0) as f32;
                self.with_w(b, |w| w.elevation = e);
            }
            "setBorder" => {
                let n = self.num(b, 1).max(0.0) as f32;
                let c = color_value(&Self::name(b, 2));
                self.with_w(b, |w| {
                    w.border = n;
                    w.border_color = c;
                });
            }
            "setText" => {
                let t = self.txt(b, 1);
                self.with_w(b, |w| w.text = t);
            }
            "setTextColor" => {
                let c = color_value(&Self::name(b, 1));
                self.with_w(b, |w| w.text_color = c);
            }
            "setTextSize" => {
                let s = self.num(b, 1) as f32;
                self.with_w(b, |w| w.text_size = s.clamp(4.0, 96.0));
            }
            "setHint" => {
                let t = self.txt(b, 1);
                self.with_w(b, |w| w.hint = t);
            }
            "setChecked" => {
                let v = self.boolean(b, 1);
                self.with_w(b, |w| w.checked = v);
            }
            "seekBarSetProgress" => {
                let v = self.num(b, 1) as i32;
                self.with_w(b, |w| w.progress = v.clamp(0, w.max.max(0)));
            }
            "seekBarSetMax" => {
                let v = self.num(b, 1) as i32;
                self.with_w(b, |w| {
                    w.max = v.max(0);
                    w.progress = w.progress.min(w.max);
                });
            }
            "listSetData" => {
                let l = Self::name(b, 1);
                let items: Vec<String> = self.lists.get(&l).map(|v| v.iter().map(|x| x.text()).collect()).unwrap_or_default();
                self.with_w(b, |w| {
                    w.items = items;
                    w.selected = 0;
                });
            }
            "webViewLoadUrl" => {
                let u = self.txt(b, 1);
                self.with_w(b, |w| w.url = u);
            }
            "setImage" => {
                let n = Self::name(b, 1);
                self.with_w(b, |w| w.image = n);
            }
            // a file path or web address: the phone view shows a picture file
            "setImageFrom" => {
                let n = self.txt(b, 1);
                self.with_w(b, |w| w.image = n);
            }
            "setVideoFrom" => {
                let n = self.txt(b, 1);
                self.with_w(b, |w| {
                    w.video = n;
                    w.playing = false;
                    w.video_pos = 0.0;
                });
            }
            "soundPlayFrom" => {
                let n = Self::name(b, 0);
                let src = self.txt(b, 1).trim().to_string();
                self.start_sound(n, src);
            }
            // the preview has no camera and no photos: it is always "cancelled"
            "cameraPhoto" | "cameraVideo" | "pickMedia" => {
                let what = match op {
                    "cameraPhoto" => "take a photo",
                    "cameraVideo" => "record a video",
                    _ => "pick a file from the phone",
                };
                self.toasts.push((format!("The preview can't {what}: try it on a phone"), self.now + 2.5));
                let mut locals = self.event_locals.clone();
                locals.insert("_filePath".into(), Val::Str(String::new()));
                let keep = std::mem::replace(&mut self.event_locals, locals);
                let f = self.exec_stack(&b.sub2);
                self.event_locals = keep;
                if matches!(f, Flow::Stop) {
                    return Flow::Stop;
                }
            }
            "setFont" => {
                let n = Self::name(b, 1);
                self.with_w(b, |w| w.font = n);
            }
            "setVideo" => {
                let n = Self::name(b, 1);
                self.with_w(b, |w| {
                    w.video = n;
                    w.playing = false;
                    w.video_pos = 0.0;
                });
            }
            "videoPlay" => self.with_w(b, |w| w.playing = !w.video.is_empty()),
            "videoPause" => self.with_w(b, |w| w.playing = false),
            "videoSeek" => {
                let ms = self.num(b, 1).max(0.0);
                let len = self.video_length(&Self::name(b, 0));
                self.with_w(b, |w| w.video_pos = ms.min(len));
            }
            "videoLoop" => {
                let on = self.boolean(b, 1);
                self.with_w(b, |w| w.looping = on);
            }
            // what to play; the preview's real player (player::Preview) plays it
            "soundPlay" => {
                let n = Self::name(b, 0);
                let f = Self::name(b, 1);
                let ok = self.project.file(&f).is_some_and(|x| x.kind == FileKind::Sound);
                self.start_sound(n, if ok { f } else { String::new() });
            }
            "soundResume" => {
                let n = Self::name(b, 0);
                if self.sounds.get(&n).is_some_and(|j| !j.source.is_empty()) {
                    self.playing.insert(n);
                }
            }
            "soundPause" => {
                let n = Self::name(b, 0);
                self.playing.remove(&n);
            }
            "soundStop" => {
                let n = Self::name(b, 0);
                self.start_sound(n, String::new());
            }
            "soundLoop" => {
                let on = self.boolean(b, 1);
                self.sounds.entry(Self::name(b, 0)).or_default().looping = on;
            }
            "soundVolume" => {
                let v = (self.num(b, 1) / 100.0).clamp(0.0, 1.0) as f32;
                self.sounds.entry(Self::name(b, 0)).or_default().volume = v;
            }
            "soundSeek" => {
                let ms = self.num(b, 1).max(0.0);
                let j = self.sounds.entry(Self::name(b, 0)).or_default();
                if !j.source.is_empty() {
                    j.seek = Some(ms);
                    j.pos = ms;
                }
            }
            "requestFocus" => {}
            "performClick" => {
                let id = Self::name(b, 0);
                let blocks = self.screen().events.iter().find(|e| e.target == id && e.name == "onClick").map(|e| e.blocks.clone());
                if let Some(bl) = blocks {
                    match self.exec_stack(&bl) {
                        Flow::Stop => return Flow::Stop,
                        _ => {}
                    }
                }
            }
            "doToast" => {
                let t = self.txt(b, 0);
                self.toasts.push((t, self.now + 2.0));
            }
            "setTitle" => {
                self.title = self.txt(b, 0);
            }
            "copyToClipboard" => {
                self.clipboard = Some(self.txt(b, 0));
            }
            "finishActivity" => {
                if self.switch_to.is_some() && self.back_stack.last() == Some(&self.screen_idx) {
                    // "open screen" just ran: as on a phone, the new screen
                    // stays and this one leaves the back stack
                    self.back_stack.pop();
                } else {
                    match self.back_stack.pop() {
                        Some(prev) => self.switch_to = Some(prev),
                        None => self.finished = true,
                    }
                }
                return Flow::Stop;
            }
            "intentSetScreen" => {
                let i = Self::name(b, 0);
                let s = Self::name(b, 1);
                self.intent_target.insert(i, s);
            }
            "intentPutExtra" => {
                let i = Self::name(b, 0);
                let k = self.txt(b, 1);
                let v = self.txt(b, 2);
                self.pending_extras.entry(i).or_default().insert(k, v);
            }
            "openScreen" => {
                let target = Self::name(b, 0);
                match self.project.screens.iter().position(|s| s.activity_name() == target) {
                    Some(idx) => {
                        self.extras = Default::default();
                        self.back_stack.push(self.screen_idx);
                        self.switch_to = Some(idx);
                    }
                    None => self.crash("android.content.ActivityNotFoundException", "No Activity found to handle Intent { }".into(), "open screen: no screen chosen".into()),
                }
            }
            "startActivity" => {
                let i = Self::name(b, 0);
                let target = self.intent_target.get(&i).cloned().unwrap_or_default();
                match self.project.screens.iter().position(|s| s.activity_name() == target) {
                    Some(idx) => {
                        self.extras = self.pending_extras.get(&i).cloned().unwrap_or_default();
                        self.back_stack.push(self.screen_idx);
                        self.switch_to = Some(idx);
                    }
                    None => self.crash("android.content.ActivityNotFoundException", "No Activity found to handle Intent { }".into(), format!("open screen with {i}: no screen chosen (use \"go to\" first)")),
                }
            }
            "timerAfter" | "timerEvery" => {
                let name = Self::name(b, 0);
                let delay = self.num(b, 1) / 1000.0;
                let every = if op == "timerEvery" { Some(self.num(b, 2) / 1000.0) } else { None };
                self.timers.retain(|t| t.name != name);
                self.timers.push(TimerJob { name, due: self.now + delay.max(0.0), every, blocks: b.sub1.clone() });
            }
            "timerCancel" => {
                let name = Self::name(b, 0);
                self.timers.retain(|t| t.name != name);
            }
            "dialogSetTitle" | "dialogSetMessage" => {
                let n = Self::name(b, 0);
                let t = self.txt(b, 1);
                let d = self.building_dialogs.entry(n).or_insert(Dialog { title: String::new(), message: String::new(), ok: None, cancel: None });
                if op == "dialogSetTitle" {
                    d.title = t;
                } else {
                    d.message = t;
                }
            }
            "dialogOkButton" | "dialogCancelButton" => {
                let n = Self::name(b, 0);
                let label = self.txt(b, 1);
                let d = self.building_dialogs.entry(n).or_insert(Dialog { title: String::new(), message: String::new(), ok: None, cancel: None });
                if op == "dialogOkButton" {
                    d.ok = Some((label, b.sub1.clone()));
                } else {
                    d.cancel = Some((label, b.sub1.clone()));
                }
            }
            "dialogShow" => {
                let n = Self::name(b, 0);
                self.dialog = Some(self.building_dialogs.get(&n).cloned().unwrap_or(Dialog { title: String::new(), message: String::new(), ok: None, cancel: None }));
            }
            "vibratorAction" => {
                let ms = self.num(b, 1);
                self.vibrating_until = self.now + ms / 1000.0;
            }
            "calendarGetNow" => {}
            "moreBlock" => {
                let n = Self::name(b, 0);
                let def = self.screen().events.iter().find(|e| e.target == "moreBlock" && e.name == n).map(|e| (e.blocks.clone(), e.params.clone()));
                if let Some((bl, params)) = def {
                    // the inputs are all the block sees of its caller, as in
                    // the phone's code where it is a function of its own
                    let mut locals = HashMap::new();
                    for (i, p) in params.iter().enumerate() {
                        let v = match p.ty {
                            VarType::Num => Val::Num(self.num(b, i + 1)),
                            VarType::Bool => Val::Bool(self.boolean(b, i + 1)),
                            _ => Val::Str(self.txt(b, i + 1)),
                        };
                        locals.insert(format!("_p_{}", p.name), v);
                    }
                    let saved = std::mem::replace(&mut self.event_locals, locals);
                    let flow = self.exec_stack(&bl);
                    self.event_locals = saved;
                    if let Flow::Stop = flow {
                        return Flow::Stop;
                    }
                }
            }
            _ => {
                // reporters used as statements do nothing
            }
        }
        Flow::Normal
    }

    fn with_w(&mut self, b: &Block, f: impl FnOnce(&mut WState)) {
        let id = Self::name(b, 0);
        match self.widgets.get_mut(&id) {
            Some(w) => f(w),
            None => self.err(format!("{}: no widget '{}'", b.op, id)),
        }
    }

    pub fn eval(&mut self, b: &Block) -> Val {
        self.steps += 1;
        let op = b.op.as_str();
        match op {
            "getVar" => {
                let n = Self::name(b, 0);
                self.vars.get(&n).cloned().unwrap_or(Val::Str(String::new()))
            }
            "paramStr" => self.event_locals.get(&format!("_p_{}", Self::name(b, 0))).cloned().unwrap_or(Val::Str(String::new())),
            "paramNum" => self.event_locals.get(&format!("_p_{}", Self::name(b, 0))).cloned().unwrap_or(Val::Num(0.0)),
            "paramBool" => self.event_locals.get(&format!("_p_{}", Self::name(b, 0))).cloned().unwrap_or(Val::Bool(false)),
            "eventPosition" => self.event_locals.get("_position").cloned().unwrap_or(Val::Num(0.0)),
            "eventChecked" => self.event_locals.get("_isChecked").cloned().unwrap_or(Val::Bool(false)),
            "eventText" => self.event_locals.get("_charSeq").cloned().unwrap_or(Val::Str(String::new())),
            "eventProgress" => self.event_locals.get("_progressValue").cloned().unwrap_or(Val::Num(0.0)),
            "webResponse" => self.event_locals.get("_response").cloned().unwrap_or(Val::Str(String::new())),
            "mediaPath" => self.event_locals.get("_filePath").cloned().unwrap_or(Val::Str(String::new())),
            "supaError" => Val::Str(self.event_locals.get("_response").map(|v| supa_error(&v.text())).unwrap_or_default()),
            "supaSignedIn" => Val::Bool(self.supa.get("access").is_some_and(|a| !a.is_empty())),
            "supaUserEmail" => Val::Str(self.supa.get("email").cloned().unwrap_or_default()),
            "supaUserId" => Val::Str(self.supa.get("id").cloned().unwrap_or_default()),
            "webStatus" => self.event_locals.get("_code").cloned().unwrap_or(Val::Num(0.0)),
            "mapToJson" => {
                let m = self.maps.get(&Self::name(b, 0)).cloned().unwrap_or_default();
                Val::Str(map_json(&m).to_string())
            }
            "listMapToJson" => {
                let l = self.list_maps.get(&Self::name(b, 0)).cloned().unwrap_or_default();
                Val::Str(serde_json::Value::Array(l.iter().map(map_json).collect()).to_string())
            }
            "mapGet" => {
                let n = Self::name(b, 0);
                let k = self.txt(b, 1);
                // a missing key reads as empty text, on both phones too
                Val::Str(self.maps.get(&n).and_then(|m| m.get(&k).cloned()).unwrap_or_default())
            }
            "mapContainKey" => {
                let n = Self::name(b, 0);
                let k = self.txt(b, 1);
                Val::Bool(self.maps.get(&n).map_or(false, |m| m.contains_key(&k)))
            }
            "mapSize" => Val::Num(self.maps.get(&Self::name(b, 0)).map_or(0, |m| m.len()) as f64),
            "mapIsEmpty" => Val::Bool(self.maps.get(&Self::name(b, 0)).map_or(true, |m| m.is_empty())),
            "getAtListInt" | "getAtListStr" => {
                let at = self.num(b, 0) as i64;
                let n = Self::name(b, 1);
                match self.lists.get(&n).and_then(|l| if at >= 0 { l.get(at as usize).cloned() } else { None }) {
                    Some(v) => {
                        if op == "getAtListInt" {
                            Val::Num(v.num())
                        } else {
                            Val::Str(v.text())
                        }
                    }
                    None => {
                        let len = self.lists.get(&n).map(|l| l.len()).unwrap_or(0);
                        self.crash("java.lang.IndexOutOfBoundsException", out_of_bounds(at, len), format!("get at {at}: outside list {n}"));
                        if op == "getAtListInt" {
                            Val::Num(0.0)
                        } else {
                            Val::Str(String::new())
                        }
                    }
                }
            }
            "indexListInt" => {
                let v = self.num(b, 0);
                let n = Self::name(b, 1);
                Val::Num(self.lists.get(&n).and_then(|l| l.iter().position(|x| x.num() == v)).map_or(-1.0, |p| p as f64))
            }
            "indexListStr" => {
                let v = self.txt(b, 0);
                let n = Self::name(b, 1);
                Val::Num(self.lists.get(&n).and_then(|l| l.iter().position(|x| x.text() == v)).map_or(-1.0, |p| p as f64))
            }
            "containListInt" => {
                let n = Self::name(b, 0);
                let v = self.num(b, 1);
                Val::Bool(self.lists.get(&n).map_or(false, |l| l.iter().any(|x| x.num() == v)))
            }
            "containListStr" => {
                let n = Self::name(b, 0);
                let v = self.txt(b, 1);
                Val::Bool(self.lists.get(&n).map_or(false, |l| l.iter().any(|x| x.text() == v)))
            }
            "getAtListMap" => {
                let at = self.num(b, 0) as i64;
                let k = self.txt(b, 1);
                let n = Self::name(b, 2);
                let len = self.list_maps.get(&n).map(|l| l.len()).unwrap_or(0);
                match self.list_maps.get(&n).and_then(|l| if at >= 0 { l.get(at as usize) } else { None }).map(|m| m.get(&k).cloned()) {
                    Some(Some(v)) => Val::Str(v),
                    // a missing key reads as empty text
                    Some(None) => Val::Str(String::new()),
                    None => {
                        self.crash("java.lang.IndexOutOfBoundsException", out_of_bounds(at, len), format!("get at {at}: outside list {n}"));
                        Val::Str(String::new())
                    }
                }
            }
            "lengthList" => {
                let n = Self::name(b, 0);
                Val::Num(self.lists.get(&n).map(|l| l.len()).or_else(|| self.list_maps.get(&n).map(|l| l.len())).unwrap_or(0) as f64)
            }
            "true" => Val::Bool(true),
            "false" => Val::Bool(false),
            "<" => Val::Bool(self.num(b, 0) < self.num(b, 1)),
            "=" => Val::Bool(self.num(b, 0) == self.num(b, 1)),
            ">" => Val::Bool(self.num(b, 0) > self.num(b, 1)),
            "&&" => Val::Bool(self.boolean(b, 0) && self.boolean(b, 1)),
            "||" => Val::Bool(self.boolean(b, 0) || self.boolean(b, 1)),
            "not" => Val::Bool(!self.boolean(b, 0)),
            "+" => Val::Num(self.num(b, 0) + self.num(b, 1)),
            "-" => Val::Num(self.num(b, 0) - self.num(b, 1)),
            "*" => Val::Num(self.num(b, 0) * self.num(b, 1)),
            "/" => Val::Num(self.num(b, 0) / self.num(b, 1)),
            "%" => Val::Num(self.num(b, 0) % self.num(b, 1)),
            "random" => {
                let lo = self.num(b, 0) as i64;
                let hi = self.num(b, 1) as i64;
                let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
                Val::Num((lo + (self.next_rand() % ((hi - lo + 1) as u64)) as i64) as f64)
            }
            "stringLength" => Val::Num(utf16_len(&self.txt(b, 0)) as f64),
            "stringJoin" => Val::Str(self.txt(b, 0) + &self.txt(b, 1)),
            "stringIndex" => {
                let needle = self.txt(b, 0);
                let hay = self.txt(b, 1);
                Val::Num(hay.find(&needle).map_or(-1.0, |p| utf16_len(&hay[..p]) as f64))
            }
            "stringLastIndex" => {
                let needle = self.txt(b, 0);
                let hay = self.txt(b, 1);
                Val::Num(hay.rfind(&needle).map_or(-1.0, |p| utf16_len(&hay[..p]) as f64))
            }
            "stringSub" => {
                // String.substring((int)a, (int)b) throws outside 0 <= a <= b <= length
                let s: Vec<u16> = self.txt(b, 0).encode_utf16().collect();
                let (a, e) = (self.num(b, 1) as i64, self.num(b, 2) as i64);
                if a < 0 || e > s.len() as i64 || a > e {
                    self.crash("java.lang.StringIndexOutOfBoundsException", format!("begin {a}, end {e}, length {}", s.len()), format!("substring {a} to {e}: outside the text"));
                    Val::Str(String::new())
                } else {
                    Val::Str(String::from_utf16_lossy(&s[a as usize..e as usize]))
                }
            }
            "stringEquals" => Val::Bool(self.txt(b, 0) == self.txt(b, 1)),
            "stringContains" => Val::Bool(self.txt(b, 0).contains(&self.txt(b, 1))),
            "stringStartsWith" => Val::Bool(self.txt(b, 0).starts_with(&self.txt(b, 1))),
            "stringEndsWith" => Val::Bool(self.txt(b, 0).ends_with(&self.txt(b, 1))),
            // a data file's text, as both phones read it ("" if it isn't there)
            "assetText" => {
                let n = Self::name(b, 0);
                let text = self.project.file(&n).filter(|f| f.kind == FileKind::Data).and_then(|f| std::fs::read(self.project.file_path(f)).ok());
                Val::Str(text.map(|t| String::from_utf8_lossy(t.strip_prefix(b"\xEF\xBB\xBF".as_slice()).unwrap_or(&t)).into_owned()).unwrap_or_default())
            }
            "isNumber" => Val::Bool(self.txt(b, 0).trim().parse::<f64>().is_ok()),
            "toStringFixed" => {
                let x = self.num(b, 0);
                let d = self.num(b, 1);
                Val::Str(fixed_text(x, d))
            }
            "sumList" => {
                let n = Self::name(b, 0);
                Val::Num(self.lists.get(&n).map(|l| l.iter().map(|v| v.num()).sum()).unwrap_or(0.0))
            }
            "joinList" => {
                let n = Self::name(b, 0);
                let sep = self.txt(b, 1);
                Val::Str(self.lists.get(&n).map(|l| l.iter().map(|v| v.text()).collect::<Vec<_>>().join(&sep)).unwrap_or_default())
            }
            "stringReplace" => {
                let s = self.txt(b, 0);
                let from = self.txt(b, 1);
                let to = self.txt(b, 2);
                Val::Str(if from.is_empty() { s } else { s.replace(&from, &to) })
            }
            "trim" => Val::Str(self.txt(b, 0).trim().to_string()),
            "toUpperCase" => Val::Str(self.txt(b, 0).to_uppercase()),
            "toLowerCase" => Val::Str(self.txt(b, 0).to_lowercase()),
            "toNumber" => {
                let t = self.txt(b, 0);
                match t.trim().parse::<f64>() {
                    Ok(n) => Val::Num(n),
                    Err(_) => {
                        // Double.parseDouble's messages
                        let detail = if t.trim().is_empty() { "empty String".to_string() } else { format!("For input string: \"{t}\"") };
                        self.crash("java.lang.NumberFormatException", detail, format!("toNumber: '{t}' is not a number"));
                        Val::Num(0.0)
                    }
                }
            }
            "toString" => Val::Str(format!("{}", self.num(b, 0) as i64)),
            "toStringWithDecimal" => Val::Str(fmt_num(self.num(b, 0))),
            // The pretend phone is 360 x 640 dp at density 3, so n dp is
            // n * 3 pixels and the screen is 1080 x 1920 pixels.
            "mathGetDip" => Val::Num((self.num(b, 0) as i64) as f64 * PHONE_DENSITY),
            "mathGetDisplayWidth" => Val::Num(360.0 * PHONE_DENSITY),
            "mathGetDisplayHeight" => Val::Num(640.0 * PHONE_DENSITY),
            "mathPi" => Val::Num(std::f64::consts::PI),
            "mathE" => Val::Num(std::f64::consts::E),
            "mathPow" => Val::Num(self.num(b, 0).powf(self.num(b, 1))),
            "mathMin" => Val::Num(self.num(b, 0).min(self.num(b, 1))),
            "mathMax" => Val::Num(self.num(b, 0).max(self.num(b, 1))),
            "mathSqrt" => Val::Num(self.num(b, 0).sqrt()),
            "mathAbs" => Val::Num(self.num(b, 0).abs()),
            // Java's Math.round: halves go up (-2.5 → -2)
            "mathRound" => Val::Num((self.num(b, 0) + 0.5).floor()),
            "mathCeil" => Val::Num(self.num(b, 0).ceil()),
            "mathFloor" => Val::Num(self.num(b, 0).floor()),
            "mathSin" => Val::Num(self.num(b, 0).sin()),
            "mathCos" => Val::Num(self.num(b, 0).cos()),
            "mathTan" => Val::Num(self.num(b, 0).tan()),
            "mathLog" => Val::Num(self.num(b, 0).ln()),
            "mathLog10" => Val::Num(self.num(b, 0).log10()),
            "mathToRadian" => Val::Num(self.num(b, 0).to_radians()),
            "mathToDegree" => Val::Num(self.num(b, 0).to_degrees()),
            "fileGetData" => {
                let f = Self::name(b, 0);
                let k = self.txt(b, 1);
                Val::Str(self.prefs.get(&f).and_then(|m| m.get(&k).cloned()).unwrap_or_default())
            }
            "fileContainsData" => {
                let f = Self::name(b, 0);
                let k = self.txt(b, 1);
                Val::Bool(self.prefs.get(&f).map_or(false, |m| m.contains_key(&k)))
            }
            "getEnable" => Val::Bool(self.widgets.get(&Self::name(b, 0)).map_or(false, |w| w.enabled)),
            "getText" => Val::Str(self.widgets.get(&Self::name(b, 0)).map(|w| w.text.clone()).unwrap_or_default()),
            "getChecked" => Val::Bool(self.widgets.get(&Self::name(b, 0)).map_or(false, |w| w.checked)),
            "seekBarGetProgress" => Val::Num(self.widgets.get(&Self::name(b, 0)).map_or(0, |w| w.progress) as f64),
            "seekBarGetMax" => Val::Num(self.widgets.get(&Self::name(b, 0)).map_or(0, |w| w.max) as f64),
            "intentGetString" => {
                let k = self.txt(b, 0);
                Val::Str(self.extras.get(&k).cloned().unwrap_or_default())
            }
            "calendarFormat" => Val::Str(format_now(&self.txt(b, 1))),
            "calendarGetTime" => Val::Num((now_secs() * 1000) as f64),
            "soundIsPlaying" => Val::Bool(self.playing.contains(&Self::name(b, 0))),
            "videoIsPlaying" => Val::Bool(self.widgets.get(&Self::name(b, 0)).is_some_and(|w| w.playing)),
            "videoPosition" => Val::Num(self.widgets.get(&Self::name(b, 0)).map_or(0.0, |w| w.video_pos)),
            "videoDuration" => Val::Num(self.video_length(&Self::name(b, 0))),
            "soundPosition" => Val::Num(self.sounds.get(&Self::name(b, 0)).filter(|j| !j.source.is_empty()).map_or(0.0, |j| j.pos)),
            "soundDuration" => Val::Num(self.sounds.get(&Self::name(b, 0)).filter(|j| !j.source.is_empty()).map_or(0.0, |j| j.len)),
            _ => Val::Str(String::new()),
        }
    }
}

/// A widget's state when its screen opens.
pub fn initial_state(w: &Widget) -> WState {
    WState {
        text: w.text.clone(),
        hint: w.hint.clone(),
        checked: w.checked,
        progress: w.progress,
        max: w.max,
        enabled: w.enabled,
        visibility: if w.visible { 0 } else { 2 },
        bg: w.bg,
        text_color: w.text_color,
        text_size: w.text_size,
        alpha: 1.0,
        items: vec![],
        selected: 0,
        url: String::new(),
        image: w.image.clone(),
        radius: w.radius as f32,
        elevation: w.elevation as f32,
        border: w.border as f32,
        border_color: w.border_color,
        font: w.font.clone(),
        video: w.video.clone(),
        playing: w.autoplay && !w.video.is_empty(),
        looping: w.looping,
        video_pos: 0.0,
        reveal: false,
    }
}

/// Every statement of a stack with its depth, id and readable text.
/// ArrayList's message for a bad index.
fn out_of_bounds(at: i64, len: usize) -> String {
    format!("Index {at} out of bounds for length {len}")
}


fn flatten(blocks: &[Block], depth: usize, out: &mut Vec<(usize, u64, String)>) {
    for b in blocks {
        out.push((depth, b.id, describe(b)));
        flatten(&b.sub1, depth + 1, out);
        if !b.sub2.is_empty() {
            out.push((depth, 0, "else".into()));
            flatten(&b.sub2, depth + 1, out);
        }
    }
}

fn find_text(blocks: &[Block], id: u64) -> Option<String> {
    for b in blocks {
        if b.id == id {
            return Some(describe(b));
        }
        if let Some(t) = find_text(&b.sub1, id).or_else(|| find_text(&b.sub2, id)) {
            return Some(t);
        }
    }
    None
}

/// Tiny subset of SimpleDateFormat (yyyy MM dd HH mm ss) in UTC.
fn format_now(pat: &str) -> String {
    let t = now_secs() as i64;
    let days = t.div_euclid(86400);
    let secs = t.rem_euclid(86400);
    let (y, m, d) = civil(days);
    let p = if pat.is_empty() { "yyyy-MM-dd HH:mm:ss" } else { pat };
    p.replace("yyyy", &format!("{y:04}"))
        .replace("MM", &format!("{m:02}"))
        .replace("dd", &format!("{d:02}"))
        .replace("HH", &format!("{:02}", secs / 3600))
        .replace("mm", &format!("{:02}", (secs / 60) % 60))
        .replace("ss", &format!("{:02}", secs % 60))
}

pub(crate) fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn sample_counter_runs() {
        let p = crate::app::sample_project(601);
        let mut rt = Runtime::new(p.clone(), 0.0);
        assert_eq!(rt.widgets["textview2"].text, "Tap the button to count");
        rt.fire("button1", "onClick", HashMap::new());
        rt.fire("button1", "onClick", HashMap::new());
        assert_eq!(rt.widgets["textview1"].text, "2");
        rt.widgets.get_mut("checkbox1").unwrap().checked = true;
        rt.fire("button1", "onClick", HashMap::new());
        assert_eq!(rt.widgets["textview1"].text, "4");
        rt.fire("button2", "onClick", HashMap::new());
        let d = rt.dialog.clone().expect("dialog shown");
        assert_eq!(d.title, "Start again?");
        rt.dialog_button(true);
        assert_eq!(rt.widgets["textview1"].text, "0");
        assert_eq!(rt.toasts.len(), 1);
        assert!(rt.log.is_empty(), "{:?}", rt.log);
        // code generation produces the listeners
        let kt = crate::native::kotlin(&p, &p.screens[0]);
        assert!(kt.contains("button1.setOnClickListener"), "{kt}");
        assert!(kt.contains("if (checkbox1.isChecked)"), "{kt}");
        assert!(crate::codegen::xml(&p.screens[0]).contains("android:id=\"@+id/checkbox1\""));
    }

    /// "open screen" opens the picked screen in one block, and is coded
    /// for both phones.
    #[test]
    fn open_screen_in_one_block() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let mut second = Screen::new("second");
        second.events[0].blocks = vec![Block { id: new_id(), op: "doToast".into(), args: vec![Arg::Text("on two".into())], sub1: vec![], sub2: vec![] }];
        p.screens.push(second);
        let screens: Vec<String> = p.screens.iter().map(|s| s.activity_name()).collect();
        let b = crate::blocks::make_block("openScreen", &p.screens[0], &crate::blocks::Choices::screens(screens));
        assert_eq!(b.args, vec![Arg::Text("SecondActivity".into())], "starts on the other screen");
        p.screens[0].events[0].blocks = vec![b];
        let rt = Runtime::new(p.clone(), 0.0);
        assert_eq!(rt.screen().name, "second");
        assert!(rt.toasts.iter().any(|t| t.0 == "on two"), "the second screen opened");
        let kt = crate::native::kotlin(&p, &p.screens[0]);
        assert!(kt.contains("startActivity(android.content.Intent(applicationContext, SecondActivity::class.java))"), "{kt}");
        let sw = crate::native::swift_screen(&p, &p.screens[0]);
        assert!(sw.contains("startActivity(Intent(screen: \"SecondActivity\"))"), "{sw}");
    }

    /// "open screen" then "close this screen" swaps screens, as on a phone:
    /// the new one shows and Back skips the closed one.
    #[test]
    fn app_bar_code() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens.push(Screen::new("second"));
        // a project saved before app bar settings existed
        let old: Screen = serde_json::from_str(r#"{"name":"main","widgets":[],"vars":[],"lists":[],"components":[],"events":[]}"#).unwrap();
        assert_eq!(old.app_bar, AppBar::default());
        // by default: the themed bar, its up arrow on screens opened from another
        let kt = crate::native::kotlin(&p, &p.screens[1]);
        assert!(kt.contains("if (!isTaskRoot) actionBar?.setDisplayHomeAsUpEnabled(true)") && kt.contains("android.R.id.home"));
        assert!(!kt.contains("statusBarColor") && !kt.contains("onTitleChanged"));
        let km = crate::native::kotlin_manifest(&p);
        assert!(km.contains("android:theme=\"@style/Blockwise\">") && km.contains("<activity android:name=\".SecondActivity\"/>"));
        let sw = crate::native::swift_screen(&p, &p.screens[1]);
        assert!(!sw.contains("toolbarBackground") && sw.contains("StatusBarLook.set(light: nil)"));
        // a white bar with a red title and no back arrow
        p.screens[1].app_bar = AppBar { color: 0xFFFF_FFFF, title_color: 0xFFE5_3935, no_back: true, ..Default::default() };
        let kt = crate::native::kotlin(&p, &p.screens[1]);
        assert!(kt.contains("ColorDrawable(0xFFFFFFFF.toInt())") && kt.contains("window.statusBarColor = 0xFFD8D8D8.toInt()") && kt.contains("SYSTEM_UI_FLAG_LIGHT_STATUS_BAR"));
        assert!(kt.contains("ForegroundColorSpan(0xFFE53935.toInt())") && !kt.contains("setDisplayHomeAsUpEnabled"));
        p.screens[1].app_bar.no_back = false;
        assert!(crate::native::kotlin(&p, &p.screens[1]).contains("it.setTint(0xFFE53935.toInt()); actionBar?.setHomeAsUpIndicator(it)"));
        p.screens[1].app_bar.no_back = true;
        let sw = crate::native::swift_screen(&p, &p.screens[1]);
        assert!(sw.contains(".toolbarBackground(Color(argb: 0xFFFFFFFF), for: .navigationBar)") && sw.contains(".toolbarColorScheme(.light, for: .navigationBar)") && sw.contains("StatusBarLook.set(light: false)"));
        assert!(sw.contains("foregroundColor(Color(argb: 0xFFE53935))") && sw.contains(".navigationBarBackButtonHidden(true)"));
        // a red title reads as dark, so the bar's arrow and menu are dark too
        assert!(crate::native::kotlin_manifest(&p).contains("SecondActivity\" android:theme=\"@style/Blockwise.LightBar\""));
        // no bar at all
        p.screens[1].app_bar = AppBar { hidden: true, ..Default::default() };
        assert!(crate::native::kotlin_manifest(&p).contains("android:theme=\"@style/Blockwise.NoBar\""));
        assert!(crate::native::swift_screen(&p, &p.screens[1]).contains(".toolbar(.hidden, for: .navigationBar)"));
        assert!(!crate::native::kotlin(&p, &p.screens[1]).contains("actionBar"));
    }

    #[test]
    fn app_bar_buttons() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens[0].app_bar.actions = vec![
            BarAction { id: "searchButton".into(), icon: "search".into(), label: "Search".into() },
            BarAction { id: "shareButton".into(), icon: "share".into(), label: String::new() },
        ];
        let toast = Block { id: new_id(), op: "doToast".into(), args: vec![Arg::Text("Searching".into())], sub1: vec![], sub2: vec![] };
        p.screens[0].events.push(Event { target: "searchButton".into(), name: "onClick".into(), blocks: vec![toast], loose: vec![], params: vec![] });
        assert!(p.screens[0].name_taken("searchButton") && p.screens[0].next_action_id("search") == "searchButton2");
        // the preview runs its event
        let mut rt = Runtime::new(p.clone(), 0.0);
        rt.fire("searchButton", "onClick", HashMap::new());
        assert_eq!(rt.toasts.last().map(|t| t.0.as_str()), Some("Searching"));
        // Android: a menu with tinted icons, the event in onOptionsItemSelected
        let kt = crate::native::kotlin(&p, &p.screens[0]);
        assert!(kt.contains("menu.add(0, 1, 0, \"Search\")") && kt.contains("setIcon(R.drawable.bw_icon_search)") && kt.contains("menu.add(0, 2, 1, \"share\")"));
        assert!(kt.contains("icon?.mutate()?.setTint(0xFFFFFFFF.toInt())"));
        let when = kt.split("override fun onOptionsItemSelected").nth(1).unwrap();
        assert!(when.contains("1 -> {") && when.contains("Searching") && when.contains("android.R.id.home -> onBackPressed()"));
        // iPhone: SF Symbols in the trailing toolbar
        let sw = crate::native::swift_screen(&p, &p.screens[0]);
        assert!(sw.contains("Button { m.searchButton_onClick() } label: { Image(systemName: \"magnifyingglass\") }") && sw.contains("func searchButton_onClick()"));
        assert!(sw.contains("Button { } label: { Image(systemName: \"square.and.arrow.up\") }"));
        // the icon drawn from the Lucide outline
        let v = crate::icons::android_vector(bar_icon("search").0);
        assert!(v.contains("android:viewportWidth=\"24\"") && v.contains("pathData=\"M") && v.matches('Z').count() >= 2);
        // a hidden bar has no buttons
        p.screens[0].app_bar.hidden = true;
        assert!(!crate::native::kotlin(&p, &p.screens[0]).contains("onCreateOptionsMenu"));
        assert!(!crate::native::swift_screen(&p, &p.screens[0]).contains("magnifyingglass"));
    }

    #[test]
    fn bottom_sheets() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens.push(Screen::new("picker"));
        p.screens[1].sheet = Sheet::Half;
        p.screens[1].app_bar.actions.push(BarAction { id: "doneButton".into(), icon: "done".into(), label: String::new() });
        // the first screen is never a sheet
        p.screens[0].sheet = Sheet::Full;
        assert_eq!(p.sheet_of(&p.screens[0]), Sheet::No);
        let km = crate::native::kotlin_manifest(&p);
        assert!(km.contains("<activity android:name=\".PickerActivity\" android:theme=\"@style/Blockwise.Sheet\"/>") && !km.contains("MainActivity\" android:theme=\"@style/Blockwise.Sheet"));
        let kt = crate::native::kotlin(&p, &p.screens[1]);
        assert!(kt.contains("window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, (resources.displayMetrics.heightPixels * 0.5).toInt())") && kt.contains("setFinishOnTouchOutside(true)"));
        // no app bar on a sheet: no up arrow, no buttons
        assert!(!kt.contains("actionBar") && !kt.contains("onCreateOptionsMenu"));
        let sw = crate::native::swift_screen(&p, &p.screens[1]);
        assert!(sw.contains(".presentationDetents([.medium])") && sw.contains("router.sheet = nil"));
        assert!(!sw.contains("doneButton") && !sw.contains("StatusBarLook.set"));
        let main = crate::native::swift_screen(&p, &p.screens[0]);
        assert!(main.contains("if Router.sheets.contains(intent.screen)") && main.contains("if openedSheet"));
        let app = crate::native::swift_app(&p);
        assert!(app.contains(".sheet(item: $router.sheet)") && app.contains("static let sheets: Set<String> = [\"PickerActivity\"]"));
        // the preview: the sheet opens over main, back closes it
        let mut rt = Runtime::new(p.clone(), 0.0);
        rt.go_to_screen(1);
        assert_eq!((rt.screen().name.as_str(), rt.back_stack.clone()), ("picker", vec![0]));
        rt.press_back();
        assert_eq!(rt.screen().name, "main");
        // saved projects without the setting open as pages
        let old: Screen = serde_json::from_str(r#"{"name":"x","widgets":[],"vars":[],"lists":[],"components":[],"events":[]}"#).unwrap();
        assert_eq!(old.sheet, Sheet::No);
        assert!(serde_json::to_string(&p.screens[1]).unwrap().contains("\"sheet\":\"half\""));
    }

    #[test]
    fn open_then_close_swaps_screens() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens.push(Screen::new("quiz"));
        p.screens.push(Screen::new("result"));
        let open = blk("openScreen", vec![Arg::Text("ResultActivity".into())], vec![], vec![]);
        p.screens[1].events[0].blocks = vec![open, blk("finishActivity", vec![], vec![], vec![])];
        let mut rt = Runtime::new(p.clone(), 0.0);
        rt.go_to_screen(1);
        assert_eq!(rt.screen().name, "result", "{:?}", rt.log);
        assert_eq!(rt.back_stack, vec![0], "the quiz left the back stack");
        rt.press_back();
        assert_eq!(rt.screen().name, "main", "Back goes past the closed quiz");
        // iOS does the same: the screen below the new one leaves the path
        let sw = crate::native::swift_screen(&p, &p.screens[1]);
        assert!(sw.contains("router.path.remove(at: router.path.count - 2)"), "{sw}");
    }

    #[test]
    fn endless_loop_is_stopped() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens[0].events[0].blocks = vec![Block { id: 1, op: "forever".into(), args: vec![], sub1: vec![], sub2: vec![] }];
        let rt = Runtime::new(p, 0.0);
        assert!(rt.log.iter().any(|l| l.contains("seconds")), "{:?}", rt.log);
    }

    #[test]
    fn timers_and_lists() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let s = &mut p.screens[0];
        s.lists.push(ListVar { name: "names".into(), ty: ListType::Str });
        s.components.push(Component { name: "t".into(), kind: CompKind::Timer });
        let add = |v: &str| Block { id: new_id(), op: "addListStr".into(), args: vec![Arg::Text(v.into()), Arg::Text("names".into())], sub1: vec![], sub2: vec![] };
        let timer = Block { id: new_id(), op: "timerAfter".into(), args: vec![Arg::Text("t".into()), Arg::Text("500".into())], sub1: vec![add("late")], sub2: vec![] };
        s.events[0].blocks = vec![add("a"), add("b"), timer];
        let mut rt = Runtime::new(p, 0.0);
        assert_eq!(rt.lists["names"].len(), 2);
        rt.tick(0.4);
        assert_eq!(rt.lists["names"].len(), 2);
        rt.tick(0.6);
        assert_eq!(rt.lists["names"].len(), 3);
        assert!(!rt.has_timers());
    }

    fn blk(op: &str, args: Vec<Arg>, sub1: Vec<Block>, sub2: Vec<Block>) -> Block {
        Block { id: new_id(), op: op.into(), args, sub1, sub2 }
    }

    fn t(s: &str) -> Arg {
        Arg::Text(s.into())
    }

    #[test]
    fn numbers_read_like_java() {
        for (n, t) in [(7.0, "7.0"), (0.1 + 0.2, "0.30000000000000004"), (0.001, "0.001"), (1234567.5, "1234567.5"), (1e7, "1.0E7"), (12345678.0, "1.2345678E7"), (1e20, "1.0E20"), (1.5e-6, "1.5E-6"), (-0.0005, "-5.0E-4"), (-3.25, "-3.25"), (100.0, "100.0"), (f64::NAN, "NaN"), (f64::NEG_INFINITY, "-Infinity"), (-0.0, "-0.0")] {
            assert_eq!(fmt_num(n), t, "{n}");
        }
    }

    #[test]
    fn json_reads_and_writes() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let s = &mut p.screens[0];
        let rows = blk("listMapFromJson", vec![t("rows"), t(r#"[{"name":"Ada","age":36,"ok":true,"x":null,"tags":["a"]},"loose"]"#)], vec![], vec![]);
        let one = blk("mapFromJson", vec![t("m"), t(r#"{"id":7,"price":2.5}"#)], vec![], vec![]);
        let back = blk("mapToJson", vec![t("m")], vec![], vec![]);
        let save = blk("setVarString", vec![t("out"), Arg::Block(Box::new(back))], vec![], vec![]);
        s.events[0].blocks = vec![rows, one, save];
        let rt = Runtime::new(p, 0.0);
        let r = &rt.list_maps["rows"];
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["name"], "Ada");
        assert_eq!(r[0]["age"], "36");
        assert_eq!(r[0]["ok"], "true");
        assert_eq!(r[0]["x"], "");
        assert_eq!(r[0]["tags"], r#"["a"]"#);
        assert_eq!(r[1]["value"], "loose");
        assert_eq!(rt.maps["m"]["price"], "2.5");
        assert_eq!(rt.vars.get("out").map(|v| v.text()), Some(r#"{"id":"7","price":"2.5"}"#.to_string()));
    }

    /// A one-request web server on this machine: answers with `code` and
    /// echoes the method, a header and the body back as JSON.
    fn serve_once(code: u16) -> String {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            let mut buf = vec![];
            let mut chunk = [0u8; 4096];
            loop {
                let n = c.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if buf.len() >= end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&buf).to_string();
            let method = text.split(' ').next().unwrap_or("").to_string();
            let key = text.lines().find_map(|l| l.strip_prefix("apikey: ")).unwrap_or("").to_string();
            let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
            let answer = serde_json::json!({ "method": method, "key": key, "body": body }).to_string();
            let _ = write!(c, "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
        });
        format!("http://{addr}/rest")
    }

    fn wait_for_web(rt: &mut Runtime) {
        for _ in 0..300 {
            rt.tick(0.0);
            if !rt.has_timers() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("no answer");
    }

    fn web_project(url: &str) -> Project {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let s = &mut p.screens[0];
        s.components.push(Component { name: "api".into(), kind: CompKind::Web });
        let read = blk("mapFromJson", vec![t("got"), Arg::Block(Box::new(blk("webResponse", vec![], vec![], vec![])))], vec![], vec![]);
        let code = blk("setVarInt", vec![t("code"), Arg::Block(Box::new(blk("webStatus", vec![], vec![], vec![])))], vec![], vec![]);
        let failed = blk("setVarString", vec![t("problem"), Arg::Block(Box::new(blk("webResponse", vec![], vec![], vec![])))], vec![], vec![]);
        s.events[0].blocks = vec![
            blk("webSetHeader", vec![t("api"), t("apikey"), t("k123")], vec![], vec![]),
            blk("webRequest", vec![t("api"), t("POST"), t(url), t(r#"{"a":1}"#)], vec![read, code.clone()], vec![failed, code]),
        ];
        p
    }

    #[test]
    fn a_web_request_answers_later() {
        let mut rt = Runtime::new(web_project(&serve_once(201)), 0.0);
        assert!(rt.has_timers(), "waiting for the answer");
        assert!(rt.maps.get("got").is_none());
        wait_for_web(&mut rt);
        assert_eq!(rt.maps["got"]["method"], "POST");
        assert_eq!(rt.maps["got"]["key"], "k123");
        assert_eq!(rt.maps["got"]["body"], r#"{"a":1}"#);
        assert_eq!(rt.vars["code"].num(), 201.0);
    }

    #[test]
    fn a_failed_web_request_runs_the_other_branch() {
        let mut rt = Runtime::new(web_project(&serve_once(404)), 0.0);
        wait_for_web(&mut rt);
        assert!(rt.maps.get("got").is_none());
        assert_eq!(rt.vars["code"].num(), 404.0);
        assert!(rt.vars["problem"].text().contains("POST"), "the error answer is still readable");
        // nothing listening: code 0 and curl's reason
        let mut rt = Runtime::new(web_project("http://127.0.0.1:9/none"), 0.0);
        wait_for_web(&mut rt);
        assert_eq!(rt.vars["code"].num(), 0.0);
        assert!(!rt.vars["problem"].text().is_empty());
        let mut rt = Runtime::new(web_project("not a url"), 0.0);
        wait_for_web(&mut rt);
        assert!(rt.vars["problem"].text().contains("Not a web address"));
    }

    /// One HTTP request read off a connection: method, path, headers, body.
    fn read_request(c: &mut std::net::TcpStream) -> (String, String, HashMap<String, String>, String) {
        use std::io::Read;
        let mut buf = vec![];
        let mut chunk = [0u8; 4096];
        loop {
            let n = c.read(&mut chunk).unwrap_or(0);
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let len = text[..end].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                if buf.len() >= end + 4 + len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        let text = String::from_utf8_lossy(&buf).to_string();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let mut lines = head.lines();
        let first: Vec<&str> = lines.next().unwrap_or("").split(' ').collect();
        let headers = lines.filter_map(|l| l.split_once(": ")).map(|(k, v)| (k.to_ascii_lowercase(), v.to_string())).collect();
        (first[0].to_string(), first.get(1).unwrap_or(&"").to_string(), headers, body.to_string())
    }

    /// A pretend Supabase: password sign-in, one expired token that has to be
    /// renewed, and a notes table. Every request is written down.
    pub(crate) fn fake_supabase() -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::Write;
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let log = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let seen = log.clone();
        std::thread::spawn(move || {
            for c in l.incoming() {
                let Ok(mut c) = c else { break };
                let (method, path, h, body) = read_request(&mut c);
                let auth = h.get("authorization").cloned().unwrap_or_default();
                seen.lock().unwrap().push(format!("{method} {path} {auth} {body}"));
                let key_ok = h.get("apikey").map(String::as_str) == Some("anon");
                let session = |a: &str| serde_json::json!({"access_token": a, "refresh_token": "R1", "user": {"id": "u1", "email": "ada@x.io"}}).to_string();
                let (code, answer) = if !key_ok {
                    (401, r#"{"message":"Invalid API key"}"#.to_string())
                } else if path == "/auth/v1/token?grant_type=password" {
                    if body.contains("\"secret1\"") {
                        (200, session("A1"))
                    } else {
                        (400, r#"{"error":"invalid_grant","error_description":"Invalid login credentials"}"#.to_string())
                    }
                } else if path == "/auth/v1/token?grant_type=refresh_token" {
                    (200, session("A2"))
                } else if path.starts_with("/rest/v1/notes") && auth == "Bearer A1" {
                    (401, r#"{"message":"JWT expired"}"#.to_string())
                } else if method == "GET" && path == "/rest/v1/notes?select=*&owner=eq.u1" {
                    (200, r#"[{"id":1,"text":"hi there"}]"#.to_string())
                } else if method == "POST" && path == "/rest/v1/notes" {
                    (201, format!("[{body}]"))
                } else {
                    (404, r#"{"message":"no such thing"}"#.to_string())
                };
                let _ = write!(c, "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
            }
        });
        (format!("http://{addr}"), log)
    }

    fn supa_project(url: &str, events: serde_json::Value) -> Project {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.supabase = Some(SupabaseLink { url: url.into(), key: "anon".into() });
        let spec = serde_json::json!({"variables": [{"name": "problem", "type": "text"}, {"name": "note", "type": "map"}, {"name": "rows", "type": "list_map"}], "events": events});
        p.screens[0] = crate::templates::build(&spec, "main", &Default::default()).unwrap();
        p
    }

    #[test]
    fn supabase_signs_in_renews_and_reads_rows() {
        let (url, log) = fake_supabase();
        let problem = serde_json::json!([{"op": "setVarString", "args": ["problem", {"op": "supaError"}]}]);
        let p = supa_project(&url, serde_json::json!([{"target": "screen", "event": "onCreate", "blocks": [
            {"op": "supaSignIn", "args": ["ada@x.io ", "secret1"], "do": [
                {"op": "supaSelectWhere", "args": ["notes", "owner", {"op": "supaUserId"}],
                 "do": [{"op": "listMapFromJson", "args": ["rows", {"op": "webResponse"}]}], "else": problem},
                {"op": "mapPut", "args": ["note", "text", "a b&c"]},
                {"op": "supaInsert", "args": ["note", "notes"], "do": [], "else": problem},
            ], "else": problem},
        ]}]));
        let mut rt = Runtime::new(p, 0.0);
        wait_for_web(&mut rt);
        assert_eq!(rt.vars.get("problem").map(|v| v.text()).unwrap_or_default(), "");
        assert_eq!(rt.supa["email"], "ada@x.io");
        assert_eq!(rt.supa["access"], "A2", "renewed after the 401");
        assert_eq!(rt.list_maps["rows"][0]["text"], "hi there");
        let log = log.lock().unwrap().join("\n");
        assert!(log.contains(r#"POST /auth/v1/token?grant_type=password Bearer anon {"email":"ada@x.io","password":"secret1"}"#), "{log}");
        assert!(log.contains("GET /rest/v1/notes?select=*&owner=eq.u1 Bearer A1"), "{log}");
        assert!(log.contains("POST /auth/v1/token?grant_type=refresh_token Bearer anon"), "{log}");
        assert!(log.contains("GET /rest/v1/notes?select=*&owner=eq.u1 Bearer A2"), "{log}");
        assert!(log.contains(r#"POST /rest/v1/notes Bearer A2 {"text":"a b&c"}"#), "{log}");
        // signing out forgets the person
        let mut rt2 = rt.clone();
        rt2.run_blocks(&[Block { id: new_id(), op: "supaSignOut".into(), args: vec![], sub1: vec![], sub2: vec![] }], HashMap::new(), "out".into());
        assert!(rt2.supa.is_empty());
    }

    #[test]
    fn supabase_errors_read_plainly() {
        let (url, _) = fake_supabase();
        let events = serde_json::json!([{"target": "screen", "event": "onCreate", "blocks": [
            {"op": "supaSignIn", "args": ["ada@x.io", "wrong"], "do": [], "else": [{"op": "setVarString", "args": ["problem", {"op": "supaError"}]}]},
        ]}]);
        let mut rt = Runtime::new(supa_project(&url, events.clone()), 0.0);
        wait_for_web(&mut rt);
        assert_eq!(rt.vars["problem"].text(), "Invalid login credentials");
        assert!(rt.supa.is_empty());
        let mut p = supa_project(&url, events);
        p.supabase = None;
        let mut rt = Runtime::new(p, 0.0);
        wait_for_web(&mut rt);
        assert!(rt.vars["problem"].text().contains("isn't linked"));
    }

    #[test]
    fn the_supabase_login_template_runs() {
        let (url, _) = fake_supabase();
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.supabase = Some(SupabaseLink { url, key: "anon".into() });
        p.screens[0] = crate::templates::screen(crate::templates::find("supabase_login").unwrap(), "main", &Default::default()).unwrap();
        let mut rt = Runtime::new(p, 0.0);
        rt.widgets.get_mut("email").unwrap().text = "ada@x.io".into();
        rt.widgets.get_mut("password").unwrap().text = "wrong".into();
        rt.fire("signIn", "onClick", HashMap::new());
        assert_eq!(rt.widgets["status"].text, "Signing in…");
        wait_for_web(&mut rt);
        assert_eq!(rt.widgets["status"].text, "Invalid login credentials");
        rt.widgets.get_mut("password").unwrap().text = "secret1".into();
        rt.fire("signIn", "onClick", HashMap::new());
        wait_for_web(&mut rt);
        assert_eq!(rt.widgets["status"].text, "");
        assert_eq!(rt.widgets["subtitle"].text, "Signed in as ada@x.io");
    }

    #[test]
    fn debugger_pauses_steps_and_continues() {
        let p = crate::app::sample_project(601);
        let click = p.screens[0].events.iter().find(|e| e.target == "button1").unwrap().clone();
        let last = click.blocks.last().unwrap().id;
        let mut bps = HashSet::new();
        bps.insert(last);
        let mut rt = Runtime::with_debug(p, 0.0, bps, false);
        rt.fire("button1", "onClick", HashMap::new());
        let pause = rt.pause.clone().expect("paused at the breakpoint");
        assert_eq!(pause.block, last);
        assert_eq!(rt.vars["count"].num(), 1.0, "blocks before the breakpoint ran");
        assert_ne!(rt.widgets["textview1"].text, "1", "the breakpoint block hasn't run");
        // taps wait while paused
        rt.fire("button1", "onClick", HashMap::new());
        assert_eq!(rt.vars["count"].num(), 1.0);
        rt.debug_step();
        assert!(rt.pause.is_none(), "that was the event's last block");
        assert_eq!(rt.widgets["textview1"].text, "1");
        // step mode pauses before the first block; Continue stops at the breakpoint
        rt.step_mode = true;
        rt.fire("button1", "onClick", HashMap::new());
        assert_eq!(rt.pause.as_ref().unwrap().at, 0);
        assert_eq!(rt.vars["count"].num(), 1.0);
        rt.debug_step();
        assert_eq!(rt.pause.as_ref().unwrap().at, 1);
        rt.debug_continue();
        assert_eq!(rt.pause.as_ref().unwrap().block, last);
        rt.debug_finish();
        assert!(rt.pause.is_none());
        assert!(!rt.step_mode);
        assert_eq!(rt.widgets["textview1"].text, "2");
        assert!(rt.trace.iter().any(|l| l.kind == TraceKind::Event));
        assert!(rt.trace.iter().any(|l| l.kind == TraceKind::Change && l.text.contains("count")));
    }

    #[test]
    fn native_code_for_both_phones() {
        let p = crate::app::sample_project(601);
        let s = &p.screens[0];
        let kt = crate::native::kotlin(&p, s);
        assert!(kt.contains("class MainActivity : Activity()"), "{kt}");
        assert!(kt.contains("button1.setOnClickListener {"), "{kt}");
        assert!(kt.contains("private var count = 0.0"), "{kt}");
        assert!(!kt.contains("not available"), "{kt}");
        let sw = crate::native::swift_screen(&p, s);
        assert!(sw.contains("final class MainModel: ObservableObject"), "{sw}");
        assert!(sw.contains("func button1_onClick()"), "{sw}");
        assert!(sw.contains("struct MainScreen: View"), "{sw}");
        assert!(!sw.contains("not available"), "{sw}");
        assert_eq!(sw.matches('{').count(), sw.matches('}').count());
        assert_eq!(kt.matches('{').count(), kt.matches('}').count());
        assert!(crate::native::swift_app(&p).contains("NavigationStack"));
    }

    /// Writes the sample's native files to $BLOCKWISE_NATIVE_OUT (for compiling them by hand).
    #[test]
    #[ignore]
    fn write_native_sample() {
        let out = std::env::var("BLOCKWISE_NATIVE_OUT").expect("set BLOCKWISE_NATIVE_OUT");
        let mut p = crate::app::sample_project(601);
        p.screens.push(crate::app::sample_project(602).screens[0].clone());
        p.screens[1].name = "second".into();
        crate::native::export_native(&p, std::path::Path::new(&out), true, true).unwrap();
    }

    /// A screen that uses every block once (for compiling the native code by hand).
    pub(crate) fn every_block_project() -> Project {
        use crate::blocks::{parse_spec, Param, Shape, Token, DEFS};
        let mut p = Project::new(1, "All Blocks", "All Blocks", "com.my.allblocks");
        p.screens.push(Screen::new("second"));
        let s = &mut p.screens[0];
        for (id, k) in [("button1", WKind::Button), ("textview1", WKind::TextView), ("edittext1", WKind::EditText), ("checkbox1", WKind::CheckBox), ("seekbar1", WKind::SeekBar), ("listview1", WKind::ListView), ("webview1", WKind::WebView), ("spinner1", WKind::Spinner), ("switch1", WKind::Switch), ("linear1", WKind::LinearH), ("progressbar1", WKind::ProgressBar), ("imageview1", WKind::ImageView), ("calendarview1", WKind::CalendarView), ("videoview1", WKind::VideoView)] {
            s.widgets.push(Widget::new(id.into(), k, "root".into()));
        }
        for (n, ty) in [("n", VarType::Num), ("s", VarType::Str), ("b", VarType::Bool), ("m", VarType::Map)] {
            s.vars.push(Var { name: n.into(), ty });
        }
        for (n, ty) in [("ln", ListType::Num), ("ls", ListType::Str), ("lm", ListType::Map)] {
            s.lists.push(ListVar { name: n.into(), ty });
        }
        for (n, kind) in [("intent1", CompKind::Intent), ("file1", CompKind::SharedPref), ("timer1", CompKind::Timer), ("dialog1", CompKind::Dialog), ("vibrator1", CompKind::Vibrator), ("calendar1", CompKind::Calendar), ("web1", CompKind::Web), ("player1", CompKind::Sound), ("camera1", CompKind::Camera)] {
            s.components.push(Component { name: n.into(), kind });
        }
        let menu = |m: &str| -> &str {
            match m {
                "calendar" => "calendar1", "checkbox" => "checkbox1", "color" => "0xFFFF0000", "dialog" => "dialog1",
                "edittext" => "edittext1", "file" => "file1", "intent" => "intent1", "list" | "listStr" => "ls",
                "listMap" => "lm", "listNum" => "ln", "listspn" => "listview1", "screen" => "SecondActivity",
                "seekbar" => "seekbar1", "textview" => "textview1", "timer" => "timer1", "varBool" => "b",
                "varMap" => "m", "varNum" => "n", "varStr" => "s", "vibrator" => "vibrator1", "view" => "button1",
                "visible" => "VISIBLE", "webview" => "webview1", "web" => "web1", "method" => "POST",
                "imageview" => "imageview1", "image" => "cat", "sound" => "ding", "player" => "player1", "videoview" => "videoview1", "video" => "clip", "font" => "chalk", "asset" => "words", "camera" => "camera1", "mediaKind" => "picture", _ => "button1",
            }
        };
        let simple = |op: &str, args: Vec<Arg>| Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let toast = || simple("doToast", vec![Arg::Text("hi".into())]);
        let mut create = vec![];
        let mut special: Vec<(&str, &str, Block)> = vec![];
        for d in DEFS {
            if d.op == "break" {
                continue;
            }
            let mut args = vec![];
            for t in parse_spec(d.spec) {
                match t {
                    Token::Param(i, p) => {
                        while args.len() < i {
                            args.push(Arg::Text(String::new()));
                        }
                        args.push(match p {
                            Param::Text => Arg::Text("x".into()),
                            Param::Num => Arg::Text("1".into()),
                            Param::Bool => Arg::Block(Box::new(simple("true", vec![]))),
                            Param::Menu(m) => Arg::Text(menu(&m).into()),
                        });
                    }
                    Token::ArgLabel(i) => {
                        while args.len() < i {
                            args.push(Arg::Text(String::new()));
                        }
                        args.push(Arg::Text(if d.op == "moreBlock" { "helper".into() } else { "n".into() }));
                    }
                    Token::Label(_) => {}
                }
            }
            let mut b = simple(d.op, args);
            if d.op == "forever" {
                b.sub1 = vec![simple("break", vec![])];
            } else {
                b.sub1 = vec![toast()];
                b.sub2 = vec![toast()];
            }
            let wrapped = match d.shape {
                Shape::Num => simple("setVarInt", vec![Arg::Text("n".into()), Arg::Block(Box::new(b))]),
                Shape::Str => simple("setVarString", vec![Arg::Text("s".into()), Arg::Block(Box::new(b))]),
                Shape::Bool => simple("setVarBoolean", vec![Arg::Text("b".into()), Arg::Block(Box::new(b))]),
                _ => b,
            };
            match d.op {
                "eventPosition" => special.push(("listview1", "onItemClicked", wrapped)),
                "eventChecked" => special.push(("checkbox1", "onCheckedChange", wrapped)),
                "eventText" => special.push(("edittext1", "onTextChanged", wrapped)),
                "eventProgress" => special.push(("seekbar1", "onProgressChanged", wrapped)),
                _ => create.push(wrapped),
            }
        }
        s.events[0].blocks = create;
        for (t, n, b) in special {
            s.events.push(Event { target: t.into(), name: n.into(), blocks: vec![b], loose: vec![], params: vec![] });
        }
        for (t, n) in [("button1", "onClick"), ("button1", "onLongClick"), ("spinner1", "onItemSelected"), ("switch1", "onCheckedChange"), ("", "onBackPressed")] {
            s.events.push(Event { target: t.into(), name: n.into(), blocks: vec![toast()], loose: vec![], params: vec![] });
        }
        s.events.push(Event { target: "moreBlock".into(), name: "helper".into(), blocks: vec![toast()], loose: vec![], params: vec![] });
        // a more block with an input of each type, read inside it and used
        let read = |op: &str, n: &str| Arg::Block(Box::new(simple(op, vec![Arg::Text(n.into())])));
        let params = vec![Var { name: "x".into(), ty: VarType::Num }, Var { name: "w".into(), ty: VarType::Str }, Var { name: "f".into(), ty: VarType::Bool }];
        let inside = vec![
            simple("setVarInt", vec![Arg::Text("n".into()), read("paramNum", "x")]),
            simple("setVarString", vec![Arg::Text("s".into()), read("paramStr", "w")]),
            simple("setVarBoolean", vec![Arg::Text("b".into()), read("paramBool", "f")]),
        ];
        s.events.push(Event { target: "moreBlock".into(), name: "withInputs".into(), blocks: inside, loose: vec![], params });
        s.events[0].blocks.push(simple("moreBlock", vec![Arg::Text("withInputs".into()), Arg::Text("2".into()), Arg::Text("two".into()), Arg::Block(Box::new(simple("true", vec![])))]));
        p
    }

    #[test]
    fn decimals_round_like_android() {
        for (x, d, want) in [
            (2.675, 2.0, "2.68"),
            (0.125, 2.0, "0.13"),
            (-2.5, 0.0, "-3"),
            (0.5, 0.0, "1"),
            (9.995, 2.0, "10.00"),
            (1e20, 2.0, "100000000000000000000.00"),
            (-0.001, 2.0, "0.00"),
            (1.5e-6, 7.0, "0.0000015"),
            (3.0, 0.0, "3"),
            (123.456, -1.0, "123"),
            (1.0, 3.9, "1.000"),
            (f64::NAN, 2.0, "NaN"),
        ] {
            assert_eq!(fixed_text(x, d), want, "{x} with {d}");
        }
    }

    #[test]
    fn more_blocks_take_inputs() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let s = &mut p.screens[0];
        s.vars.push(Var { name: "out".into(), ty: VarType::Str });
        let blk = |op: &str, args: Vec<Arg>| Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let read = |op: &str, n: &str| Arg::Block(Box::new(blk(op, vec![Arg::Text(n.into())])));
        // greet name times: out = name repeated
        let body = vec![blk(
            "repeat",
            vec![read("paramNum", "times")],
        )];
        let mut body = body;
        body[0].sub1 = vec![blk("setVarString", vec![Arg::Text("out".into()), Arg::Block(Box::new(blk("stringJoin", vec![Arg::Block(Box::new(blk("getVar", vec![Arg::Text("out".into())]))), read("paramStr", "name")])))])];
        s.events.push(Event { target: "moreBlock".into(), name: "greet".into(), blocks: body, loose: vec![], params: vec![Var { name: "name".into(), ty: VarType::Str }, Var { name: "times".into(), ty: VarType::Num }] });
        s.events[0].blocks = vec![blk("moreBlock", vec![Arg::Text("greet".into()), Arg::Text("hi".into()), Arg::Text("3".into())])];
        let rt = Runtime::new(p.clone(), 0.0);
        assert_eq!(rt.vars.get("out").map(|v| v.text()), Some("hihihi".to_string()));
        // the phones' code: a function with the inputs, called with values
        let kt = crate::native::kotlin(&p, &p.screens[0]);
        assert!(kt.contains("private fun _greet(_p_name: String, _p_times: Double)"), "{kt}");
        assert!(kt.contains("_greet(\"hi\", 3.0)"), "{kt}");
        let sw = crate::native::swift_screen(&p, &p.screens[0]);
        assert!(sw.contains("func _greet(_ _p_name: String, _ _p_times: Double)"), "{sw}");
        assert!(sw.contains("_greet(\"hi\", 3.0)"), "{sw}");
        // renaming an input and adding one before it: uses keep their values
        let s = &mut p.screens[0];
        crate::logic::apply_more_edit(s, "greet", "say", vec![Var { name: "loud".into(), ty: VarType::Bool }, Var { name: "word".into(), ty: VarType::Str }, Var { name: "times".into(), ty: VarType::Num }], &[None, Some(0), Some(1)]);
        let call = &s.events[0].blocks[0];
        assert_eq!(call.args, vec![Arg::Text("say".into()), Arg::Text(String::new()), Arg::Text("hi".into()), Arg::Text("3".into())]);
        let def = s.events.iter().find(|e| e.target == "moreBlock").unwrap();
        assert_eq!(def.title(), "define say (loud) (word) (times)");
        let inner = &def.blocks[0].sub1[0];
        let Arg::Block(join) = &inner.args[1] else { panic!() };
        let Arg::Block(word) = &join.args[1] else { panic!() };
        assert_eq!((word.op.as_str(), &word.args[0]), ("paramStr", &Arg::Text("word".into())));
        // taking an input away leaves an empty slot where it was read
        crate::logic::apply_more_edit(s, "say", "say", vec![Var { name: "word".into(), ty: VarType::Str }], &[Some(1)]);
        let def = s.events.iter().find(|e| e.target == "moreBlock").unwrap();
        assert_eq!(def.blocks[0].args[0], Arg::Text(String::new()));
        assert_eq!(s.events[0].blocks[0].args, vec![Arg::Text("say".into()), Arg::Text("hi".into())]);
    }

    #[test]
    fn sound_players_say_what_to_play() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.files.push(ProjFile { name: "ding".into(), ext: "wav".into(), kind: FileKind::Sound });
        let s = &mut p.screens[0];
        s.components.push(Component { name: "player".into(), kind: CompKind::Sound });
        let blk = |op: &str, args: Vec<Arg>| Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let t = |s: &str| Arg::Text(s.into());
        let tru = || Arg::Block(Box::new(blk("true", vec![])));
        s.events[0].blocks = vec![
            blk("soundLoop", vec![t("player"), tru()]),
            blk("soundVolume", vec![t("player"), t("40")]),
            blk("soundPlay", vec![t("player"), t("ding")]),
            blk("soundSeek", vec![t("player"), t("250")]),
        ];
        let mut rt = Runtime::new(p.clone(), 0.0);
        let j = rt.sounds["player"].clone();
        assert_eq!((j.source.as_str(), j.gen, j.looping, j.volume, j.seek), ("ding", 1, true, 0.4, Some(250.0)));
        assert!(rt.playing.contains("player"));
        // the same sound again starts over; stop forgets it, and resume can't bring it back
        let again = blk("soundPlay", vec![t("player"), t("ding")]);
        rt.exec(&again);
        assert_eq!(rt.sounds["player"].gen, 2);
        rt.exec(&blk("soundStop", vec![t("player")]));
        rt.exec(&blk("soundResume", vec![t("player")]));
        assert!(!rt.playing.contains("player"));
        assert!(rt.sounds["player"].source.is_empty());
        // a sound that isn't one of the project's: nothing plays
        rt.exec(&blk("soundPlay", vec![t("player"), t("nope")]));
        assert!(!rt.playing.contains("player"));
        // pause and resume keep the sound
        rt.exec(&blk("soundPlayFrom", vec![t("player"), t("https://example.com/a.mp3")]));
        rt.exec(&blk("soundPause", vec![t("player")]));
        assert!(!rt.playing.contains("player"));
        rt.exec(&blk("soundResume", vec![t("player")]));
        assert!(rt.playing.contains("player"));
        assert_eq!(rt.sounds["player"].source, "https://example.com/a.mp3");
    }

    #[test]
    fn camera_and_phone_files() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let s = &mut p.screens[0];
        s.widgets.push(Widget::new("imageview1".into(), WKind::ImageView, "root".into()));
        s.widgets.push(Widget::new("videoview1".into(), WKind::VideoView, "root".into()));
        s.components.push(Component { name: "cam".into(), kind: CompKind::Camera });
        s.components.push(Component { name: "player".into(), kind: CompKind::Sound });
        s.vars.push(Var { name: "got".into(), ty: VarType::Str });
        let blk = |op: &str, args: Vec<Arg>| Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let t = |s: &str| Arg::Text(s.into());
        let path = || Arg::Block(Box::new(blk("mediaPath", vec![])));
        let mut photo = blk("cameraPhoto", vec![t("cam")]);
        photo.sub1 = vec![blk("setImageFrom", vec![t("imageview1"), path()])];
        photo.sub2 = vec![blk("setVarString", vec![t("got"), t("cancelled")])];
        let mut pick = blk("pickMedia", vec![t("video")]);
        pick.sub1 = vec![blk("setVideoFrom", vec![t("videoview1"), path()])];
        s.events[0].blocks = vec![
            photo,
            pick,
            blk("setImageFrom", vec![t("imageview1"), t("https://example.com/cat.png")]),
            blk("soundPlayFrom", vec![t("player"), t("/tmp/song.mp3")]),
            // outside a camera or pick block there is no file
            blk("setVarString", vec![t("got"), Arg::Block(Box::new(blk("stringJoin", vec![r_var("got"), path()])))]),
        ];
        fn r_var(n: &str) -> Arg {
            Arg::Block(Box::new(Block { id: new_id(), op: "getVar".into(), args: vec![Arg::Text(n.into())], sub1: vec![], sub2: vec![] }))
        }
        // the preview has no camera: "if cancelled" runs, with a message
        let rt = Runtime::new(p.clone(), 0.0);
        assert_eq!(rt.vars.get("got").map(|v| v.text()), Some("cancelled".to_string()));
        assert!(rt.toasts.iter().any(|(m, _)| m.contains("take a photo")), "{:?}", rt.toasts);
        assert_eq!(rt.widgets["imageview1"].image, "https://example.com/cat.png");
        assert!(rt.playing.contains("player"));
        let kt = crate::native::kotlin(&p, &p.screens[0]);
        assert!(kt.contains("AppMedia.photo(_activity) { _ok, _filePath ->"), "{kt}");
        assert!(kt.contains("AppUtil.setPicture(imageview1, _filePath)"), "{kt}");
        assert!(kt.contains("AppMedia.pick(_activity, \"video\") {"), "{kt}");
        assert!(kt.contains("videoview1_video.load(_filePath)"), "{kt}");
        assert!(kt.contains("override fun onActivityResult("), "{kt}");
        assert!(kt.contains("got = (got + \"\")"), "{kt}");
        let sw = crate::native::swift_screen(&p, &p.screens[0]);
        assert!(sw.contains("MediaPicker.shared.camera(video: false) { [weak self] _ok, _filePath in"), "{sw}");
        assert!(sw.contains("imageview1.image = _filePath"), "{sw}");
        assert!(sw.contains("MediaPicker.shared.pick(\"video\")"), "{sw}");
        assert!(sw.contains("player.play(\"/tmp/song.mp3\")"), "{sw}");
        assert!(crate::native::kotlin_manifest(&p).contains("android:name=\".MediaFiles\" android:authorities=\"com.my.t.media\""));
        // a screen without them has no onActivityResult
        p.screens[0].events[0].blocks.clear();
        assert!(!crate::native::kotlin(&p, &p.screens[0]).contains("onActivityResult"));
    }

    #[test]
    fn new_text_and_list_blocks() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let s = &mut p.screens[0];
        for (n, ty) in [("ls", ListType::Str), ("ln", ListType::Num)] {
            s.lists.push(ListVar { name: n.into(), ty });
        }
        for (n, ty) in [("t", VarType::Str), ("x", VarType::Num), ("ok", VarType::Bool)] {
            s.vars.push(Var { name: n.into(), ty });
        }
        let blk = |op: &str, args: Vec<Arg>| Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let t = |s: &str| Arg::Text(s.into());
        let r = |b: Block| Arg::Block(Box::new(b));
        s.events[0].blocks = vec![
            blk("splitToList", vec![t("pear,Apple,banana,,"), t(","), t("ls")]),
            blk("sortListStr", vec![t("ls")]),
            blk("setVarString", vec![t("t"), r(blk("joinList", vec![t("ls"), t("|")]))]),
            blk("addListInt", vec![t("3"), t("ln")]),
            blk("addListInt", vec![t("-1"), t("ln")]),
            blk("addListInt", vec![t("2.5"), t("ln")]),
            blk("sortListInt", vec![t("ln")]),
            blk("reverseList", vec![t("ln")]),
            blk("setVarInt", vec![t("x"), r(blk("sumList", vec![t("ln")]))]),
            blk("setVarBoolean", vec![t("ok"), r(blk("&&", vec![r(blk("stringStartsWith", vec![t("Hello"), t("He")])), r(blk("isNumber", vec![t(" 4.5 ")]))]))]),
        ];
        let rt = Runtime::new(p.clone(), 0.0);
        assert_eq!(rt.vars.get("t").map(|v| v.text()), Some("||Apple|banana|pear".to_string()));
        assert_eq!(rt.lists.get("ln").map(|l| l.iter().map(|v| v.num()).collect::<Vec<_>>()), Some(vec![3.0, 2.5, -1.0]));
        assert_eq!(rt.vars.get("x").map(|v| v.num()), Some(4.5));
        assert_eq!(rt.vars.get("ok").map(|v| v.truth()), Some(true));
    }

    #[test]
    fn every_block_has_native_code() {
        let p = every_block_project();
        let kt = crate::native::kotlin(&p, &p.screens[0]);
        let sw = crate::native::swift_screen(&p, &p.screens[0]);
        for (lang, src) in [("Kotlin", &kt), ("Swift", &sw)] {
            assert!(!src.contains("not available"), "{lang}: {src}");
            assert!(!src.contains("unknown block"), "{lang}");
            assert_eq!(src.matches('{').count(), src.matches('}').count(), "{lang}");
        }
    }

    #[test]
    #[ignore]
    fn write_every_block_native() {
        let out = std::env::var("BLOCKWISE_NATIVE_OUT").expect("set BLOCKWISE_NATIVE_OUT");
        crate::native::export_native(&every_block_project(), std::path::Path::new(&out), true, true).unwrap();
    }

    #[test]
    fn bad_blocks_crash_like_the_real_app() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        let s = &mut p.screens[0];
        s.lists.push(ListVar { name: "names".into(), ty: ListType::Str });
        s.vars.push(Var { name: "n".into(), ty: VarType::Num });
        s.components.push(Component { name: "t".into(), kind: CompKind::Timer });
        let get = Block { id: new_id(), op: "getAtListStr".into(), args: vec![Arg::Text("3".into()), Arg::Text("names".into())], sub1: vec![], sub2: vec![] };
        let toast = Block { id: new_id(), op: "doToast".into(), args: vec![Arg::Block(Box::new(get))], sub1: vec![], sub2: vec![] };
        let after = Block { id: new_id(), op: "setVarInt".into(), args: vec![Arg::Text("n".into()), Arg::Text("5".into())], sub1: vec![], sub2: vec![] };
        let timer = Block { id: new_id(), op: "timerEvery".into(), args: vec![Arg::Text("t".into()), Arg::Text("100".into()), Arg::Text("100".into())], sub1: vec![], sub2: vec![] };
        s.events[0].blocks = vec![timer];
        s.events.push(Event { target: "".into(), name: "onBackPressed".into(), blocks: vec![toast, after], loose: vec![], params: vec![] });
        let mut rt = Runtime::new(p, 0.0);
        assert!(rt.has_timers());
        rt.press_back();
        let c = rt.crash.clone().expect("crashed");
        assert!(c.text.starts_with("java.lang.IndexOutOfBoundsException: Index 3 out of bounds for length 0"), "{}", c.text);
        assert_eq!(rt.vars["n"].num(), 0.0, "blocks after the crash don't run");
        assert!(rt.toasts.is_empty() && !rt.has_timers(), "the app is gone");
        assert!(rt.crash_screen().is_none());
        rt.tick(CRASH_RESTART_S + 0.01);
        assert!(rt.crash_screen().is_some());
        rt.press_back();
        rt.end_after_crash();
        assert!(rt.finished);
        // text that isn't a number
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens[0].vars.push(Var { name: "n".into(), ty: VarType::Num });
        let num = Block { id: new_id(), op: "toNumber".into(), args: vec![Arg::Text("abc".into())], sub1: vec![], sub2: vec![] };
        p.screens[0].events[0].blocks = vec![Block { id: new_id(), op: "setVarInt".into(), args: vec![Arg::Text("n".into()), Arg::Block(Box::new(num))], sub1: vec![], sub2: vec![] }];
        let rt = Runtime::new(p.clone(), 0.0);
        assert!(rt.crash.as_ref().unwrap().text.starts_with("java.lang.NumberFormatException: For input string: \"abc\""));
        // the exported app has the crash handler
        let km = crate::native::kotlin_manifest(&p);
        assert!(km.contains("android:name=\".CrashCatcher\"") && km.contains("android:name=\".CrashActivity\" android:process=\":crash\"") && km.contains("android:exported=\"true\""));
        assert!(!km.contains("package="));
        assert!(crate::native::kotlin_crash_activity(&p).contains("The app stopped"));
        assert!(crate::native::kotlin_crash_catcher(&p).contains("getStackTraceString"));
    }
}
