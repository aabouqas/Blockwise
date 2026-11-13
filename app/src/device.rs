//! Runs the app for real: builds the Kotlin project with Gradle and starts it on
//! an Android emulator, or compiles the Swift app and starts it on an iPhone
//! simulator. Everything happens on a background thread; the window reads the
//! shared state (step, log lines) every frame.

use crate::model::Project;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Android,
    Iphone,
}

impl Device {
    pub fn label(self) -> &'static str {
        match self {
            Device::Android => "Android emulator",
            Device::Iphone => "iOS simulator",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    /// What Blockwise is doing ("Building…").
    Step,
    /// A debug line from the app: an event started.
    Event,
    /// A block ran.
    Block,
    /// A variable changed.
    Value,
    /// Anything else the app or a tool printed.
    Output,
    /// Something went wrong.
    Problem,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Working,
    Running,
    Stopped,
    Failed,
}

pub struct Shared {
    pub phase: Phase,
    pub step: String,
    pub lines: Vec<(LineKind, String)>,
    pub started: Instant,
    /// Where a standalone build wrote its output (.apk or .app).
    pub built_path: Option<PathBuf>,
    /// The live log process (logcat / the simulator console), stopped on Stop.
    log_child: Option<Child>,
    /// Where it runs, for Stop: (adb, serial) or the simulator's id.
    stop_with: Option<StopWith>,
    cancel: bool,
    /// The run stopped because Android tools are missing: what would be
    /// downloaded, for the person to agree to.
    pub setup: Option<Vec<crate::android_setup::Need>>,
}

enum StopWith {
    Android { adb: PathBuf, serial: String, package: String },
    Iphone { udid: String, bundle: String },
}

const MAX_LINES: usize = 4000;

impl Shared {
    fn push(&mut self, kind: LineKind, text: impl Into<String>) {
        self.lines.push((kind, text.into()));
        if self.lines.len() > MAX_LINES {
            let cut = self.lines.len() - MAX_LINES;
            self.lines.drain(..cut);
        }
    }
}

pub struct DeviceRun {
    pub device: Device,
    pub app_name: String,
    pub build_only: bool,
    pub state: Arc<Mutex<Shared>>,
}

impl DeviceRun {
    /// Start building and running in the background, as a debug run (the
    /// log shows each event, block and value).
    pub fn start(device: Device, project: Project, ctx: egui::Context) -> DeviceRun {
        Self::run(device, project, ctx, true)
    }

    /// Start a run: with the block log (`debug`), or a plain build.
    pub fn run(device: Device, project: Project, ctx: egui::Context, debug: bool) -> DeviceRun {
        Self::launch(device, project, ctx, false, debug)
    }

    /// Build and export an installable APK or iOS app bundle in the background without launching an emulator.
    pub fn build(device: Device, project: Project, ctx: egui::Context) -> DeviceRun {
        Self::launch(device, project, ctx, true, false)
    }

    fn launch(device: Device, project: Project, ctx: egui::Context, build_only: bool, debug: bool) -> DeviceRun {
        let state = Arc::new(Mutex::new(Shared {
            phase: Phase::Working,
            step: "Getting ready…".into(),
            lines: vec![],
            started: Instant::now(),
            built_path: None,
            log_child: None,
            stop_with: None,
            cancel: false,
            setup: None,
        }));
        let st = state.clone();
        let app_name = project.app_name.clone();
        std::thread::spawn(move || {
            let res = match (device, build_only) {
                (Device::Android, false) => run_android(&project, &st, &ctx, debug),
                (Device::Iphone, false) => run_iphone(&project, &st, &ctx, debug),
                (Device::Android, true) => build_android(&project, &st, &ctx),
                (Device::Iphone, true) => build_iphone(&project, &st, &ctx),
            };
            let mut s = st.lock().unwrap();
            match res {
                Ok(()) => {
                    if s.phase == Phase::Running {
                        // the log ended: the app (or the emulator) closed
                        s.phase = Phase::Stopped;
                        s.step = "The app closed".into();
                    } else if build_only && s.phase == Phase::Working {
                        s.phase = Phase::Stopped;
                    }
                }
                Err(e) => {
                    if s.cancel {
                        s.phase = Phase::Stopped;
                        s.step = "Stopped".into();
                    } else {
                        s.phase = Phase::Failed;
                        eprintln!("[device] failed: {e}");
                        s.push(LineKind::Problem, e.clone());
                        s.step = e;
                    }
                }
            }
            ctx.request_repaint();
        });
        DeviceRun { device, app_name, build_only, state }
    }

    /// A run stopped for the first-time setup, as a computer without any
    /// Android tools shows it (for screenshots).
    pub fn setup_sample(app_name: String) -> DeviceRun {
        let need = crate::android_setup::needs(&crate::android_setup::Have::default(), true);
        let total: u32 = need.iter().map(|n| n.mb).sum();
        let msg = format!("Android isn't set up on this computer yet. Blockwise can download what it needs (about {}).", crate::android_setup::size(total));
        let state = Arc::new(Mutex::new(Shared {
            phase: Phase::Failed,
            step: msg.clone(),
            lines: vec![(LineKind::Problem, msg)],
            started: Instant::now(),
            built_path: None,
            log_child: None,
            stop_with: None,
            cancel: false,
            setup: Some(need),
        }));
        DeviceRun { device: Device::Android, app_name, build_only: false, state }
    }

    /// Let go of this run without closing the app: stop following its log
    /// and stop its build. Used when a new build replaces it, so the app
    /// stays open on the phone until the new one is installed over it (as
    /// Android Studio does).
    pub fn detach(&self) {
        let mut s = self.state.lock().unwrap();
        s.cancel = true;
        s.stop_with = None;
        if let Some(mut c) = s.log_child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if s.phase == Phase::Working || s.phase == Phase::Running {
            s.phase = Phase::Stopped;
            s.step = "Replaced by a new build".into();
        }
    }

    /// Close the app on the device and stop following its log.
    pub fn stop(&self) {
        let mut s = self.state.lock().unwrap();
        s.cancel = true;
        if let Some(mut c) = s.log_child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        match s.stop_with.take() {
            Some(StopWith::Android { adb, serial, package }) => {
                let _ = Command::new(adb).args(["-s", &serial, "shell", "am", "force-stop", &package]).output();
            }
            Some(StopWith::Iphone { udid, bundle }) => {
                let _ = Command::new("xcrun").args(["simctl", "terminate", &udid, &bundle]).output();
            }
            None => {}
        }
        if s.phase == Phase::Working || s.phase == Phase::Running {
            s.phase = Phase::Stopped;
            s.step = "Stopped".into();
        }
    }
}

impl Drop for DeviceRun {
    fn drop(&mut self) {
        if let Ok(mut s) = self.state.lock() {
            s.cancel = true;
            if let Some(mut c) = s.log_child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

type Res = Result<(), String>;

pub(crate) fn step(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, text: &str) -> Res {
    let mut s = st.lock().unwrap();
    if s.cancel {
        return Err("Stopped".into());
    }
    s.step = text.to_string();
    s.push(LineKind::Step, text);
    eprintln!("[device] {text}");
    ctx.request_repaint();
    Ok(())
}

pub(crate) fn say(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, kind: LineKind, text: impl Into<String>) {
    let text = text.into();
    eprintln!("[device] {kind:?}: {text}");
    st.lock().unwrap().push(kind, text);
    ctx.request_repaint();
}

/// Updates the step line in place (a download's progress), without a log line.
pub(crate) fn progress(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, text: &str) {
    st.lock().unwrap().step = text.to_string();
    ctx.request_repaint();
}

pub(crate) fn cancelled(st: &Arc<Mutex<Shared>>) -> bool {
    st.lock().unwrap().cancel
}

/// Run a tool to the end; on failure return the last lines it printed.
fn run_tool(cmd: &mut Command, what: &str) -> Result<String, String> {
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| format!("Couldn't start {what}: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(text)
    } else {
        let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let tail = tail[tail.len().saturating_sub(12)..].join("\n");
        Err(format!("{what} failed:\n{tail}"))
    }
}

/// Where Blockwise builds the app it runs on a device.
fn work_dir(p: &Project, device: &str) -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("Blockwise").join("device-builds").join(crate::codegen::sanitize(&p.name)).join(device)
}

/// Where standalone builds (`.apk` and `.app`) are saved for the user.
pub fn build_out_dir(p: &Project) -> PathBuf {
    dirs::document_dir().unwrap_or_else(std::env::temp_dir).join("Blockwise builds").join(crate::codegen::sanitize(&p.name))
}

/// Open the folder containing `path` (or reveal `path` in Finder / Explorer).
pub fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let folder = path.parent().unwrap_or(path);
        let _ = Command::new("xdg-open").arg(folder).spawn();
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Sort "9.1.0"-style version strings numerically.
fn version_key(v: &str) -> Vec<u32> {
    v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(0)).collect()
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

// ---------------------------------------------------------------- finding the tools

pub fn android_sdk() -> Option<PathBuf> {
    let mut c = vec![];
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Ok(v) = std::env::var(var) {
            c.push(PathBuf::from(v));
        }
    }
    c.push(home().join("Library/Android/sdk"));
    c.push(home().join("Android/Sdk"));
    if let Some(l) = dirs::data_local_dir() {
        c.push(l.join("Android/Sdk"));
    }
    // the one Blockwise downloaded on a computer without Android Studio
    c.push(crate::android_setup::managed_sdk());
    c.into_iter().find(|p| p.join("platform-tools").is_dir())
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// A Java the Android tools accept: JAVA_HOME, or the one inside Android Studio.
pub(crate) fn java_home() -> Option<PathBuf> {
    let mut c = vec![];
    if let Ok(v) = std::env::var("JAVA_HOME") {
        c.push(PathBuf::from(v));
    }
    c.push(PathBuf::from("/Applications/Android Studio.app/Contents/jbr/Contents/Home"));
    c.push(home().join("Applications/Android Studio.app/Contents/jbr/Contents/Home"));
    c.push(PathBuf::from("/opt/android-studio/jbr"));
    c.push(home().join("android-studio/jbr"));
    c.push(PathBuf::from("C:/Program Files/Android/Android Studio/jbr"));
    c.extend(crate::android_setup::managed_java());
    c.into_iter().find(|p| p.join("bin").join(exe("java")).is_file())
}

/// A gradle program: one Gradle already downloaded for other projects
/// (9.1.0 first, the version checked here, then the newest), or `gradle` on PATH.
pub(crate) fn gradle() -> Option<PathBuf> {
    let dists = home().join(".gradle/wrapper/dists");
    let mut found: Vec<(Vec<u32>, PathBuf)> = vec![];
    if let Ok(rd) = std::fs::read_dir(&dists) {
        for d in rd.flatten() {
            let name = d.file_name().to_string_lossy().to_string();
            let Some(ver) = name.strip_prefix("gradle-").map(|v| v.trim_end_matches("-all").trim_end_matches("-bin").to_string()) else { continue };
            if let Ok(hashes) = std::fs::read_dir(d.path()) {
                for h in hashes.flatten() {
                    let bin = h.path().join(format!("gradle-{ver}")).join("bin").join(if cfg!(windows) { "gradle.bat" } else { "gradle" });
                    if bin.is_file() {
                        found.push((version_key(&ver), bin));
                    }
                }
            }
        }
    }
    let k = version_key("9.1.0");
    if let Some((_, b)) = found.iter().find(|(v, _)| *v == k) {
        return Some(b.clone());
    }
    if let Some(b) = crate::android_setup::managed_gradle() {
        return Some(b);
    }
    found.sort();
    if let Some((_, b)) = found.into_iter().rev().find(|(v, _)| v.first().copied().unwrap_or(0) >= 9) {
        return Some(b);
    }
    which("gradle")
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(exe(name))).find(|p| p.is_file())
}

/// Versions and SDK paths for the Gradle files.
pub fn gradle_setup() -> crate::native::GradleSetup {
    let mut g = crate::native::GradleSetup::default();
    if let Some(sdk) = android_sdk() {
        // highest whole-number platform the plugin knows (up to 36)
        if let Ok(rd) = std::fs::read_dir(sdk.join("platforms")) {
            let best = rd.flatten().filter_map(|d| d.file_name().to_string_lossy().strip_prefix("android-").and_then(|v| v.parse::<u32>().ok())).filter(|v| *v <= 36).max();
            if let Some(b) = best {
                g.compile_sdk = b;
            }
        }
        if let Ok(rd) = std::fs::read_dir(sdk.join("build-tools")) {
            let mut tools: Vec<(Vec<u32>, PathBuf)> = rd.flatten().map(|d| (version_key(&d.file_name().to_string_lossy()), d.path().join(exe("aapt2")))).filter(|(_, p)| p.is_file()).collect();
            tools.sort();
            let pick = tools.iter().rev().find(|(v, _)| v.first().copied().unwrap_or(0) <= 36).or(tools.last());
            g.aapt2 = pick.map(|(_, p)| p.clone());
        }
        g.sdk_dir = Some(sdk);
    }
    g
}

// ---------------------------------------------------------------- Android

fn compile_android(p: &Project, st: &Arc<Mutex<Shared>>, ctx: &egui::Context, trace: bool) -> Result<(PathBuf, PathBuf), String> {
    // the first time on a computer without the tools: download them
    crate::android_setup::ensure(st, ctx, false)?;
    let sdk = android_sdk().ok_or("The Android SDK wasn't found. Install Android Studio (it brings the SDK and an emulator), then try again.")?;
    let java = java_home().ok_or("No Java was found for the Android build tools. Install Android Studio, or set JAVA_HOME.")?;
    let gradle = gradle().ok_or("Gradle wasn't found. Open any project once in Android Studio so it downloads Gradle, then try again.")?;

    step(st, ctx, if trace { "Writing the Kotlin project (debug build)…" } else { "Writing the Kotlin project…" })?;
    let dir = work_dir(p, "android");
    crate::native::write_kotlin_project(p, &dir, &gradle_setup(), trace).map_err(|e| format!("Couldn't write the project: {e}"))?;

    step(st, ctx, "Building with Gradle (the first build takes a minute)…")?;
    let build = |offline: bool| {
        let mut c = Command::new(&gradle);
        c.current_dir(&dir).env("JAVA_HOME", &java).env("ANDROID_HOME", &sdk).args(["-q", "assembleDebug", "--console=plain"]);
        if offline {
            c.arg("--offline");
        }
        run_tool(&mut c, "The Android build")
    };
    if let Err(first) = build(true) {
        say(st, ctx, LineKind::Output, "Some build parts aren't on this computer yet; downloading them…");
        build(false).map_err(|e| if e.contains("offline") { first } else { e })?;
    }
    let apk = dir.join("app/build/outputs/apk/debug/app-debug.apk");
    if !apk.is_file() {
        return Err("The build finished but no app file came out.".into());
    }
    say(st, ctx, LineKind::Output, "Built the app.");
    Ok((sdk, apk))
}

fn build_android(p: &Project, st: &Arc<Mutex<Shared>>, ctx: &egui::Context) -> Res {
    let (_, apk) = compile_android(p, st, ctx, false)?;
    let out_dir = build_out_dir(p);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Couldn't create build folder: {e}"))?;
    let dest = out_dir.join(format!("{}.apk", crate::codegen::sanitize(&p.name)));
    std::fs::copy(&apk, &dest).map_err(|e| format!("Couldn't save APK: {e}"))?;
    let msg = format!("Built APK → {}", dest.display());
    {
        let mut s = st.lock().unwrap();
        s.built_path = Some(dest);
        s.phase = Phase::Stopped;
        s.step = msg.clone();
        s.push(LineKind::Step, msg);
    }
    ctx.request_repaint();
    Ok(())
}

fn run_android(p: &Project, st: &Arc<Mutex<Shared>>, ctx: &egui::Context, debug: bool) -> Res {
    crate::android_setup::ensure(st, ctx, true)?;
    let sdk = android_sdk().ok_or("The Android SDK wasn't found. Install Android Studio (it brings the SDK and an emulator), then try again.")?;
    let adb = sdk.join("platform-tools").join(exe("adb"));

    // the emulator first: it starts while the app builds
    if android_device(&adb).is_none() {
        let emulator = sdk.join("emulator").join(exe("emulator"));
        let avd = pick_avd(&emulator).ok_or("There is no Android emulator yet, and Blockwise couldn't make one.")?;
        step(st, ctx, &format!("Opening the emulator ({})…", avd.replace('_', " ")))?;
        let mut cmd = Command::new(&emulator);
        cmd.args(["-avd", &avd, "-no-snapshot-save"]).env("ANDROID_AVD_HOME", crate::android_setup::avd_home()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        // its own process group: the emulator stays open when Blockwise closes
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        cmd.spawn().map_err(|e| format!("Couldn't start the emulator: {e}"))?;
        say(st, ctx, LineKind::Output, format!("The emulator ({avd}) is starting; the app builds meanwhile."));
    }

    // build; an app already open on the phone stays open meanwhile
    let (_, apk) = compile_android(p, st, ctx, debug)?;

    let t0 = Instant::now();
    let serial = loop {
        if let Some(s) = android_device(&adb) {
            break s;
        }
        if t0.elapsed().as_secs() == 0 {
            step(st, ctx, "Waiting for the emulator…")?;
        }
        if st.lock().unwrap().cancel {
            return Err("Stopped".into());
        }
        if t0.elapsed() > Duration::from_secs(180) {
            return Err("The emulator didn't start within 3 minutes.".into());
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    let mut asked = false;
    let t0 = Instant::now();
    loop {
        let out = Command::new(&adb).args(["-s", &serial, "shell", "getprop", "sys.boot_completed"]).output();
        if out.map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1").unwrap_or(false) {
            break;
        }
        if !asked {
            step(st, ctx, "Waiting for the emulator to finish starting…")?;
            asked = true;
        }
        if st.lock().unwrap().cancel {
            return Err("Stopped".into());
        }
        if t0.elapsed() > Duration::from_secs(240) {
            return Err("The emulator didn't finish starting within 4 minutes.".into());
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    // installing over the old app is what closes it, right before the new one opens
    step(st, ctx, "Installing the app…")?;
    run_tool(Command::new(&adb).args(["-s", &serial, "install", "-r", "-t"]).arg(&apk), "Installing")?;
    let _ = Command::new(&adb).args(["-s", &serial, "logcat", "-c"]).output();
    let first = p.screens.first().map(|s| s.activity_name()).unwrap_or_else(|| "MainActivity".into());
    step(st, ctx, "Opening it…")?;
    run_tool(Command::new(&adb).args(["-s", &serial, "shell", "am", "start", "-S", "-n", &format!("{}/.{}", p.package, first)]), "Opening the app")?;

    let child = Command::new(&adb)
        .args(["-s", &serial, "logcat", "-v", "brief", "-s", &format!("{}:V", crate::codegen::LOG_TAG), "AndroidRuntime:E"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Couldn't follow the app's log: {e}"))?;
    follow(st, ctx, child, StopWith::Android { adb: adb.clone(), serial: serial.clone(), package: p.package.clone() }, &format!("Running on the Android emulator ({serial})"), parse_logcat)
}

/// The first connected device or emulator that's ready.
fn android_device(adb: &Path) -> Option<String> {
    let out = Command::new(adb).arg("devices").output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().skip(1).find_map(|l| {
        let mut it = l.split_whitespace();
        match (it.next(), it.next()) {
            (Some(s), Some("device")) => Some(s.to_string()),
            _ => None,
        }
    })
}

/// A phone-shaped emulator if there is one.
pub(crate) fn pick_avd(emulator: &Path) -> Option<String> {
    let out = Command::new(emulator).arg("-list-avds").env("ANDROID_AVD_HOME", crate::android_setup::avd_home()).output().ok()?;
    let avds: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with("INFO")).collect();
    let phone = avds.iter().find(|a| {
        let l = a.to_lowercase();
        !["tablet", "tv", "wear", "auto", "desktop", "fold"].iter().any(|w| l.contains(w))
    });
    phone.or(avds.first()).cloned()
}

/// "D/Blockwise( 1234): event: When the screen opens" → (kind, text).
fn parse_logcat(line: &str) -> Option<(LineKind, String)> {
    // only the line ending: a value may end in a space
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() || line.starts_with("---------") {
        return None;
    }
    let msg = line.split_once("): ").map(|(_, m)| m).unwrap_or(line);
    if line.starts_with("E/") {
        return Some((LineKind::Problem, msg.to_string()));
    }
    Some(classify(msg))
}

fn classify(msg: &str) -> (LineKind, String) {
    if let Some(m) = msg.strip_prefix("event: ") {
        (LineKind::Event, m.to_string())
    } else if let Some(m) = msg.strip_prefix("block: ") {
        (LineKind::Block, m.to_string())
    } else if let Some(m) = msg.strip_prefix("value: ") {
        (LineKind::Value, m.to_string())
    } else if let Some(m) = msg.strip_prefix("crash: ") {
        (LineKind::Problem, format!("The app crashed: {m}"))
    } else {
        (LineKind::Output, msg.to_string())
    }
}

/// Read the live log until it ends or Stop is pressed.
fn follow(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, mut child: Child, stop: StopWith, running: &str, parse: fn(&str) -> Option<(LineKind, String)>) -> Res {
    let out = child.stdout.take().ok_or("No log output")?;
    {
        let mut s = st.lock().unwrap();
        if s.cancel {
            let _ = child.kill();
            return Err("Stopped".into());
        }
        s.phase = Phase::Running;
        s.step = running.to_string();
        s.push(LineKind::Step, running);
        s.log_child = Some(child);
        s.stop_with = Some(stop);
    }
    ctx.request_repaint();
    for line in BufReader::new(out).lines() {
        let Ok(line) = line else { break };
        if let Some((k, t)) = parse(&line) {
            say(st, ctx, k, t);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- iPhone

fn compile_iphone(p: &Project, st: &Arc<Mutex<Shared>>, ctx: &egui::Context, trace: bool) -> Result<(String, PathBuf), String> {
    if !cfg!(target_os = "macos") {
        return Err("Building an iOS app needs a Mac with Xcode.".into());
    }
    let sdk = run_tool(Command::new("xcrun").args(["--sdk", "iphonesimulator", "--show-sdk-path"]), "Finding the iOS tools")
        .map_err(|_| "Xcode wasn't found. Install Xcode from the App Store (with the iOS simulator), then try again.".to_string())?;
    let sdk = sdk.trim().lines().last().unwrap_or("").to_string();

    step(st, ctx, if trace { "Writing the Swift app (debug build)…" } else { "Writing the Swift app…" })?;
    let dir = work_dir(p, "iphone");
    let src = dir.join("src");
    crate::native::write_swift_sources(p, &src, trace).map_err(|e| format!("Couldn't write the app: {e}"))?;
    let exec_name = {
        let n: String = p.name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if n.is_empty() { "App".to_string() } else { n }
    };
    let app = dir.join(format!("{exec_name}.app"));
    std::fs::create_dir_all(&app).map_err(|e| format!("Couldn't make the app folder: {e}"))?;

    step(st, ctx, "Compiling with Swift…")?;
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&src).map_err(|e| e.to_string())?.flatten().map(|d| d.path()).filter(|p| p.extension().map_or(false, |e| e == "swift")).collect();
    files.sort();
    run_tool(
        Command::new("xcrun").args(["swiftc", "-sdk", &sdk, "-target", &format!("{arch}-apple-ios17.0-simulator"), "-parse-as-library", "-Onone", "-o"]).arg(app.join(&exec_name)).args(&files),
        "The Swift build",
    )?;
    // the pictures and sounds sit in the app as they are
    if let Ok(rd) = std::fs::read_dir(src.join("Resources")) {
        for f in rd.flatten() {
            std::fs::copy(f.path(), app.join(f.file_name())).map_err(|e| format!("Couldn't add {}: {e}", f.file_name().to_string_lossy()))?;
        }
    }
    for (name, bytes) in crate::icons::ios_bundle_icons(p) {
        std::fs::write(app.join(name), bytes).map_err(|e| format!("Couldn't write the app icon: {e}"))?;
    }
    std::fs::write(app.join("Info.plist"), info_plist(p, &exec_name)).map_err(|e| e.to_string())?;
    run_tool(Command::new("codesign").args(["-s", "-", "--force"]).arg(&app), "Signing the app")?;
    say(st, ctx, LineKind::Output, "Built the app.");
    Ok((exec_name, app))
}

fn build_iphone(p: &Project, st: &Arc<Mutex<Shared>>, ctx: &egui::Context) -> Res {
    let (exec_name, app) = compile_iphone(p, st, ctx, false)?;
    let out_dir = build_out_dir(p);
    let dest = out_dir.join(format!("{exec_name}.app"));
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&dest);
    }
    copy_dir_all(&app, &dest).map_err(|e| format!("Couldn't save app bundle: {e}"))?;
    let msg = format!("Built iOS app → {}", dest.display());
    {
        let mut s = st.lock().unwrap();
        s.built_path = Some(dest);
        s.phase = Phase::Stopped;
        s.step = msg.clone();
        s.push(LineKind::Step, msg);
    }
    ctx.request_repaint();
    Ok(())
}

fn run_iphone(p: &Project, st: &Arc<Mutex<Shared>>, ctx: &egui::Context, debug: bool) -> Res {
    // the simulator first: it starts while the app builds
    let (udid, name, booted) = pick_simulator(hub_selected().as_deref()).ok_or("No iOS simulator was found. In Xcode, open Device Hub (or Window › Devices and Simulators) and add one, then try again.")?;
    if !booted {
        step(st, ctx, &format!("Opening the simulator ({name})…"))?;
        let _ = Command::new("xcrun").args(["simctl", "boot", &udid]).output();
    }
    show_simulator(&udid);

    // build; an app already open on the simulator stays open meanwhile
    let (_, app) = compile_iphone(p, st, ctx, debug)?;

    step(st, ctx, "Waiting for the simulator to finish starting…")?;
    run_tool(Command::new("xcrun").args(["simctl", "bootstatus", &udid, "-b"]), "Starting the simulator")?;

    step(st, ctx, "Installing the app…")?;
    run_tool(Command::new("xcrun").args(["simctl", "install", &udid]).arg(&app), "Installing")?;
    step(st, ctx, "Opening it…")?;
    let child = Command::new("xcrun")
        .args(["simctl", "launch", "--console-pty", "--terminate-running-process", &udid, &p.package])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Couldn't open the app: {e}"))?;
    // bring the simulator's window to the front with the app on it
    show_simulator(&udid);
    follow(st, ctx, child, StopWith::Iphone { udid: udid.clone(), bundle: p.package.clone() }, &format!("Running on the iOS simulator ({name})"), parse_console)
}

/// Device Hub (Xcode 27 and later) replaces the Simulator app.
const DEVICE_HUB: &str = "com.apple.dt.Devices";

/// The device Device Hub shows: it opens on the one picked there last.
fn hub_selected() -> Option<String> {
    let out = Command::new("defaults").args(["export", DEVICE_HUB, "-"]).output().ok()?;
    let xml = String::from_utf8_lossy(&out.stdout);
    let after = xml.split("<key>lastSelectedDeviceIdentifier</key>").nth(1)?;
    let data = after.split("<data>").nth(1)?.split("</data>").next()?;
    // the value is a JSON string: "\"UDID\""
    let raw = base64_decode(data)?;
    let id: String = serde_json::from_slice(&raw).ok()?;
    (!id.is_empty()).then_some(id)
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let bytes: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    let mut out = vec![];
    for chunk in bytes.chunks(4) {
        let v: Vec<u8> = chunk.iter().map(|c| val(*c)).collect::<Option<_>>()?;
        let n = v.iter().enumerate().fold(0u32, |n, (i, x)| n | (*x as u32) << (18 - 6 * i));
        for i in 0..v.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

/// Show the simulator's window on `udid`: the Simulator app on older Xcode,
/// Device Hub on newer (told which device to show when it isn't open yet).
fn show_simulator(udid: &str) {
    let old = Command::new("open").args(["-a", "Simulator", "--args", "-CurrentDeviceUDID", udid]).output();
    if old.is_ok_and(|o| o.status.success()) {
        return;
    }
    let hub_open = Command::new("pgrep").args(["-x", "DeviceHub"]).output().is_ok_and(|o| o.status.success());
    if !hub_open && hub_selected().as_deref() != Some(udid) {
        let json = format!("\"{udid}\"");
        let hex: String = json.bytes().map(|b| format!("{b:02x}")).collect();
        let _ = Command::new("defaults").args(["write", DEVICE_HUB, "lastSelectedDeviceIdentifier", "-data", &hex]).output();
    }
    let _ = Command::new("open").args(["-b", DEVICE_HUB]).output();
}

fn parse_console(line: &str) -> Option<(LineKind, String)> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() {
        return None;
    }
    let tag = format!("[{}] ", crate::codegen::LOG_TAG);
    if let Some(m) = line.strip_prefix(&tag) {
        return Some(classify(m));
    }
    if let Some((id, pid)) = line.split_once(": ") {
        // "com.my.app: 1234" — the process id simctl prints first
        if id.contains('.') && !id.contains(' ') && !pid.is_empty() && pid.trim().chars().all(|c| c.is_ascii_digit()) {
            return Some((LineKind::Output, format!("Started (process {})", pid.trim())));
        }
    }
    let kind = if line.contains("Fatal error") || line.contains("fatal error") { LineKind::Problem } else { LineKind::Output };
    Some((kind, line.to_string()))
}

/// iOS apps can only be built on a Mac (Xcode), so elsewhere Blockwise is for
/// Android only and hides everything iOS. BLOCKWISE_NO_IOS=1 hides it on a
/// Mac too, to see what Windows and Linux show.
pub fn ios_tools() -> bool {
    cfg!(target_os = "macos") && !std::env::var("BLOCKWISE_NO_IOS").is_ok_and(|v| v == "1")
}

fn info_plist(p: &Project, exec_name: &str) -> String {
    let esc = crate::codegen::xml_escape;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>CFBundleIdentifier</key><string>{id}</string>\n<key>CFBundleExecutable</key><string>{exe}</string>\n<key>CFBundleName</key><string>{name}</string>\n<key>CFBundleDisplayName</key><string>{name}</string>\n<key>CFBundlePackageType</key><string>APPL</string>\n<key>CFBundleShortVersionString</key><string>{ver}</string>\n<key>CFBundleVersion</key><string>{code}</string>\n<key>MinimumOSVersion</key><string>17.0</string>\n<key>UIDeviceFamily</key><array><integer>1</integer></array>\n<key>CFBundleSupportedPlatforms</key><array><string>iPhoneSimulator</string></array>\n<key>UILaunchScreen</key><dict/>\n<key>NSCameraUsageDescription</key><string>Takes photos and videos for this app.</string>\n<key>NSMicrophoneUsageDescription</key><string>Records the sound of videos.</string>\n<key>NSAppTransportSecurity</key><dict><key>NSAllowsLocalNetworking</key><true/></dict>\n<key>CFBundleIcons</key><dict><key>CFBundlePrimaryIcon</key><dict><key>CFBundleIconFiles</key><array><string>AppIcon60x60</string></array><key>CFBundleIconName</key><string>AppIcon</string></dict></dict>\n</dict></plist>\n",
        id = esc(&p.package),
        exe = esc(exec_name),
        name = esc(&p.app_name),
        ver = esc(&p.ver_name),
        code = p.ver_code.max(1),
    )
}

/// The iPhone simulator to run on: (id, name, booted). The one Device Hub
/// shows (`prefer`) when it's an iPhone, else a booted one, else the newest.
fn pick_simulator(prefer: Option<&str>) -> Option<(String, String, bool)> {
    let out = Command::new("xcrun").args(["simctl", "list", "devices", "available", "-j"]).output().ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let mut all: Vec<(Vec<u32>, String, String, bool)> = vec![];
    for (runtime, list) in v.get("devices")?.as_object()? {
        if !runtime.contains("iOS") {
            continue;
        }
        let ver = version_key(runtime.rsplit("iOS-").next().unwrap_or(""));
        for d in list.as_array()? {
            let name = d.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            if !name.starts_with("iPhone") {
                continue;
            }
            let udid = d.get("udid").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let booted = d.get("state").and_then(|n| n.as_str()) == Some("Booted");
            all.push((ver.clone(), udid, name, booted));
        }
    }
    if let Some(d) = prefer.and_then(|id| all.iter().find(|d| d.1 == id)) {
        return Some((d.1.clone(), d.2.clone(), d.3));
    }
    if let Some(b) = all.iter().find(|d| d.3) {
        return Some((b.1.clone(), b.2.clone(), true));
    }
    all.sort_by(|a, b| a.0.cmp(&b.0));
    let newest = all.last()?.0.clone();
    // plain models first ("iPhone 17" before "iPhone 17 Pro Max")
    let mut same: Vec<&(Vec<u32>, String, String, bool)> = all.iter().filter(|d| d.0 == newest).collect();
    same.sort_by_key(|d| d.2.len());
    same.first().map(|d| (d.1.clone(), d.2.clone(), false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_debug_log_lines() {
        assert_eq!(parse_logcat("D/Blockwise( 4321): event: When the screen opens"), Some((LineKind::Event, "When the screen opens".into())));
        assert_eq!(parse_logcat("D/Blockwise( 4321): value: count = 2.0"), Some((LineKind::Value, "count = 2.0".into())));
        assert_eq!(parse_logcat("E/AndroidRuntime( 4321): FATAL EXCEPTION: main").map(|l| l.0), Some(LineKind::Problem));
        assert_eq!(parse_console("[Blockwise] block: show message \"hi\""), Some((LineKind::Block, "show message \"hi\"".into())));
        assert_eq!(parse_console("com.my.counter: 5870").map(|l| l.0), Some(LineKind::Output));
    }

    #[test]
    fn debug_builds_log_events_blocks_and_values() {
        let p = crate::app::sample_project(1);
        let k = crate::native::kotlin_with(&p, &p.screens[0], true);
        assert!(k.contains("Log.d(\"Blockwise\", \"event: When the screen opens\")"), "{k}");
        assert!(k.contains("\"value: count = \" + count"), "{k}");
        let s = crate::native::swift_screen_with(&p, &p.screens[0], true);
        assert!(s.contains("print(\"[Blockwise] event: When"), "{s}");
        assert!(s.contains("print(\"[Blockwise] value: count = \\(count)\")"), "{s}");
        // the code shown and saved stays clean
        assert!(!crate::native::kotlin(&p, &p.screens[0]).contains("Log.d(\"Blockwise\""));
    }

    /// Builds and runs the sample for real: BLOCKWISE_DEVICE=android or iphone.
    #[test]
    #[ignore]
    fn run_on_a_real_device() {
        let d = match std::env::var("BLOCKWISE_DEVICE").as_deref() {
            Ok("iphone") => Device::Iphone,
            _ => Device::Android,
        };
        let run = DeviceRun::start(d, crate::app::sample_project(1), egui::Context::default());
        let t0 = Instant::now();
        let mut shown = 0;
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let s = run.state.lock().unwrap();
            for (k, t) in &s.lines[shown..] {
                println!("{k:?}: {t}");
            }
            shown = s.lines.len();
            let done = s.phase == Phase::Failed || s.phase == Phase::Stopped || (s.phase == Phase::Running && s.lines.iter().any(|l| l.0 == LineKind::Value || l.0 == LineKind::Event)) || t0.elapsed() > Duration::from_secs(600);
            if done {
                println!("phase {:?}: {}", s.phase, s.step);
                assert!(s.phase == Phase::Running, "{}", s.step);
                break;
            }
        }
        std::thread::sleep(Duration::from_secs(2));
        for (k, t) in &run.state.lock().unwrap().lines[shown..] {
            println!("{k:?}: {t}");
        }
        run.stop();
    }
}

#[cfg(test)]
mod iphone_e2e {
    use super::*;
    use crate::model::*;

    fn blk(op: &str, args: Vec<Arg>) -> Block {
        Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] }
    }
    fn t(s: &str) -> Arg {
        Arg::Text(s.into())
    }

    /// Main screen: a toast when it opens, then after 2.5 s the second screen.
    /// A picture with round corners and a shadow, and a sound that plays
    /// when the screen opens. BLOCKWISE_MEDIA=<folder with sunset.png, ding.wav>.
    pub fn media_project() -> Project {
        let dir = std::path::PathBuf::from(std::env::var("BLOCKWISE_MEDIA").expect("set BLOCKWISE_MEDIA"));
        let mut p = Project::new(990_002, "Media", "Media", "com.my.media");
        let _ = std::fs::remove_dir_all(p.files_dir());
        let pic = p.add_file(&dir.join("sunset.png")).unwrap();
        let snd = p.add_file(&dir.join("ding.wav")).unwrap();
        let s = &mut p.screens[0];
        let mut col = Widget::new("col".into(), WKind::LinearV, "root".into());
        col.width = Size::Match;
        col.padding = 16;
        s.widgets.push(col);
        let mut img = Widget::new("photo".into(), WKind::ImageView, "col".into());
        img.image = pic;
        img.width = Size::Match;
        img.radius = 20;
        img.elevation = 6;
        img.bg = 0xFFFF_FFFF;
        s.widgets.push(img);
        let mut b = Widget::new("play".into(), WKind::Button, "col".into());
        b.text = "Play again".into();
        b.radius = 22;
        b.elevation = 4;
        b.bg = 0xFF5B_5BD6;
        b.text_color = 0xFFFF_FFFF;
        b.margin = 12;
        s.widgets.push(b);
        let mut t = Widget::new("label".into(), WKind::TextView, "col".into());
        t.border = 2;
        t.border_color = 0xFF5B_5BD6;
        t.radius = 8;
        t.text_size = 16.0;
        s.widgets.push(t);
        s.components.push(Component { name: "player".into(), kind: CompKind::Sound });
        s.vars.push(Var { name: "playing".into(), ty: VarType::Bool });
        let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let t = |x: &str| Arg::Text(x.into());
        let play = vec![
            blk("soundPlay", vec![t("player"), t(&snd)]),
            blk("setVarBoolean", vec![t("playing"), Arg::Block(Box::new(blk("soundIsPlaying", vec![t("player")])))]),
            blk("setText", vec![t("label"), Arg::Block(Box::new(blk("stringJoin", vec![t("Playing: "), Arg::Block(Box::new(blk("toStringFixed", vec![Arg::Block(Box::new(blk("soundDuration", vec![t("player")]))), t("0")])))])))]),
        ];
        s.events[0].blocks = play.clone();
        s.events.push(Event { target: "play".into(), name: "onClick".into(), blocks: play, loose: vec![], params: vec![] });
        p
    }

    /// A font and a data file: a title in the font, the file's text below,
    /// and a button that changes the font back and forth. BLOCKWISE_MEDIA
    /// holds a .ttf and a .json.
    pub fn assets_project() -> Project {
        let dir = std::path::PathBuf::from(std::env::var("BLOCKWISE_MEDIA").expect("set BLOCKWISE_MEDIA"));
        let mut p = Project::new(990_005, "Assets", "Assets", "com.my.assets");
        let _ = std::fs::remove_dir_all(p.files_dir());
        let mut font = String::new();
        let mut data = String::new();
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            if let Ok(n) = p.add_file(&e.path()) {
                match p.file(&n).map(|f| f.kind) {
                    Some(FileKind::Font) => font = n,
                    Some(FileKind::Data) => data = n,
                    _ => {}
                }
            }
        }
        let s = &mut p.screens[0];
        let mut col = Widget::new("col".into(), WKind::LinearV, "root".into());
        col.width = Size::Match;
        col.padding = 16;
        s.widgets.push(col);
        let mut title = Widget::new("title".into(), WKind::TextView, "col".into());
        title.text = "Hello fonts".into();
        title.text_size = 34.0;
        title.font = font.clone();
        s.widgets.push(title);
        let mut body = Widget::new("body".into(), WKind::TextView, "col".into());
        body.text_size = 15.0;
        s.widgets.push(body);
        let mut b = Widget::new("swap".into(), WKind::Button, "col".into());
        b.text = "Swap font".into();
        b.font = font.clone();
        s.widgets.push(b);
        let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let t = |x: &str| Arg::Text(x.into());
        s.events[0].blocks = vec![
            blk("setText", vec![t("body"), Arg::Block(Box::new(blk("assetText", vec![t(&data)])))]),
            blk("setFont", vec![t("body"), t(&font)]),
        ];
        s.events.push(Event { target: "swap".into(), name: "onClick".into(), blocks: vec![blk("setFont", vec![t("title"), t("")])], loose: vec![], params: vec![] });
        p
    }

    /// A video that starts by itself; after 1 s its length and state show,
    /// it jumps near the end, and its end shows "ended". BLOCKWISE_MEDIA has clip.mp4.
    pub fn video_project() -> Project {
        let dir = std::path::PathBuf::from(std::env::var("BLOCKWISE_MEDIA").expect("set BLOCKWISE_MEDIA"));
        let mut p = Project::new(990_007, "Video", "Video", "com.my.video");
        let _ = std::fs::remove_dir_all(p.files_dir());
        let clip = p.add_file(&dir.join(if dir.join("clip.mp4").exists() { "clip.mp4" } else { "clip.mov" })).unwrap();
        let s = &mut p.screens[0];
        let mut col = Widget::new("col".into(), WKind::LinearV, "root".into());
        col.width = Size::Match;
        col.padding = 16;
        s.widgets.push(col);
        let mut v = Widget::new("movie".into(), WKind::VideoView, "col".into());
        v.video = clip;
        v.autoplay = true;
        s.widgets.push(v);
        let mut info = Widget::new("info".into(), WKind::TextView, "col".into());
        info.text = "waiting".into();
        info.text_size = 18.0;
        info.bold = true;
        s.widgets.push(info);
        let mut end = Widget::new("end".into(), WKind::TextView, "col".into());
        end.text = "not ended".into();
        end.text_size = 18.0;
        s.widgets.push(end);
        s.components.push(Component { name: "t".into(), kind: CompKind::Timer });
        let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let t = |x: &str| Arg::Text(x.into());
        let r = |b: Block| Arg::Block(Box::new(b));
        let join = |a: Arg, b: Arg| r(blk("stringJoin", vec![a, b]));
        let mut later = blk("timerAfter", vec![t("t"), t("1000")]);
        let mut state = blk("ifElse", vec![r(blk("videoIsPlaying", vec![t("movie")]))]);
        state.sub1 = vec![blk("setText", vec![t("end"), t("playing")])];
        state.sub2 = vec![blk("setText", vec![t("end"), t("not playing")])];
        later.sub1 = vec![
            blk("setText", vec![t("info"), join(t("length "), r(blk("toStringFixed", vec![r(blk("videoDuration", vec![t("movie")])), t("0")])))]),
            state,
            blk("videoSeek", vec![t("movie"), t("4500")]),
        ];
        s.events[0].blocks = vec![later];
        s.events.push(Event { target: "movie".into(), name: "onCompletion".into(), blocks: vec![blk("setText", vec![t("end"), t("ended")])], loose: vec![], params: vec![] });
        p
    }

    /// The camera, pictures, videos and sounds from the phone, and a picture
    /// from a web address (BLOCKWISE_PICTURE_URL).
    /// A button whose tap reads past the end of an empty list: the app
    /// crashes after it is up, and reopens on the crash screen.
    pub fn tap_crash_project() -> Project {
        let mut p = Project::new(990_010, "Tap crash", "Tap crash", "com.my.tapcrash");
        let s = &mut p.screens[0];
        let mut b = Widget::new("crash".into(), WKind::Button, "root".into());
        b.text = "Crash".into();
        s.widgets.push(b);
        s.lists.push(ListVar { name: "names".into(), ty: ListType::Str });
        let get = Block { id: new_id(), op: "getAtListStr".into(), args: vec![Arg::Text("2".into()), Arg::Text("names".into())], sub1: vec![], sub2: vec![] };
        let set = Block { id: new_id(), op: "doToast".into(), args: vec![Arg::Block(Box::new(get))], sub1: vec![], sub2: vec![] };
        s.events.push(Event { target: "crash".into(), name: "onClick".into(), blocks: vec![set], loose: vec![], params: vec![] });
        p
    }

    /// Text fields of each kind: a password with its eye, an email, a number,
    /// a phone number and several lines.
    pub fn inputs_project() -> Project {
        let mut p = Project::new(990_009, "Inputs", "Inputs", "com.my.inputs");
        let s = &mut p.screens[0];
        let mut col = Widget::new("col".into(), WKind::LinearV, "root".into());
        col.width = Size::Match;
        col.padding = 16;
        s.widgets.push(col);
        for (id, k, hint, text) in [
            ("pw", InputKind::Password, "Password", "secret42"),
            ("mail", InputKind::Email, "Email", "me@mail.com"),
            ("age", InputKind::Number, "Age", "27"),
            ("tel", InputKind::Phone, "Phone", "+212 600"),
            ("note", InputKind::Multiline, "A note", ""),
        ] {
            let mut w = Widget::new(id.into(), WKind::EditText, "col".into());
            w.input = k;
            w.password = k == InputKind::Password;
            w.hint = hint.into();
            w.text = text.into();
            w.width = Size::Match;
            w.text_size = 16.0;
            s.widgets.push(w);
        }
        p
    }

    pub fn camera_project() -> Project {
        let mut p = Project::new(990_008, "Camera", "Camera", "com.my.camera");
        let s = &mut p.screens[0];
        let mut col = Widget::new("col".into(), WKind::LinearV, "root".into());
        col.width = Size::Match;
        col.padding = 12;
        s.widgets.push(col);
        for (row, buttons) in [("row1", [("photo", "Photo"), ("record", "Video"), ("web", "Web")]), ("row2", [("pickpic", "Pick pic"), ("pickvid", "Pick video"), ("picksnd", "Pick sound")])] {
            let mut r = Widget::new(row.into(), WKind::LinearH, "col".into());
            r.width = Size::Match;
            s.widgets.push(r);
            for (id, label) in buttons {
                let mut b = Widget::new(id.into(), WKind::Button, row.into());
                b.text = label.into();
                b.weight = 1.0;
                b.width = Size::Px(0);
                s.widgets.push(b);
            }
        }
        let mut info = Widget::new("info".into(), WKind::TextView, "col".into());
        info.text = "ready".into();
        info.text_size = 14.0;
        s.widgets.push(info);
        let mut pic = Widget::new("pic".into(), WKind::ImageView, "col".into());
        pic.width = Size::Match;
        pic.height = Size::Px(220);
        s.widgets.push(pic);
        let mut movie = Widget::new("movie".into(), WKind::VideoView, "col".into());
        movie.height = Size::Px(200);
        s.widgets.push(movie);
        s.components.push(Component { name: "cam".into(), kind: CompKind::Camera });
        s.components.push(Component { name: "player".into(), kind: CompKind::Sound });
        let blk = |op: &str, args: Vec<Arg>| Block { id: crate::model::new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
        let t = |x: &str| Arg::Text(x.into());
        let path = || Arg::Block(Box::new(blk("mediaPath", vec![])));
        let said = |what: &str| blk("setText", vec![t("info"), Arg::Block(Box::new(blk("stringJoin", vec![t(what), path()])))]);
        let cancelled = |what: &str| vec![blk("setText", vec![t("info"), t(&format!("{what} cancelled"))])];
        let answer = |op: &str, args: Vec<Arg>, what: &str, then: Vec<Block>| {
            let mut b = blk(op, args);
            b.sub1 = std::iter::once(said(&format!("{what}: "))).chain(then).collect();
            b.sub2 = cancelled(what);
            b
        };
        let show_pic = || blk("setImageFrom", vec![t("pic"), path()]);
        let play_vid = || vec![blk("setVideoFrom", vec![t("movie"), path()]), blk("videoPlay", vec![t("movie")])];
        let url = std::env::var("BLOCKWISE_PICTURE_URL").unwrap_or_else(|_| "https://picsum.photos/id/10/600/400".into());
        let on = |id: &str, blocks: Vec<Block>| Event { target: id.into(), name: "onClick".into(), blocks, loose: vec![], params: vec![] };
        s.events.push(on("photo", vec![answer("cameraPhoto", vec![t("cam")], "photo", vec![show_pic()])]));
        s.events.push(on("record", vec![answer("cameraVideo", vec![t("cam")], "video", play_vid())]));
        s.events.push(on("web", vec![blk("setImageFrom", vec![t("pic"), t(&url)]), blk("setText", vec![t("info"), t(&url)])]));
        s.events.push(on("pickpic", vec![answer("pickMedia", vec![t("picture")], "picture", vec![show_pic()])]));
        s.events.push(on("pickvid", vec![answer("pickMedia", vec![t("video")], "picked video", play_vid())]));
        s.events.push(on("picksnd", vec![answer("pickMedia", vec![t("sound")], "sound", vec![blk("soundPlayFrom", vec![t("player"), path()])])]));
        p
    }

    pub fn nav_project() -> Project {
        let mut p = Project::new(1, "NavTest", "Nav Test", "com.blockwise.navtest");
        let mut second = Screen::new("second");
        let mut label = Widget::new("textview1".into(), WKind::TextView, "root".into());
        label.text = "Second screen".into();
        label.text_size = 28.0;
        second.widgets.push(label);
        let target = second.activity_name();
        p.screens.push(second);
        let main = &mut p.screens[0];
        main.components.push(Component { name: "i".into(), kind: CompKind::Intent });
        main.components.push(Component { name: "t".into(), kind: CompKind::Timer });
        let mut later = blk("timerAfter", vec![t("t"), t("2500")]);
        later.sub1 = vec![blk("openScreen", vec![t(&target)])];
        main.events[0].blocks = vec![blk("doToast", vec![t("Hello toast")]), later];
        p
    }

    /// A real web request when the screen opens: the answer's code and one
    /// JSON field on screen, a toast once it is in.
    pub fn web_project() -> Project {
        let mut p = Project::new(1, "WebTest", "Web Test", "com.blockwise.webtest");
        let main = &mut p.screens[0];
        let mut col = Widget::new("linear1".into(), WKind::LinearV, "root".into());
        col.width = Size::Match;
        col.padding = 24;
        main.widgets.push(col);
        for (id, text, size) in [("status", "Loading…", 22.0), ("title", "", 18.0)] {
            let mut w = Widget::new(id.into(), WKind::TextView, "linear1".into());
            w.text = text.into();
            w.text_size = size;
            main.widgets.push(w);
        }
        main.vars.push(Var { name: "todo".into(), ty: VarType::Map });
        main.components.push(Component { name: "api".into(), kind: CompKind::Web });
        let rep = |op: &str, args: Vec<Arg>| Arg::Block(Box::new(blk(op, args)));
        let join = |a: Arg, b: Arg| rep("stringJoin", vec![a, b]);
        let mut req = blk("webRequest", vec![t("api"), t("GET"), t("https://jsonplaceholder.typicode.com/todos/1"), t("")]);
        req.sub1 = vec![
            blk("mapFromJson", vec![t("todo"), rep("webResponse", vec![])]),
            blk("setText", vec![t("status"), join(t("Code "), rep("toString", vec![rep("webStatus", vec![])]))]),
            blk("setText", vec![t("title"), join(t("Title: "), rep("mapGet", vec![t("todo"), t("title")]))]),
            blk("doToast", vec![t("Loaded from the web")]),
        ];
        req.sub2 = vec![blk("setText", vec![t("status"), join(t("Failed: "), rep("webResponse", vec![]))])];
        main.events[0].blocks = vec![blk("webSetHeader", vec![t("api"), t("Accept"), t("application/json")]), req];
        p
    }

    /// The repeating toast from a person's project: every second from 0 ms.
    fn toast_project() -> Project {
        let mut p = Project::new(1, "ToastTest", "Toast Test", "com.blockwise.toasttest");
        let main = &mut p.screens[0];
        main.components.push(Component { name: "t".into(), kind: CompKind::Timer });
        let mut every = blk("timerEvery", vec![t("t"), t("0"), t("1000")]);
        every.sub1 = vec![blk("doToast", vec![t("Hello world")])];
        main.events[0].blocks = vec![every];
        p
    }

    #[test]
    fn nav_project_swift() {
        let p = nav_project();
        println!("{}", crate::native::swift_screen_with(&p, &p.screens[0], false));
        println!("{}", crate::native::swift_app(&p));
    }

    /// Runs it on the simulator and saves two screenshots: the toast (1 s)
    /// and the second screen (5 s). BLOCKWISE_SHOTS=<folder>.
    #[test]
    #[ignore]
    fn iphone_toast_and_navigation() {
        let dir = std::path::PathBuf::from(std::env::var("BLOCKWISE_SHOTS").unwrap_or_else(|_| std::env::temp_dir().display().to_string()));
        let project = match std::env::var("BLOCKWISE_E2E").ok() {
            Some(v) if v == "toast" => toast_project(),
            Some(v) if v == "web" => web_project(),
            Some(v) if v == "all" => crate::runtime::tests::every_block_project(),
            Some(v) if v == "media" => media_project(),
            Some(v) if v == "assets" => assets_project(),
            Some(v) if v == "video" => video_project(),
            Some(v) if v == "camera" => camera_project(),
            Some(v) if v == "inputs" => inputs_project(),
            Some(v) if v == "tap-crash" => tap_crash_project(),
            // a saved project, by app name
            Some(name) if !name.is_empty() => {
                // a real project: its assets are where Blockwise keeps them
                std::env::set_var("BLOCKWISE_FILES_ROOT", crate::model::real_files_root());
                crate::model::Store::load().projects.into_iter().find(|p| p.app_name == name).expect("no project with that name")
            }
            _ => nav_project(),
        };
        let android = std::env::var("BLOCKWISE_DEVICE").as_deref() == Ok("android");
        // BLOCKWISE_SETUP=1: agree to download missing Android tools
        if std::env::var("BLOCKWISE_SETUP").as_deref() == Ok("1") {
            crate::android_setup::allow();
        }
        let wait: u64 = std::env::var("BLOCKWISE_E2E_WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
        // BLOCKWISE_E2E_PLAIN=1: a plain Run instead of a debug run
        let debug = std::env::var("BLOCKWISE_E2E_PLAIN").as_deref() != Ok("1");
        let run = DeviceRun::run(if android { Device::Android } else { Device::Iphone }, project, egui::Context::default(), debug);
        let t0 = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(300));
            let s = run.state.lock().unwrap();
            if s.phase == Phase::Running || s.phase == Phase::Failed || t0.elapsed() > Duration::from_secs(wait) {
                for (k, l) in &s.lines {
                    println!("{k:?}: {l}");
                }
                assert!(s.phase == Phase::Running, "{}", s.step);
                break;
            }
        }
        for (wait, name) in [(1200, "toast.png"), (3300, "second.png")] {
            std::thread::sleep(Duration::from_millis(wait));
            if android {
                let out = Command::new(android_sdk().unwrap().join("platform-tools/adb")).args(["exec-out", "screencap", "-p"]).output().unwrap();
                std::fs::write(dir.join(name), out.stdout).unwrap();
            } else {
                let (udid, _, _) = pick_simulator(hub_selected().as_deref()).unwrap();
                let out = Command::new("xcrun").args(["simctl", "io", &udid, "screenshot"]).arg(dir.join(name)).output().unwrap();
                println!("{name}: {}", String::from_utf8_lossy(&out.stderr).trim());
            }
        }
        std::thread::sleep(Duration::from_secs(1));
        for (k, l) in &run.state.lock().unwrap().lines {
            println!("{k:?}: {l}");
        }
    }
}
