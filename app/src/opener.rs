//! Opening project files from outside the window: a .bw file double-clicked
//! in Finder, Explorer or a Linux file manager, or named on the command line.
//!
//! - macOS hands files to the running app through `application:openURLs:`
//!   on the app's delegate, a method added here to winit's delegate.
//! - Windows and Linux start Blockwise with the file's path. If Blockwise is
//!   already open, the new one passes the path to it over a local socket and
//!   quits, so two copies never save over each other's projects.
//! - On Windows and Linux, Blockwise registers itself for .bw files in the
//!   person's own settings (no administrator rights), once per place it runs
//!   from. The macOS app declares the file type in its Info.plist.
//!
//! The window picks up what arrived with `take`.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// The local port the first Blockwise listens on for files from later ones.
const PORT: u16 = 7317;
/// The first line a later Blockwise sends, so nothing else (a web page
/// posting to localhost, say) can pass for one.
const HELLO: &str = "BLOCKWISE-OPEN 1";

static PENDING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static CTX: OnceLock<egui::Context> = OnceLock::new();

/// Whether Blockwise opens this kind of file.
pub fn is_project_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| [crate::model::BW_EXT, "json", "zip"].iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Run first thing. Collects the project files named on the command line
/// and starts listening for more. Returns false when another Blockwise is
/// open and took the files: this one should quit.
pub fn start(args: &[String]) -> bool {
    let files: Vec<PathBuf> = args.iter().skip(1).map(PathBuf::from).filter(|p| is_project_file(p) && p.is_file()).map(|p| std::fs::canonicalize(&p).unwrap_or(p)).collect();
    match TcpListener::bind(("127.0.0.1", PORT)) {
        Ok(listener) => {
            std::thread::spawn(move || listen(listener));
        }
        Err(_) if !files.is_empty() => {
            if hand_over(&files).is_ok() {
                return false;
            }
        }
        Err(_) => {}
    }
    push(files);
    #[cfg(target_os = "macos")]
    mac::install();
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    std::thread::spawn(register);
    true
}

/// Wakes the window when files arrive.
pub fn set_ctx(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
    if !PENDING.lock().unwrap().is_empty() {
        ctx.request_repaint();
    }
}

/// The files that arrived since the last call.
pub fn take() -> Vec<PathBuf> {
    std::mem::take(&mut *PENDING.lock().unwrap())
}

fn push(files: Vec<PathBuf>) {
    if files.is_empty() {
        return;
    }
    PENDING.lock().unwrap().extend(files);
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

fn hand_over(files: &[PathBuf]) -> std::io::Result<()> {
    let mut s = TcpStream::connect_timeout(&([127, 0, 0, 1], PORT).into(), Duration::from_secs(2))?;
    let mut msg = format!("{HELLO}\n");
    for f in files {
        msg.push_str(&format!("{}\n", f.display()));
    }
    // a blank line ends the list
    msg.push('\n');
    s.write_all(msg.as_bytes())?;
    // the other side answers once it has the files
    s.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut ok = String::new();
    BufReader::new(s).read_line(&mut ok)?;
    if ok.trim() == "ok" {
        Ok(())
    } else {
        Err(std::io::Error::other("not taken"))
    }
}

fn listen(listener: TcpListener) {
    for mut conn in listener.incoming().flatten() {
        let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
        let Ok(reader) = conn.try_clone() else { continue };
        let mut lines = BufReader::new(reader).lines().map_while(|l| l.ok());
        if lines.next().as_deref() != Some(HELLO) {
            continue;
        }
        let files: Vec<PathBuf> = lines.take_while(|l| !l.is_empty()).take(100).map(PathBuf::from).filter(|p| is_project_file(p) && p.is_file()).collect();
        push(files);
        let _ = conn.write_all(b"ok\n");
        if let Some(ctx) = CTX.get() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }
}

/// Registers Blockwise for .bw files, once per place it runs from.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn register() {
    let Ok(exe) = std::env::current_exe() else { return };
    let mark = crate::model::Store::path().with_file_name("file-type.txt");
    let want = format!("{} {}", env!("CARGO_PKG_VERSION"), exe.display());
    if std::fs::read_to_string(&mark).is_ok_and(|m| m == want) {
        return;
    }
    if register_for(&exe) {
        let _ = std::fs::create_dir_all(mark.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(&mark, want);
    }
}

/// Windows: the per-user file type keys, which Explorer reads.
#[cfg(target_os = "windows")]
fn register_for(exe: &Path) -> bool {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    // the same id the macOS app gives the file type
    const PROG_ID: &str = "dev.eledrissi.blockwise.project";
    let exe = exe.display();
    let keys = [
        (r"HKCU\Software\Classes\.bw".to_string(), PROG_ID.to_string()),
        (format!(r"HKCU\Software\Classes\{PROG_ID}"), "Blockwise project".to_string()),
        (format!(r"HKCU\Software\Classes\{PROG_ID}\DefaultIcon"), format!("\"{exe}\",0")),
        (format!(r"HKCU\Software\Classes\{PROG_ID}\shell\open\command"), format!("\"{exe}\" \"%1\"")),
    ];
    keys.iter().all(|(key, value)| std::process::Command::new("reg").args(["add", key.as_str(), "/ve", "/d", value.as_str(), "/f"]).creation_flags(NO_WINDOW).output().is_ok_and(|o| o.status.success()))
}

/// Linux: a MIME type for *.bw and a desktop entry that opens it, in the
/// person's own data folder, as the freedesktop.org standards describe.
#[cfg(target_os = "linux")]
fn register_for(exe: &Path) -> bool {
    use std::process::Command;
    let Some(share) = dirs::data_dir() else { return false };
    const MIME: &str = "application/x-blockwise-project";
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n  <mime-type type=\"{MIME}\">\n    <comment>Blockwise project</comment>\n    <glob pattern=\"*.bw\"/>\n  </mime-type>\n</mime-info>\n"
    );
    // the release package has its icon beside the program
    let icon = exe.with_file_name("blockwise.png");
    let icon = if icon.is_file() { icon.display().to_string() } else { "blockwise".into() };
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=Blockwise\nComment=Build Android and iOS apps visually\nExec=\"{}\" %f\nIcon={icon}\nCategories=Development;\nMimeType={MIME};\nStartupWMClass={id}\nTerminal=false\n",
        exe.display(),
        id = crate::model::APP_ID
    );
    // named after the app's id, which the window also gives, so the desktop
    // matches the window to this entry and its icon
    let entry = format!("{}.desktop", crate::model::APP_ID);
    let (mime_dir, apps) = (share.join("mime"), share.join("applications"));
    let written = std::fs::create_dir_all(mime_dir.join("packages")).is_ok()
        && std::fs::create_dir_all(&apps).is_ok()
        && std::fs::write(mime_dir.join(format!("packages/{}.xml", crate::model::APP_ID)), xml).is_ok()
        && std::fs::write(apps.join(&entry), desktop).is_ok();
    // these tools may be missing; the files above still work after the next login
    let _ = Command::new("update-mime-database").arg(&mime_dir).output();
    let _ = Command::new("update-desktop-database").arg(&apps).output();
    let _ = Command::new("xdg-mime").args(["default", entry.as_str(), MIME]).output();
    written
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::declare::ClassBuilder;
    use objc2::runtime::{AnyObject, NSObject, Sel};
    use objc2::{class, msg_send, sel, ClassType};
    use std::ffi::{c_char, CStr, CString};

    /// `application:openURLs:`: Finder (or `open`) gives the app files.
    extern "C" fn open_urls(_this: *mut AnyObject, _cmd: Sel, _app: *mut AnyObject, urls: *mut AnyObject) {
        let mut files = vec![];
        unsafe {
            let n: usize = msg_send![urls, count];
            for i in 0..n {
                let url: *mut AnyObject = msg_send![urls, objectAtIndex: i];
                let path: *mut AnyObject = msg_send![url, path];
                if path.is_null() {
                    continue;
                }
                let c: *const c_char = msg_send![path, UTF8String];
                if !c.is_null() {
                    files.push(std::path::PathBuf::from(CStr::from_ptr(c).to_string_lossy().into_owned()));
                }
            }
        }
        super::push(files.into_iter().filter(|f| super::is_project_file(f)).collect());
    }

    /// When the app is about to finish launching (winit has made it and its
    /// delegate by then), teach winit's delegate `application:openURLs:`:
    /// macOS hands over the files the app was opened with right after this.
    extern "C" fn will_finish(_this: *mut AnyObject, _cmd: Sel, _note: *mut AnyObject) {
        unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let delegate: *mut AnyObject = msg_send![app, delegate];
            if delegate.is_null() {
                return;
            }
            let cls = objc2::ffi::object_getClass(delegate as *const _) as *mut objc2::ffi::objc_class;
            let imp: unsafe extern "C" fn() = std::mem::transmute(open_urls as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject));
            objc2::ffi::class_addMethod(cls, sel!(application:openURLs:).as_ptr(), Some(imp), c"v@:@@".as_ptr());
            // the app notes which delegate methods exist when it's given the
            // delegate: give it again so it sees the new one
            let _: () = msg_send![app, setDelegate: delegate];
        }
    }

    pub fn install() {
        let Some(mut b) = ClassBuilder::new("BlockwiseFileOpener", NSObject::class()) else { return };
        unsafe {
            b.add_method(sel!(willFinishLaunching:), will_finish as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject));
        }
        let cls = b.register();
        let name = CString::new("NSApplicationWillFinishLaunchingNotification").unwrap();
        unsafe {
            // kept for the app's whole life (the app holds its delegate weakly)
            let opener: *mut AnyObject = msg_send![cls, new];
            let name: *mut AnyObject = msg_send![class!(NSString), stringWithUTF8String: name.as_ptr()];
            let center: *mut AnyObject = msg_send![class!(NSNotificationCenter), defaultCenter];
            let _: () = msg_send![center, addObserver: opener, selector: sel!(willFinishLaunching:), name: name, object: std::ptr::null_mut::<AnyObject>()];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_files_by_extension() {
        for f in ["a.bw", "B.BW", "x/project.json", "folder.zip"] {
            assert!(is_project_file(Path::new(f)), "{f}");
        }
        for f in ["shot.png", "--open", "notes.txt", "bw"] {
            assert!(!is_project_file(Path::new(f)), "{f}");
        }
    }
}
