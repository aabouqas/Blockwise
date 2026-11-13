//! First-time Android setup. A computer without Android Studio has none of
//! the tools an Android build needs, so Blockwise can fetch them itself: a
//! Java, the Android SDK (command-line tools, platform tools, an SDK platform
//! and build tools), the emulator with a phone system image, and Gradle. Java
//! and Gradle go into Blockwise's own folder; the SDK goes where Android
//! Studio keeps it (so Studio finds it too, and its path has no spaces, which
//! the Android tools dislike). An SDK already on the computer gets only the
//! missing parts.
//!
//! Nothing is downloaded until the person agrees: a run that finds tools
//! missing stops and the log shows what would be downloaded, from where and
//! about how big, with the Android SDK license. Pressing "Download and set
//! up" allows it for the rest of the session and runs again.

use crate::device::{progress, say, step, LineKind, Shared};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Res = Result<(), String>;

/// The person agreed to download the Android tools (for this session).
static ALLOWED: AtomicBool = AtomicBool::new(false);

pub fn allow() {
    ALLOWED.store(true, Ordering::Relaxed);
}

pub const LICENSE_URL: &str = "https://developer.android.com/studio/terms";

/// The emulator Blockwise makes when there is none.
pub const AVD: &str = "Blockwise_Phone";
const API: u32 = 36;
const GRADLE: &str = "9.1.0";
const JAVA: u32 = 21;

/// Where the downloaded tools live.
pub fn root() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Blockwise").join("android")
}

/// Where a new SDK goes: Android Studio's own default place.
pub fn managed_sdk() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    if cfg!(target_os = "macos") {
        home.join("Library/Android/sdk")
    } else if cfg!(windows) {
        dirs::data_local_dir().unwrap_or(home).join("Android/Sdk")
    } else {
        home.join("Android/Sdk")
    }
}

/// Where emulators (AVDs) are kept. Given to avdmanager and the emulator
/// alike: Java finds the home folder its own way, so without this the two
/// can look in different places.
pub fn avd_home() -> PathBuf {
    if let Some(v) = std::env::var_os("ANDROID_AVD_HOME") {
        return PathBuf::from(v);
    }
    if let Some(v) = std::env::var_os("ANDROID_USER_HOME") {
        return PathBuf::from(v).join("avd");
    }
    dirs::home_dir().unwrap_or_else(std::env::temp_dir).join(".android").join("avd")
}

/// The downloaded Java's home folder.
pub fn managed_java() -> Option<PathBuf> {
    let rd = std::fs::read_dir(root().join("jdk")).ok()?;
    rd.flatten().map(|d| d.path()).flat_map(|p| [p.join("Contents/Home"), p]).find(|h| h.join("bin").join(exe("java")).is_file())
}

/// The downloaded Gradle's program.
pub fn managed_gradle() -> Option<PathBuf> {
    let p = root().join("gradle").join(format!("gradle-{GRADLE}")).join("bin").join(if cfg!(windows) { "gradle.bat" } else { "gradle" });
    p.is_file().then_some(p)
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn script(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.bat")
    } else {
        name.to_string()
    }
}

fn arm() -> bool {
    cfg!(target_arch = "aarch64")
}

fn image() -> String {
    format!("system-images;android-{API};google_apis;{}", if arm() { "arm64-v8a" } else { "x86_64" })
}

/// One thing to download.
#[derive(Clone, Debug, PartialEq)]
pub struct Need {
    pub what: String,
    pub from: &'static str,
    /// About how big the download is, in MB.
    pub mb: u32,
    part: Part,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Java,
    CmdlineTools,
    Packages,
    Emulator,
    Image,
    Gradle,
}

/// What the computer has already.
#[derive(Default, Clone)]
pub struct Have {
    pub java: bool,
    pub sdk: Option<PathBuf>,
    pub sdkmanager: bool,
    /// An SDK platform from 34 up and build tools.
    pub packages: bool,
    pub emulator: bool,
    pub image: bool,
    pub avd: bool,
    pub gradle: bool,
}

impl Have {
    pub fn look() -> Have {
        let sdk = crate::device::android_sdk();
        let mut h = Have { java: crate::device::java_home().is_some(), gradle: crate::device::gradle().is_some(), ..Default::default() };
        if let Some(sdk) = &sdk {
            h.sdkmanager = sdkmanager_in(sdk).is_some();
            let platform = std::fs::read_dir(sdk.join("platforms")).map(|rd| rd.flatten().any(|d| d.file_name().to_string_lossy().strip_prefix("android-").and_then(|v| v.parse::<u32>().ok()).is_some_and(|v| (34..=API).contains(&v)))).unwrap_or(false);
            let tools = std::fs::read_dir(sdk.join("build-tools")).map(|rd| rd.flatten().any(|d| d.path().join(exe("aapt2")).is_file())).unwrap_or(false);
            h.packages = platform && tools;
            let emulator = sdk.join("emulator").join(exe("emulator"));
            h.emulator = emulator.is_file();
            h.image = sdk.join(image().replace(';', "/")).join("system.img").is_file();
            h.avd = h.emulator && crate::device::pick_avd(&emulator).is_some();
        }
        h.sdk = sdk;
        h
    }
}

/// What has to be downloaded before a build (and, for `run`, an emulator run).
pub fn needs(h: &Have, run: bool) -> Vec<Need> {
    let mut n = vec![];
    let mut add = |what: String, from, mb, part| n.push(Need { what, from, mb, part });
    if !h.java {
        add(format!("Java {JAVA} (Eclipse Temurin), to run the Android tools"), "adoptium.net", 200, Part::Java);
    }
    let packages = !h.packages || h.sdk.is_none();
    let avd = run && !h.avd;
    let emulator = run && !h.emulator;
    let image = avd && !h.image;
    if !h.sdkmanager && (packages || emulator || image || avd) {
        add("Android SDK command-line tools".into(), "dl.google.com", 160, Part::CmdlineTools);
    }
    if packages {
        add(format!("Android platform tools, SDK {API} and build tools"), "dl.google.com", 180, Part::Packages);
    }
    if emulator {
        add("The Android emulator".into(), "dl.google.com", 330, Part::Emulator);
    }
    if image {
        add(format!("A phone system image (Android {})", API - 20), "dl.google.com", 1600, Part::Image);
    }
    if !h.gradle {
        add(format!("Gradle {GRADLE}, the Android build tool"), "services.gradle.org", 140, Part::Gradle);
    }
    n
}

/// "1.9 GB", "340 MB".
pub fn size(mb: u32) -> String {
    if mb >= 1000 {
        format!("{:.1} GB", mb as f64 / 1000.0)
    } else {
        format!("{mb} MB")
    }
}

/// Makes sure the tools are there: nothing to do, or downloads them if the
/// person agreed, or stops the run with the list for them to agree to.
pub fn ensure(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, run: bool) -> Res {
    let have = Have::look();
    let need = needs(&have, run);
    let avd_only = run && !have.avd && need.is_empty();
    if need.is_empty() && !avd_only {
        return Ok(());
    }
    if !ALLOWED.load(Ordering::Relaxed) {
        let total: u32 = need.iter().map(|n| n.mb).sum();
        st.lock().unwrap().setup = Some(need);
        return Err(if total > 0 {
            format!("Android isn't set up on this computer yet. Blockwise can download what it needs (about {}).", size(total))
        } else {
            "There is no Android emulator yet. Blockwise can make one.".into()
        });
    }
    install(st, ctx, &have, run)
}

fn install(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, have: &Have, run: bool) -> Res {
    let need = needs(have, run);
    let has = |p: Part| need.iter().any(|n| n.part == p);
    let dir = root();
    let tmp = dir.join("downloads");
    std::fs::create_dir_all(&tmp).map_err(|e| format!("Couldn't make {}: {e}", tmp.display()))?;
    say(st, ctx, LineKind::Output, format!("Setting up Android in {}", dir.display()));

    if has(Part::Java) {
        step(st, ctx, &format!("Downloading Java {JAVA}…"))?;
        let os = if cfg!(target_os = "macos") { "mac" } else if cfg!(windows) { "windows" } else { "linux" };
        let url = format!("https://api.adoptium.net/v3/binary/latest/{JAVA}/ga/{os}/{}/jdk/hotspot/normal/eclipse", if arm() { "aarch64" } else { "x64" });
        let file = tmp.join(if cfg!(windows) { "jdk.zip" } else { "jdk.tar.gz" });
        download(st, ctx, &url, &file, &format!("Java {JAVA}"))?;
        step(st, ctx, "Unpacking Java…")?;
        let out = dir.join("jdk");
        let _ = std::fs::remove_dir_all(&out);
        unpack(&file, &out)?;
        let _ = std::fs::remove_file(&file);
        managed_java().ok_or("Java was downloaded but its program wasn't found in it.")?;
    }
    let java = crate::device::java_home().ok_or("No Java to run the Android tools.")?;

    let sdk = have.sdk.clone().unwrap_or_else(managed_sdk);
    if has(Part::CmdlineTools) {
        step(st, ctx, "Finding the latest Android command-line tools…")?;
        let os = if cfg!(target_os = "macos") { "mac" } else if cfg!(windows) { "win" } else { "linux" };
        let index = Command::new("curl").args(["-fsSL", "--max-time", "60", "https://dl.google.com/android/repository/repository2-3.xml"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        let name = latest_cmdline_tools(&index, os).unwrap_or_else(|| format!("commandlinetools-{os}-13114758_latest.zip"));
        let file = tmp.join(&name);
        download(st, ctx, &format!("https://dl.google.com/android/repository/{name}"), &file, "the command-line tools")?;
        step(st, ctx, "Unpacking the command-line tools…")?;
        let unpacked = tmp.join("cmdline-tools-unpacked");
        let _ = std::fs::remove_dir_all(&unpacked);
        unpack(&file, &unpacked)?;
        let latest = sdk.join("cmdline-tools").join("latest");
        let _ = std::fs::remove_dir_all(&latest);
        std::fs::create_dir_all(sdk.join("cmdline-tools")).map_err(|e| e.to_string())?;
        std::fs::rename(unpacked.join("cmdline-tools"), &latest).map_err(|e| format!("Couldn't put the command-line tools in place: {e}"))?;
        let _ = std::fs::remove_dir_all(&unpacked);
        let _ = std::fs::remove_file(&file);
    }

    let mut packages: Vec<String> = vec![];
    if has(Part::Packages) {
        packages.extend(["platform-tools".to_string(), format!("platforms;android-{API}"), format!("build-tools;{API}.0.0")]);
    }
    if has(Part::Emulator) {
        packages.push("emulator".into());
    }
    if has(Part::Image) {
        packages.push(image());
    }
    let make_avd = run && !have.avd;
    if !packages.is_empty() || make_avd {
        let sm = sdkmanager_in(&sdk).ok_or("The Android command-line tools weren't found in the SDK.")?;
        if !packages.is_empty() {
            step(st, ctx, "Accepting the Android SDK license…")?;
            tool(st, ctx, Command::new(&sm).arg(format!("--sdk_root={}", sdk.display())).arg("--licenses").env("JAVA_HOME", &java), &"y\n".repeat(40), "Accepting the licenses")?;
            step(st, ctx, "Downloading the Android SDK parts (this takes a while)…")?;
            tool(st, ctx, Command::new(&sm).arg(format!("--sdk_root={}", sdk.display())).args(&packages).env("JAVA_HOME", &java), &"y\n".repeat(10), "Installing the Android SDK parts")?;
        }
        if make_avd {
            step(st, ctx, "Making an Android emulator…")?;
            let am = sm.with_file_name(script("avdmanager"));
            let img = if sdk.join(image().replace(';', "/")).join("system.img").is_file() { image() } else { any_image(&sdk).ok_or("There is no phone system image to make an emulator with.")? };
            let _ = std::fs::create_dir_all(avd_home());
            tool(st, ctx, Command::new(&am).args(["create", "avd", "--force", "-n", AVD, "-k", &img, "-d", "pixel_7"]).env("JAVA_HOME", &java).env("ANDROID_SDK_ROOT", &sdk).env("ANDROID_AVD_HOME", avd_home()), "no\n", "Making the emulator")?;
            tune_avd();
            say(st, ctx, LineKind::Output, format!("Made the emulator {}.", AVD.replace('_', " ")));
        }
    }

    if has(Part::Gradle) {
        step(st, ctx, &format!("Downloading Gradle {GRADLE}…"))?;
        let file = tmp.join(format!("gradle-{GRADLE}-bin.zip"));
        download(st, ctx, &format!("https://services.gradle.org/distributions/gradle-{GRADLE}-bin.zip"), &file, &format!("Gradle {GRADLE}"))?;
        step(st, ctx, "Unpacking Gradle…")?;
        let out = dir.join("gradle");
        let _ = std::fs::remove_dir_all(&out);
        unpack(&file, &out)?;
        let _ = std::fs::remove_file(&file);
        let g = managed_gradle().ok_or("Gradle was downloaded but its program wasn't found in it.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o755));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    say(st, ctx, LineKind::Output, "Android is set up.");
    Ok(())
}

/// The newest "commandlinetools-<os>-<n>_latest.zip" in Google's index.
fn latest_cmdline_tools(index: &str, os: &str) -> Option<String> {
    let pre = format!("commandlinetools-{os}-");
    index.match_indices(&pre).filter_map(|(i, _)| {
        let rest = &index[i + pre.len()..];
        let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        (rest[n.len()..].starts_with("_latest.zip") && !n.is_empty()).then(|| n.parse::<u64>().ok()).flatten()
    }).max().map(|n| format!("{pre}{n}_latest.zip"))
}

/// sdkmanager in an SDK: cmdline-tools/latest first, then any version.
fn sdkmanager_in(sdk: &Path) -> Option<PathBuf> {
    let name = script("sdkmanager");
    let latest = sdk.join("cmdline-tools/latest/bin").join(&name);
    if latest.is_file() {
        return Some(latest);
    }
    let mut all: Vec<PathBuf> = std::fs::read_dir(sdk.join("cmdline-tools")).ok()?.flatten().map(|d| d.path().join("bin").join(&name)).filter(|p| p.is_file()).collect();
    all.sort();
    all.pop()
}

/// A phone system image already in the SDK, newest first.
fn any_image(sdk: &Path) -> Option<String> {
    let mut found = vec![];
    for api in std::fs::read_dir(sdk.join("system-images")).ok()?.flatten() {
        for tag in std::fs::read_dir(api.path()).into_iter().flatten().flatten() {
            for abi in std::fs::read_dir(tag.path()).into_iter().flatten().flatten() {
                if abi.path().join("system.img").is_file() {
                    let n = |d: &std::fs::DirEntry| d.file_name().to_string_lossy().to_string();
                    let (a, t, b) = (n(&api), n(&tag), n(&abi));
                    if !["tv", "wear", "automotive", "desktop"].iter().any(|w| t.contains(w)) {
                        found.push((a.trim_start_matches("android-").parse::<u32>().unwrap_or(0), format!("system-images;{a};{t};{b}")));
                    }
                }
            }
        }
    }
    found.sort();
    found.pop().map(|f| f.1)
}

/// A keyboard and the graphics card for the new emulator.
fn tune_avd() {
    let ini = avd_home().join(format!("{AVD}.avd/config.ini"));
    let Ok(text) = std::fs::read_to_string(&ini) else { return };
    let mut lines: Vec<String> = text.lines().filter(|l| !l.starts_with("hw.keyboard") && !l.starts_with("hw.gpu.")).map(String::from).collect();
    lines.extend(["hw.keyboard=yes".into(), "hw.gpu.enabled=yes".into(), "hw.gpu.mode=auto".into()]);
    let _ = std::fs::write(&ini, lines.join("\n") + "\n");
}

/// Downloads a file with curl, showing how far it got.
fn download(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, url: &str, to: &Path, what: &str) -> Res {
    let total = Command::new("curl").args(["-sIL", "--max-time", "30", url]).output().ok().and_then(|o| {
        String::from_utf8_lossy(&o.stdout).lines().filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("content-length").then(|| v.trim().parse::<u64>().ok()).flatten()
        }).last()
    });
    let _ = std::fs::remove_file(to);
    let mut child = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--connect-timeout", "30", "-o"])
        .arg(to)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't start curl to download {what}: {e}"))?;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if status.success() {
                return Ok(());
            }
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return Err(format!("Couldn't download {what} from {url}: {}", err.trim()));
        }
        if crate::device::cancelled(st) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Stopped".into());
        }
        let got = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
        let mb = |b: u64| (b / 1_000_000) as u32;
        let text = match total {
            Some(t) if t > 0 => format!("Downloading {what}…  {} of {} ({}%)", size(mb(got)), size(mb(t)), got * 100 / t),
            _ => format!("Downloading {what}…  {}", size(mb(got))),
        };
        progress(st, ctx, &text);
        std::thread::sleep(Duration::from_millis(400));
    }
}

/// Unpacks a .zip or .tar.gz into a folder.
fn unpack(file: &Path, into: &Path) -> Res {
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    let name = file.to_string_lossy();
    let mut c = if name.ends_with(".zip") && !cfg!(windows) {
        let mut c = Command::new("unzip");
        c.arg("-q").arg("-o").arg(file).arg("-d").arg(into);
        c
    } else {
        let mut c = Command::new("tar");
        c.arg(if name.ends_with(".zip") { "-xf" } else { "-xzf" }).arg(file).arg("-C").arg(into);
        c
    };
    let out = c.output().map_err(|e| format!("Couldn't unpack {name}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("Couldn't unpack {name}: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Runs an SDK tool, answering its questions with `input`, and shows its
/// progress ("[====   ] 40% Downloading…") as the step.
fn tool(st: &Arc<Mutex<Shared>>, ctx: &egui::Context, cmd: &mut Command, input: &str, what: &str) -> Res {
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("Couldn't start {what}: {e}"))?;
    if let Some(mut i) = child.stdin.take() {
        let input = input.to_string();
        std::thread::spawn(move || {
            let _ = i.write_all(input.as_bytes());
        });
    }
    let err = child.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = e.read_to_string(&mut s);
            s
        })
    });
    let mut tail: Vec<String> = vec![];
    if let Some(mut out) = child.stdout.take() {
        let mut buf = [0u8; 4096];
        let mut line = String::new();
        loop {
            if crate::device::cancelled(st) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Stopped".into());
            }
            let n = match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            for ch in String::from_utf8_lossy(&buf[..n]).chars() {
                if ch == '\r' || ch == '\n' {
                    let l = line.trim().to_string();
                    line.clear();
                    if l.is_empty() {
                        continue;
                    }
                    if l.contains('%') && l.starts_with('[') {
                        progress(st, ctx, &format!("{what}…  {}", l.trim_start_matches(|c| c == '[' || c == '=' || c == ' ' || c == ']')));
                    } else if !l.starts_with("Accept? (y/N)") && !l.contains("---") && l.len() < 300 {
                        tail.push(l);
                    }
                } else {
                    line.push(ch);
                }
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let err = err.and_then(|h| h.join().ok()).unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        tail.extend(err.lines().filter(|l| !l.trim().is_empty()).map(String::from));
        let t = tail[tail.len().saturating_sub(10)..].join("\n");
        Err(format!("{what} failed:\n{t}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_computer_needs_everything_and_a_ready_one_nothing() {
        let all = needs(&Have::default(), true);
        let parts: Vec<Part> = all.iter().map(|n| n.part).collect();
        assert_eq!(parts, [Part::Java, Part::CmdlineTools, Part::Packages, Part::Emulator, Part::Image, Part::Gradle]);
        assert!(all.iter().map(|n| n.mb).sum::<u32>() > 2000);
        // a build alone needs no emulator
        assert!(!needs(&Have::default(), false).iter().any(|n| matches!(n.part, Part::Emulator | Part::Image)));
        let ready = Have { java: true, sdk: Some("/sdk".into()), sdkmanager: true, packages: true, emulator: true, image: true, avd: true, gradle: true };
        assert!(needs(&ready, true).is_empty());
        // Android Studio without an emulator yet: only the system image
        let studio = Have { avd: false, image: false, ..ready.clone() };
        assert_eq!(needs(&studio, true).iter().map(|n| n.part).collect::<Vec<_>>(), [Part::Image]);
        assert_eq!(size(1600), "1.6 GB");
        assert_eq!(size(340), "340 MB");
    }

    #[test]
    fn picks_the_newest_command_line_tools() {
        let xml = r#"<url>commandlinetools-mac-11076708_latest.zip</url><url>commandlinetools-linux-13114758_latest.zip</url><url>commandlinetools-mac-13114758_latest.zip</url><url>commandlinetools-mac-9_x.zip</url>"#;
        assert_eq!(latest_cmdline_tools(xml, "mac").as_deref(), Some("commandlinetools-mac-13114758_latest.zip"));
        assert_eq!(latest_cmdline_tools("", "mac"), None);
    }
}
