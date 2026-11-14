//! MCP (Model Context Protocol) server: lets AI assistants such as Claude
//! Code or Claude Desktop build apps in Blockwise.
//!
//! The server runs inside the open app, on 127.0.0.1 only, and speaks MCP's
//! Streamable HTTP transport (plain JSON replies, no event streams). Every
//! tool call is handed to the UI thread and runs against the same projects
//! the window shows, so changes appear live, land in undo, and can't race
//! the editor's own saving. `Blockwise --mcp` (see `bridge`) relays stdio to
//! the running app for clients that only start servers themselves.

use crate::blocks::{self, Param, Shape, Token};
use crate::model::*;
use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 7311;
const PROTOCOL: &str = "2025-06-18";
const KNOWN_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_BODY: usize = 8 << 20;

/// What the UI thread answers a tool call with.
pub enum Reply {
    Json(Value),
    Image { png: Vec<u8>, note: String },
    Error(String),
}

/// A tool call waiting for the UI thread.
pub struct Call {
    pub tool: String,
    pub args: Value,
    pub reply: Sender<Reply>,
}

/// The running server; dropping it stops it.
pub struct Server {
    pub port: u16,
    rx: Receiver<Call>,
    stop: Arc<AtomicBool>,
}

impl Server {
    pub fn start(port: u16, ctx: egui::Context) -> std::io::Result<Server> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        std::thread::Builder::new().name("mcp".into()).spawn(move || {
            for conn in listener.incoming() {
                if st.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                let (tx, ctx) = (tx.clone(), ctx.clone());
                let _ = std::thread::Builder::new().name("mcp-conn".into()).spawn(move || {
                    let _ = serve(conn, &tx, &ctx);
                });
            }
        })?;
        Ok(Server { port, rx, stop })
    }

    pub fn poll(&self) -> Vec<Call> {
        self.rx.try_iter().collect()
    }

    pub fn url(&self) -> String {
        url(self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // wake the accept loop so it sees the flag
        let _ = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], self.port)), Duration::from_millis(200));
    }
}

pub fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

// ---------- HTTP

struct Request {
    method: String,
    path: String,
    origin: Option<String>,
    close: bool,
    body: Vec<u8>,
}

fn read_request(r: &mut impl BufRead) -> std::io::Result<Option<Request>> {
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if !line.trim().is_empty() {
            break;
        }
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").split('?').next().unwrap_or("").to_string();
    let http10 = parts.next() == Some("HTTP/1.0");
    let (mut len, mut origin, mut close) = (0usize, None, http10);
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 {
            return Ok(None);
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let Some((k, v)) = h.split_once(':') else { continue };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => len = v.parse().unwrap_or(0),
            "origin" => origin = Some(v.to_string()),
            "connection" => close = v.eq_ignore_ascii_case("close"),
            _ => {}
        }
    }
    if len > MAX_BODY {
        return Err(std::io::Error::other("request too large"));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    Ok(Some(Request { method, path, origin, close, body }))
}

/// Only pages served from this machine may call in (a web page elsewhere
/// could otherwise reach the local port through the browser).
fn origin_ok(o: Option<&str>) -> bool {
    match o {
        None => true,
        Some(o) => {
            let host = o.split("://").nth(1).unwrap_or("").split(['/', ':']).next().unwrap_or("");
            matches!(host, "localhost" | "127.0.0.1" | "[::1]")
        }
    }
}

fn serve(conn: TcpStream, tx: &Sender<Call>, ctx: &egui::Context) -> std::io::Result<()> {
    conn.set_read_timeout(Some(Duration::from_secs(600)))?;
    let mut out = conn.try_clone()?;
    let mut r = BufReader::new(conn);
    while let Some(req) = read_request(&mut r)? {
        let (status, body): (u16, Option<Value>) = if !origin_ok(req.origin.as_deref()) {
            (403, Some(rpc_error(Value::Null, -32600, "Requests from web pages are not allowed")))
        } else if req.path != "/mcp" && req.path != "/" {
            (404, None)
        } else if req.method == "POST" {
            handle_post(&req.body, tx, ctx)
        } else {
            (405, None)
        };
        let reason = match status {
            200 => "OK",
            202 => "Accepted",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            _ => "Method Not Allowed",
        };
        let text = body.map(|b| b.to_string()).unwrap_or_default();
        let mut head = format!("HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n", text.len());
        if !text.is_empty() {
            head.push_str("Content-Type: application/json\r\n");
        }
        if status == 405 {
            head.push_str("Allow: POST\r\n");
        }
        if req.close {
            head.push_str("Connection: close\r\n");
        }
        head.push_str("\r\n");
        out.write_all(head.as_bytes())?;
        out.write_all(text.as_bytes())?;
        out.flush()?;
        if req.close {
            break;
        }
    }
    Ok(())
}

fn handle_post(body: &[u8], tx: &Sender<Call>, ctx: &egui::Context) -> (u16, Option<Value>) {
    let Ok(msg) = serde_json::from_slice::<Value>(body) else {
        return (400, Some(rpc_error(Value::Null, -32700, "Parse error")));
    };
    match msg {
        Value::Array(list) => {
            let replies: Vec<Value> = list.iter().filter_map(|m| answer(m, tx, ctx)).collect();
            if replies.is_empty() {
                (202, None)
            } else {
                (200, Some(Value::Array(replies)))
            }
        }
        m => match answer(&m, tx, ctx) {
            Some(r) => (200, Some(r)),
            None => (202, None),
        },
    }
}

fn rpc_error(id: Value, code: i64, msg: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg}})
}

/// The reply to one JSON-RPC message; None for notifications and responses.
fn answer(m: &Value, tx: &Sender<Call>, ctx: &egui::Context) -> Option<Value> {
    let id = m.get("id")?.clone();
    let Some(method) = m.get("method").and_then(|v| v.as_str()) else {
        return None; // a response to something we never send
    };
    let params = m.get("params").cloned().unwrap_or(json!({}));
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("");
            let version = if KNOWN_PROTOCOLS.contains(&asked) { asked } else { PROTOCOL };
            Ok(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "blockwise", "title": "Blockwise", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if !TOOLS.iter().any(|t| t.0 == name) {
                Err((-32602, format!("Unknown tool: {name}")))
            } else {
                let args = params.get("arguments").cloned().filter(|a| a.is_object()).unwrap_or(json!({}));
                Ok(call_ui(name, args, tx, ctx))
            }
        }
        _ => Err((-32601, format!("Method not found: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, msg)) => rpc_error(id, code, &msg),
    })
}

fn call_ui(tool: &str, args: Value, tx: &Sender<Call>, ctx: &egui::Context) -> Value {
    let (reply, rx) = channel();
    let sent = tx.send(Call { tool: tool.into(), args, reply }).is_ok();
    ctx.request_repaint();
    let got = if sent { rx.recv_timeout(Duration::from_secs(90)).ok() } else { None };
    let text = |t: String, err: bool| json!({"content": [{"type": "text", "text": t}], "isError": err});
    match got {
        Some(Reply::Json(v)) => text(serde_json::to_string_pretty(&v).unwrap_or_default(), false),
        Some(Reply::Image { png, note }) => json!({"content": [{"type": "image", "data": base64(&png), "mimeType": "image/png"}, {"type": "text", "text": note}], "isError": false}),
        Some(Reply::Error(e)) => text(e, true),
        None => text("Blockwise didn't answer in time. Is its window open (not minimised)?".into(), true),
    }
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

// ---------- stdio bridge

/// `Blockwise --mcp`: relay MCP messages between stdin/stdout and the
/// running app's server, for clients that only start stdio servers.
pub fn bridge(port: u16) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match post(port, &line) {
            Ok(Some(body)) => Some(body),
            Ok(None) => None,
            Err(e) => {
                // answer requests ourselves so the client isn't left waiting
                let id = serde_json::from_str::<Value>(&line).ok().and_then(|m| m.get("id").cloned());
                id.map(|id| rpc_error(id, -32000, &format!("Can't reach Blockwise on port {port} ({e}). Open Blockwise and turn on Settings › AI assistants (MCP).")).to_string())
            }
        };
        if let Some(r) = reply {
            writeln!(stdout, "{}", r.replace('\n', " "))?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn post(port: u16, body: &str) -> std::io::Result<Option<String>> {
    let mut s = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_secs(3))?;
    s.set_read_timeout(Some(Duration::from_secs(120)))?;
    write!(s, "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())?;
    let mut text = String::new();
    s.read_to_string(&mut text)?;
    let (head, rest) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status: u16 = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    match status {
        202 => Ok(None),
        200 | 400 | 403 => Ok(Some(rest.to_string()).filter(|r| !r.is_empty())),
        _ => Err(std::io::Error::other(format!("HTTP {status}"))),
    }
}

// ---------- tools

const INSTRUCTIONS: &str = "Blockwise builds phone apps (Android in Kotlin, iOS in SwiftUI) from a widget designer and Scratch-like blocks. \
Work like this: list_projects (or create_project), then get_screen to see a screen's widget tree, variables and events. \
Build the layout with add_widget (it can add a whole tree at once through `children`) and update_widget; rows and columns are LinearLayouts: \
`weight` shares the spare room along the parent's direction, `gravity` places the children. Call preview_screenshot to look at the result. \
For behaviour, read list_blocks once, add any variables or components the blocks need (add_variable, add_component), then set_event_blocks for each event \
(e.g. a button's onClick, or the screen's onCreate). Blocks are JSON: {\"op\": \"setText\", \"args\": [\"textview1\", \"Hello\"]}; an argument may be another \
reporter block; C-shaped blocks take \"do\": [...], if/else also \"else\": [...]. Everything you change shows up live in the open Blockwise window and can be undone there (⌘Z). \
For sign-in and online data, link_supabase links the project to a Supabase project, then the Supabase blocks (category Supabase) sign people in and \
read and write tables; their \"do\" runs on success and \"else\" on failure, and inside them webResponse is the answer (rows come as a JSON list: use listMapFromJson) and supaError the readable error. \
get_code shows the generated Kotlin, Swift or layout XML; run_on_device builds and runs the app on the Android emulator or the iOS simulator, and device_status reports how it went.";

/// (name, title, description, input schema properties, required, read only)
type ToolDef = (&'static str, &'static str, &'static str, fn() -> Value, &'static [&'static str], bool);

fn pid() -> Value {
    json!({"type": "integer", "description": "The project's id (from list_projects)"})
}
fn scr() -> Value {
    json!({"type": "string", "description": "Screen name, e.g. \"main\" (default: the first screen)"})
}

const TOOLS: &[ToolDef] = &[
    ("list_projects", "List projects", "Lists the projects in Blockwise with their ids and screens, and which one is open in the window.", || json!({}), &[], true),
    ("create_project", "Create a project", "Makes a new project with one empty screen called \"main\" and returns its id.", || json!({
        "app_name": {"type": "string", "description": "The name shown under the app's icon"},
        "package": {"type": "string", "description": "App id like com.example.notes (default: made from the name)"},
    }), &["app_name"], false),
    ("open_project", "Open a project", "Opens a project in the Blockwise window, on its design view. Fails if another project with unsaved changes is open.", || json!({
        "project_id": pid(), "screen": scr(),
    }), &["project_id"], false),
    ("get_screen", "Read a screen", "A screen's widget tree with every property, its variables, lists, components and events with their blocks (in the same JSON set_event_blocks takes).", || json!({
        "project_id": pid(), "screen": scr(),
    }), &["project_id"], true),
    ("update_screen", "Change a screen", "Sets a screen's title and its app bar (the bar at the top with the title). \
Colours are \"#RRGGBB\", or \"default\" for the phone's own look: the theme colour with a white title on Android, the system bar on iPhone. \
A title colour left as default is white or black, whichever reads on the bar.", || json!({
        "project_id": pid(), "screen": scr(),
        "title": {"type": "string", "description": "The bar's title; empty shows the app's name"},
        "app_bar": {"type": "boolean", "description": "false hides the bar: the screen starts right under the status bar"},
        "bar_color": {"type": "string"},
        "title_color": {"type": "string"},
        "back_button": {"type": "boolean", "description": "false: no back arrow, even when the screen was opened from another (the phone's back still works)"},
        "opens_as": {"type": "string", "enum": ["page", "half_sheet", "full_sheet"], "description": "page: a whole screen (default); half_sheet / full_sheet: a bottom sheet that slides up over the screen that opens it (openScreen), half or nearly all of the phone's height, and closes with finishActivity or a tap above it. A sheet has no app bar. Not for the first screen."},
        "actions": {"type": "array", "items": {"type": "object"}, "description": "Icon buttons on the right of the bar, replacing the ones there: [{\"icon\": \"search\", \"label\": \"Search\", \"id\": optional}]. \
Icons: search, add, edit, delete, share, done, close, refresh, settings, info, help, more, menu, filter, favorite, star, bookmark, notifications, person, home, cart, camera, send, mail, phone, calendar, download, logout. Each button's id (default: icon + \"Button\", e.g. searchButton) takes an onClick event through set_event_blocks; buttons kept by id keep their events."},
    }), &["project_id"], false),
    ("link_supabase", "Link Supabase", "Links the project to a Supabase project, for the Supabase blocks (sign up, sign in, table rows). \
The URL and the public (anon or publishable) key are in the Supabase dashboard under Project Settings → API; never the secret / service_role key. \
An empty url unlinks.", || json!({
        "project_id": pid(),
        "url": {"type": "string", "description": "e.g. https://abcd.supabase.co"},
        "key": {"type": "string", "description": "The anon or publishable key"},
    }), &["project_id", "url"], false),
    ("add_screen", "Add a screen", "Adds a screen to the project, empty or from a ready-made template with its widgets, variables and blocks already in place. \
Its activity is the name in CamelCase + Activity (\"second\" → SecondActivity), used by the openScreen block.", || json!({
        "project_id": pid(), "name": {"type": "string", "description": "Small letters, digits and _, e.g. \"settings\""},
        "template": {"type": "string", "enum": crate::templates::TEMPLATES.iter().map(|t| t.id).collect::<Vec<_>>(),
            "description": crate::templates::TEMPLATES.iter().map(|t| format!("{}: {}", t.id, t.description)).collect::<Vec<_>>().join("; ")},
    }), &["project_id", "name"], false),
    ("add_widget", "Add widgets", "Adds a widget, or a whole tree of them through `children`, into a layout. Returns the ids given. \
Properties (all optional): width/height (\"match_parent\", \"wrap_content\" or a number of dp), padding, margin (dp), gravity (none/left/center/right: where children sit across), \
vertical_gravity (none/top/center/bottom), weight (share of the parent's spare room), background, corner_radius (dp), shadow (dp, needs a background), border (dp), border_color, picture (an image's file, from add_file), fit (whole or crop), text, text_size (sp), text_color, bold, font (a font file's name, from add_file), video (a video widget's file, from add_file), autoplay, loop, controls (video widgets), hint, hint_color, input (text fields: text, password, email, number, decimal, phone or multiline), checked, progress, max, enabled, visible. \
Colours are \"#RRGGBB\", \"#AARRGGBB\" or \"transparent\".", || json!({
        "project_id": pid(), "screen": scr(),
        "parent": {"type": "string", "description": "Id of a row/column to put it in, or \"root\" for the screen itself (default)"},
        "index": {"type": "integer", "description": "Position among the parent's children (default: last)"},
        "widget": {"type": "object", "description": "{\"type\": one of row, column, scrolling_row, scrolling_column, button, text, text_field, image, web_page, progress_bar, list, dropdown, checkbox, switch, slider, calendar; \"id\": optional name; any properties; \"children\": [more widgets, for layouts]}"},
    }), &["project_id", "widget"], false),
    ("update_widget", "Change a widget", "Sets some of a widget's properties (see add_widget for the list). Giving \"id\" renames it everywhere, blocks included.", || json!({
        "project_id": pid(), "screen": scr(),
        "id": {"type": "string", "description": "The widget to change"},
        "set": {"type": "object", "description": "Properties to set, e.g. {\"text\": \"Hi\", \"width\": \"match_parent\"}"},
    }), &["project_id", "id", "set"], false),
    ("move_widget", "Move a widget", "Moves a widget (with what it holds) into another layout or to another position.", || json!({
        "project_id": pid(), "screen": scr(),
        "id": {"type": "string"},
        "parent": {"type": "string", "description": "Layout id or \"root\""},
        "index": {"type": "integer", "description": "Position among the new parent's children (default: last)"},
    }), &["project_id", "id", "parent"], false),
    ("delete_widget", "Delete a widget", "Deletes a widget, everything inside it, and its events.", || json!({
        "project_id": pid(), "screen": scr(), "id": {"type": "string"},
    }), &["project_id", "id"], false),
    ("add_variable", "Add a variable", "Adds a variable or list to a screen, for blocks to use.", || json!({
        "project_id": pid(), "screen": scr(),
        "name": {"type": "string"},
        "type": {"type": "string", "enum": ["number", "string", "boolean", "map", "list_number", "list_string", "list_map"]},
    }), &["project_id", "name", "type"], false),
    ("add_file", "Add an asset", "Copies a picture (PNG, JPEG), sound (MP3, WAV, M4A), video (MP4, M4V, MOV), font (TTF, OTF) or data file (TXT, JSON, CSV, XML, HTML, MD) from this computer into the project's assets. Returns the name it got, \
for an image widget's \"picture\" property or the setImage block, a sound player's soundPlay block, a text widget's \"font\" property or the setFont block, or the assetText block (a data file's text). A picture's pixels show as dp.", || json!({
        "project_id": pid(),
        "path": {"type": "string", "description": "Full path of the file on this computer"},
    }), &["project_id", "path"], false),
    ("list_files", "List assets", "The project's assets (pictures, sounds, fonts, data files) with their names, kinds and where each is used.", || json!({
        "project_id": pid(),
    }), &["project_id"], true),
    ("rename_file", "Rename an asset", "Gives an asset a new name; widgets and blocks that use it follow. Returns the name it got (small letters, digits and _).", || json!({
        "project_id": pid(),
        "name": {"type": "string"},
        "new_name": {"type": "string"},
    }), &["project_id", "name", "new_name"], false),
    ("delete_file", "Delete an asset", "Takes an asset out of the project. Widgets and blocks that used it are left without one (the reply lists them).", || json!({
        "project_id": pid(),
        "name": {"type": "string"},
    }), &["project_id", "name"], false),
    ("add_component", "Add a component", "Adds a component to a screen: timer, popup (dialog), navigation (intent), saved_data, vibration, date_time, web, sound (a player for the project's sounds: soundPlay, soundStop...; soundPlayFrom plays a file path or web address) \
or camera (cameraPhoto / cameraVideo take a photo or record a video; pickMedia, which needs no component, picks a picture, video or sound from the phone; \
their \"do\" runs with the file, whose path is mediaPath, \"else\" when cancelled; setImageFrom, setVideoFrom and soundPlayFrom show or play that path) \
(web requests: webSetHeader, then webRequest with method, URL and body; its \"do\" runs on a 2xx answer, \"else\" on anything else, \
and inside them webResponse is the answer's text and webStatus its code. mapFromJson / listMapFromJson read that text).", || json!({
        "project_id": pid(), "screen": scr(),
        "type": {"type": "string", "enum": ["timer", "popup", "navigation", "saved_data", "vibration", "date_time", "web", "sound"]},
        "name": {"type": "string", "description": "Default: t, d, i, data, vib, cal, web or player (numbered if taken)"},
    }), &["project_id", "type"], false),
    ("list_blocks", "List the blocks", "The block catalogue: each block's op, shape and what its arguments are. Read this before set_event_blocks.", || json!({
        "category": {"type": "string", "description": "Only one category: Variable, List, Control, Operator, Math, Saved data, View, Component"},
    }), &[], true),
    ("set_event_blocks", "Program an event", "Replaces the blocks of an event (adding the event if it isn't there). Events: the screen's onCreate / onBackPressed (target \"screen\"), \
or a widget's: onClick, onLongClick (buttons, texts, images, layouts), onCheckedChange (checkbox, switch), onTextChanged (text field), onProgressChanged (slider), onItemClicked (list), onItemSelected (dropdown), onCompletion (video: it played to the end). \
Each block is {\"op\", \"args\": [...], \"do\": [...], \"else\": [...]}; args are literals (text, numbers, or a drop-down's value such as a widget id or variable name) or reporter blocks. \
The blocks are checked like the editor checks them; errors say which block and argument is wrong.", || json!({
        "project_id": pid(), "screen": scr(),
        "target": {"type": "string", "description": "A widget id, or \"screen\""},
        "event": {"type": "string", "description": "e.g. onClick or onCreate"},
        "blocks": {"type": "array", "items": {"type": "object"}},
    }), &["project_id", "target", "event", "blocks"], false),
    ("set_more_block", "Make a more block", "Makes (or changes) a more block: your own block, defined once and used anywhere on the screen, to keep projects organised. \
Its inputs are {\"name\", \"type\": text, number or boolean}. Inside its blocks, read an input with paramStr / paramNum / paramBool and the input's name. \
Use it with {\"op\": \"moreBlock\", \"args\": [name, value for each input...]}. Changing the inputs keeps the values in existing uses whose input names stay.", || json!({
        "project_id": pid(), "screen": scr(),
        "name": {"type": "string"},
        "inputs": {"type": "array", "items": {"type": "object"}, "description": "e.g. [{\"name\": \"price\", \"type\": \"number\"}]"},
        "blocks": {"type": "array", "items": {"type": "object"}},
    }), &["project_id", "name", "blocks"], false),
    ("preview_screenshot", "Look at the screen", "Opens the screen in the designer and returns a picture of the phone preview.", || json!({
        "project_id": pid(), "screen": scr(),
        "device": {"type": "string", "enum": ["iphone", "android"], "description": "Which phone to show (default: the one picked in Blockwise)"},
    }), &["project_id"], true),
    ("get_code", "Read the generated code", "The Kotlin activity, the SwiftUI view, or the Android layout XML Blockwise makes for a screen.", || json!({
        "project_id": pid(), "screen": scr(),
        "language": {"type": "string", "enum": ["kotlin", "swift", "layout_xml"]},
    }), &["project_id", "language"], true),
    ("run_on_device", "Run on a phone", "Opens the project and builds and runs it on the Android emulator or the iOS simulator (this takes a while; follow with device_status).", || json!({
        "project_id": pid(),
        "device": {"type": "string", "enum": ["android", "iphone"]},
    }), &["project_id", "device"], false),
    ("device_status", "Check the device run", "How the last run_on_device is going: its phase, current step and the end of its log.", || json!({
        "lines": {"type": "integer", "description": "How many log lines (default 40)"},
    }), &[], true),
];

fn tools() -> Value {
    Value::Array(
        TOOLS
            .iter()
            .map(|(name, title, desc, props, req, ro)| {
                json!({
                    "name": name, "title": title, "description": desc,
                    "inputSchema": {"type": "object", "properties": props(), "required": req},
                    "annotations": {"title": title, "readOnlyHint": ro, "destructiveHint": *name == "delete_widget" || *name == "delete_file" || *name == "set_event_blocks", "openWorldHint": false},
                })
            })
            .collect(),
    )
}

// ---------- tools that work on the projects alone

type R<T> = Result<T, String>;

fn arg_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(|v| v.as_str())
}

fn need_str<'a>(a: &'a Value, k: &str) -> R<&'a str> {
    arg_str(a, k).ok_or_else(|| format!("\"{k}\" is needed (a string)"))
}

pub fn project_id(a: &Value) -> R<u32> {
    a.get("project_id").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).map(|v| v as u32).ok_or_else(|| "\"project_id\" is needed (from list_projects)".into())
}

pub fn find_project<'a>(store: &'a mut Store, a: &Value) -> R<&'a mut Project> {
    let id = project_id(a)?;
    let ids: Vec<u32> = store.projects.iter().map(|p| p.id).collect();
    store.projects.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("No project {id}; the projects are {ids:?}"))
}

/// The screen named in the arguments (by name, activity or shown name), else the first.
pub fn screen_index(p: &Project, a: &Value) -> R<usize> {
    let Some(want) = arg_str(a, "screen").filter(|s| !s.trim().is_empty()) else { return Ok(0) };
    let w = want.trim().to_lowercase();
    p.screens
        .iter()
        .position(|s| s.name.to_lowercase() == w || s.activity_name().to_lowercase() == w || s.display_name().to_lowercase() == w)
        .ok_or_else(|| format!("No screen \"{want}\"; the screens are {:?}", p.screens.iter().map(|s| &s.name).collect::<Vec<_>>()))
}

const KINDS: [(&str, WKind); 17] = [
    ("row", WKind::LinearH),
    ("column", WKind::LinearV),
    ("scrolling_row", WKind::ScrollH),
    ("scrolling_column", WKind::ScrollV),
    ("button", WKind::Button),
    ("text", WKind::TextView),
    ("text_field", WKind::EditText),
    ("image", WKind::ImageView),
    ("web_page", WKind::WebView),
    ("progress_bar", WKind::ProgressBar),
    ("list", WKind::ListView),
    ("dropdown", WKind::Spinner),
    ("checkbox", WKind::CheckBox),
    ("switch", WKind::Switch),
    ("slider", WKind::SeekBar),
    ("calendar", WKind::CalendarView),
    ("video", WKind::VideoView),
];

fn kind_name(k: WKind) -> &'static str {
    KINDS.iter().find(|(_, x)| *x == k).map(|(n, _)| *n).unwrap_or("?")
}

fn parse_kind(s: &str) -> R<WKind> {
    let norm = |t: &str| t.to_lowercase().replace([' ', '_', '-'], "");
    let n = norm(s);
    KINDS
        .iter()
        .find(|(name, k)| norm(name) == n || norm(&format!("{k:?}")) == n || norm(k.palette_name()) == n || norm(k.android_class()) == n && !k.is_container())
        .map(|(_, k)| *k)
        .ok_or_else(|| format!("Unknown widget type \"{s}\"; use one of {}", KINDS.iter().map(|k| k.0).collect::<Vec<_>>().join(", ")))
}

fn size_json(s: Size) -> Value {
    match s {
        Size::Wrap => json!("wrap_content"),
        Size::Match => json!("match_parent"),
        Size::Px(p) => json!(p),
    }
}

fn parse_size(v: &Value) -> R<Size> {
    if let Some(n) = v.as_f64() {
        return Ok(Size::Px((n.round() as i32).clamp(1, 2000)));
    }
    let s = v.as_str().unwrap_or("").trim().to_lowercase();
    match s.as_str() {
        "match_parent" | "match" | "fill_parent" | "fill" => Ok(Size::Match),
        "wrap_content" | "wrap" => Ok(Size::Wrap),
        _ => s.trim_end_matches("dp").trim().parse::<f64>().map(|n| Size::Px((n.round() as i32).clamp(1, 2000))).map_err(|_| format!("A size is \"match_parent\", \"wrap_content\" or a number of dp, not {v}")),
    }
}

pub fn color_json(c: u32) -> String {
    match c >> 24 {
        0 => "transparent".into(),
        0xFF => format!("#{:06X}", c & 0xFF_FFFF),
        _ => format!("#{c:08X}"),
    }
}

fn parse_color(v: &Value) -> R<u32> {
    let s = v.as_str().unwrap_or("").trim();
    if s.eq_ignore_ascii_case("transparent") || s.eq_ignore_ascii_case("none") {
        return Ok(0x00FF_FFFF);
    }
    let hex = s.trim_start_matches('#').trim_start_matches("0x").trim_start_matches("0X");
    let n = u32::from_str_radix(hex, 16).map_err(|_| format!("A colour is \"#RRGGBB\", \"#AARRGGBB\" or \"transparent\", not {v}"))?;
    match hex.len() {
        6 => Ok(0xFF00_0000 | n),
        8 => Ok(n),
        3 => {
            let (r, g, b) = (n >> 8 & 15, n >> 4 & 15, n & 15);
            Ok(0xFF00_0000 | (r * 17) << 16 | (g * 17) << 8 | b * 17)
        }
        _ => Err(format!("A colour is \"#RRGGBB\", \"#AARRGGBB\" or \"transparent\", not {v}")),
    }
}

fn gravity_name(g: Gravity) -> &'static str {
    match g {
        Gravity::None => "none",
        Gravity::Left => "left",
        Gravity::Center => "center",
        Gravity::Right => "right",
    }
}

fn vgravity_name(g: VGravity) -> &'static str {
    match g {
        VGravity::None => "none",
        VGravity::Top => "top",
        VGravity::Center => "center",
        VGravity::Bottom => "bottom",
    }
}

fn widget_json(s: &Screen, w: &Widget) -> Value {
    let mut o = json!({
        "id": w.id, "type": kind_name(w.kind),
        "width": size_json(w.width), "height": size_json(w.height),
        "padding": w.padding, "margin": w.margin,
        "gravity": gravity_name(w.gravity), "vertical_gravity": vgravity_name(w.vgravity),
        "weight": w.weight, "background": color_json(w.bg),
    });
    let m = o.as_object_mut().unwrap();
    if w.radius > 0 {
        m.insert("corner_radius".into(), json!(w.radius));
    }
    if w.elevation > 0 {
        m.insert("shadow".into(), json!(w.elevation));
    }
    if w.kind == WKind::ImageView {
        m.insert("picture".into(), json!(w.image));
        m.insert("fit".into(), json!(if w.crop { "crop" } else { "whole" }));
    }
    if w.kind == WKind::VideoView {
        m.insert("video".into(), json!(w.video));
        m.insert("autoplay".into(), json!(w.autoplay));
        m.insert("loop".into(), json!(w.looping));
        m.insert("controls".into(), json!(w.controls));
    }
    if !w.font.is_empty() {
        m.insert("font".into(), json!(w.font));
    }
    if w.border > 0 {
        m.insert("border".into(), json!(w.border));
        m.insert("border_color".into(), json!(color_json(w.border_color)));
    }
    if w.kind.has_text() {
        m.insert("text".into(), json!(w.text));
        m.insert("text_size".into(), json!(w.text_size));
        m.insert("text_color".into(), json!(color_json(w.text_color)));
        m.insert("bold".into(), json!(w.bold));
    }
    if w.kind == WKind::EditText {
        m.insert("hint".into(), json!(w.hint));
        m.insert("hint_color".into(), json!(color_json(w.hint_color)));
        m.insert("input".into(), json!(w.input_kind().key()));
    }
    if w.kind.is_compound() {
        m.insert("checked".into(), json!(w.checked));
    }
    if matches!(w.kind, WKind::SeekBar | WKind::ProgressBar) {
        m.insert("progress".into(), json!(w.progress));
        m.insert("max".into(), json!(w.max));
    }
    if !w.enabled {
        m.insert("enabled".into(), json!(false));
    }
    if !w.visible {
        m.insert("visible".into(), json!(false));
    }
    let events: Vec<&str> = w.kind.events().to_vec();
    if !events.is_empty() {
        m.insert("events".into(), json!(events));
    }
    if w.kind.is_container() {
        m.insert("children".into(), Value::Array(s.children(&w.id).into_iter().map(|i| widget_json(s, &s.widgets[i])).collect()));
    }
    o
}

const PROPS: &str = "width, height, padding, margin, gravity, vertical_gravity, weight, background, corner_radius, shadow, border, border_color, picture, fit, text, text_size, text_color, bold, font, video, autoplay, loop, controls, hint, hint_color, input, password, checked, progress, max, enabled, visible, id";

/// Set properties on a widget (everything checked before anything changes).
fn apply_props(s: &mut Screen, id: &str, props: &Map<String, Value>) -> R<String> {
    let mut w = s.widget(id).ok_or_else(|| format!("No widget \"{id}\" on this screen"))?.clone();
    let num = |k: &str, v: &Value| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().trim_end_matches("dp").trim_end_matches("sp").parse().ok())).ok_or_else(|| format!("\"{k}\" is a number, not {v}"));
    let boolean = |k: &str, v: &Value| v.as_bool().ok_or_else(|| format!("\"{k}\" is true or false, not {v}"));
    let text = |k: &str, v: &Value| match v {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(format!("\"{k}\" is text, not {v}")),
    };
    let mut rename = None;
    for (k, v) in props {
        match k.as_str() {
            "type" | "children" => {}
            "id" => {
                let n = text(k, v)?;
                if n != w.id {
                    if let Some(p) = crate::logic::name_problem(s, &n) {
                        return Err(format!("Can't call it \"{n}\": {p}"));
                    }
                    rename = Some(n);
                }
            }
            "width" => w.width = parse_size(v)?,
            "height" => w.height = parse_size(v)?,
            "padding" => w.padding = num(k, v)?.round().clamp(0.0, 200.0) as i32,
            "margin" => w.margin = num(k, v)?.round().clamp(0.0, 200.0) as i32,
            "weight" => w.weight = num(k, v)?.clamp(0.0, 10.0) as f32,
            "gravity" | "horizontal_gravity" => {
                w.gravity = match v.as_str().unwrap_or("").to_lowercase().as_str() {
                    "none" | "" => Gravity::None,
                    "left" | "start" => Gravity::Left,
                    "center" | "center_horizontal" | "middle" => Gravity::Center,
                    "right" | "end" => Gravity::Right,
                    _ => return Err(format!("\"gravity\" is none, left, center or right, not {v}")),
                }
            }
            "vertical_gravity" => {
                let vg = match v.as_str().unwrap_or("").to_lowercase().as_str() {
                    "none" | "" => VGravity::None,
                    "top" => VGravity::Top,
                    "center" | "center_vertical" | "middle" => VGravity::Center,
                    "bottom" => VGravity::Bottom,
                    _ => return Err(format!("\"vertical_gravity\" is none, top, center or bottom, not {v}")),
                };
                // stored as one number, as the editor does
                (w.gravity, w.vgravity) = gravity_from_flags(gravity_flags(w.gravity, vg));
            }
            "background" | "bg" => w.bg = parse_color(v)?,
            "corner_radius" | "radius" | "corners" => w.radius = num(k, v)?.round().clamp(0.0, 200.0) as i32,
            "shadow" | "elevation" => w.elevation = num(k, v)?.round().clamp(0.0, 24.0) as i32,
            "border" | "border_width" => w.border = num(k, v)?.round().clamp(0.0, 20.0) as i32,
            "border_color" => w.border_color = parse_color(v)?,
            "picture" | "image" | "src" => w.image = text(k, v)?,
            "font" | "font_family" | "typeface" => w.font = text(k, v)?,
            "fit" | "crop" => {
                w.crop = match v {
                    Value::Bool(b) => *b,
                    _ => match text(k, v)?.to_lowercase().as_str() {
                        "crop" | "fill" | "cut" | "center_crop" => true,
                        "whole" | "fit" | "fit_center" | "" => false,
                        o => return Err(format!("\"fit\" is whole or crop, not \"{o}\"")),
                    },
                }
            }
            "text" => w.text = text(k, v)?,
            "text_size" => w.text_size = num(k, v)?.clamp(4.0, 96.0) as f32,
            "text_color" => w.text_color = parse_color(v)?,
            "bold" => w.bold = boolean(k, v)?,
            "hint" => w.hint = text(k, v)?,
            "hint_color" => w.hint_color = parse_color(v)?,
            "password" => {
                w.password = boolean(k, v)?;
                w.input = if w.password { InputKind::Password } else { InputKind::Text };
            }
            "input" => {
                let t = text(k, v)?;
                w.input = InputKind::from_key(&t).ok_or_else(|| format!("\"input\" is one of text, password, email, number, decimal, phone, multiline; not {t}"))?;
                w.password = w.input == InputKind::Password;
            }
            "checked" => w.checked = boolean(k, v)?,
            "video" | "movie" => w.video = text(k, v)?,
            "autoplay" | "auto_play" => w.autoplay = boolean(k, v)?,
            "loop" | "looping" => w.looping = boolean(k, v)?,
            "controls" | "show_controls" => w.controls = boolean(k, v)?,
            "progress" => w.progress = num(k, v)?.round().max(0.0) as i32,
            "max" => w.max = num(k, v)?.round().clamp(0.0, 100000.0) as i32,
            "enabled" => w.enabled = boolean(k, v)?,
            "visible" => w.visible = boolean(k, v)?,
            _ => return Err(format!("Unknown property \"{k}\"; the properties are {PROPS}")),
        }
    }
    let i = s.widgets.iter().position(|x| x.id == id).unwrap();
    s.widgets[i] = w;
    let mut id = id.to_string();
    if let Some(n) = rename {
        crate::designer::rename_widget(s, &id, &n);
        id = n;
    }
    Ok(id)
}

/// Add a widget and its children; returns the ids made, in order.
pub(crate) fn add_tree(s: &mut Screen, parent: &str, index: usize, spec: &Value, made: &mut Vec<String>) -> R<()> {
    let o = spec.as_object().ok_or("A widget is an object like {\"type\": \"button\", \"text\": \"OK\"}")?;
    let kind = parse_kind(o.get("type").and_then(|v| v.as_str()).ok_or("Each widget needs a \"type\"")?)?;
    if parent != "root" {
        match s.widget(parent) {
            Some(p) if p.kind.is_container() => {}
            Some(p) => return Err(format!("\"{parent}\" is a {}, which can't hold widgets; use a row or column", kind_name(p.kind))),
            None => return Err(format!("No layout \"{parent}\" on this screen")),
        }
    }
    let id = crate::designer::put_widget(s, &crate::designer::WDrag::New(kind), parent, index).ok_or("Couldn't add the widget")?;
    made.push(id.clone());
    let id = apply_props(s, &id, o)?;
    if let Some(i) = made.last_mut() {
        *i = id.clone();
    }
    if let Some(kids) = o.get("children") {
        let kids = kids.as_array().ok_or("\"children\" is a list of widgets")?;
        if !kind.is_container() && !kids.is_empty() {
            return Err(format!("A {} can't hold children", kind_name(kind)));
        }
        for k in kids {
            add_tree(s, &id, usize::MAX, k, made)?;
        }
    }
    Ok(())
}

pub(crate) fn var_type(t: &str) -> R<Result<VarType, ListType>> {
    Ok(match t.to_lowercase().replace([' ', '-'], "_").as_str() {
        "number" | "num" | "int" | "integer" | "double" => Ok(VarType::Num),
        "string" | "text" | "str" => Ok(VarType::Str),
        "boolean" | "bool" => Ok(VarType::Bool),
        "map" => Ok(VarType::Map),
        "list_number" | "list_num" => Err(ListType::Num),
        "list_string" | "list_str" | "list_text" => Err(ListType::Str),
        "list_map" => Err(ListType::Map),
        _ => return Err(format!("Unknown type \"{t}\"; use number, string, boolean, map, list_number, list_string or list_map")),
    })
}

pub(crate) fn comp_kind(t: &str) -> R<CompKind> {
    match t.to_lowercase().replace([' ', '-'], "_").as_str() {
        "timer" => Ok(CompKind::Timer),
        "popup" | "pop_up" | "dialog" => Ok(CompKind::Dialog),
        "navigation" | "intent" => Ok(CompKind::Intent),
        "saved_data" | "shared_pref" | "sharedpref" | "file" => Ok(CompKind::SharedPref),
        "vibration" | "vibrator" => Ok(CompKind::Vibrator),
        "date_time" | "calendar" | "date" => Ok(CompKind::Calendar),
        "web" | "web_request" | "http" | "api" => Ok(CompKind::Web),
        "sound" | "sound_player" | "player" | "audio" | "media_player" => Ok(CompKind::Sound),
        "camera" | "cam" | "photo" => Ok(CompKind::Camera),
        _ => Err(format!("Unknown component \"{t}\"; use timer, popup, navigation, saved_data, vibration, date_time, web, sound or camera")),
    }
}

// ----- blocks as JSON

fn shape_name(s: Shape) -> &'static str {
    match s {
        Shape::Stmt => "statement",
        Shape::Bool => "boolean reporter",
        Shape::Num => "number reporter",
        Shape::Str => "text reporter",
        Shape::C => "C block (\"do\")",
        Shape::E => "if/else block (\"do\", \"else\")",
        Shape::Cap => "cap (ends a stack)",
    }
}

fn menu_words(kind: &str) -> String {
    match kind {
        "view" => "widget id".into(),
        "textview" => "id of a widget with text".into(),
        "edittext" => "text field id".into(),
        "checkbox" => "checkbox or switch id".into(),
        "seekbar" => "slider or progress bar id".into(),
        "listspn" => "list or dropdown id".into(),
        "webview" => "web page id".into(),
        "varBool" => "boolean variable".into(),
        "varNum" => "number variable".into(),
        "varStr" => "string variable".into(),
        "varMap" => "map variable".into(),
        "listNum" => "number list".into(),
        "listStr" => "string list".into(),
        "listMap" => "map list".into(),
        "list" => "any list".into(),
        "timer" => "timer component".into(),
        "dialog" => "popup component".into(),
        "intent" => "navigation component".into(),
        "file" => "saved data component".into(),
        "vibrator" => "vibration component".into(),
        "calendar" => "date & time component".into(),
        "screen" => "screen activity, e.g. SecondActivity".into(),
        "visible" => "VISIBLE, INVISIBLE or GONE".into(),
        "color" => format!("one of {}", blocks::COLORS.iter().map(|c| c.0).collect::<Vec<_>>().join(", ")),
        k => k.into(),
    }
}

fn list_blocks(a: &Value) -> R<Value> {
    let want = arg_str(a, "category").map(|c| c.to_lowercase());
    if let Some(w) = &want {
        if !blocks::CATEGORIES.iter().any(|c| c.name.to_lowercase() == *w) {
            return Err(format!("No category \"{w}\"; they are {}", blocks::CATEGORIES.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")));
        }
    }
    let mut out = Map::new();
    for d in blocks::DEFS.iter().filter(|d| d.op != "moreBlock") {
        let cat = blocks::CATEGORIES[d.cat].name;
        if want.as_ref().is_some_and(|w| cat.to_lowercase() != *w) {
            continue;
        }
        let mut reads = vec![];
        let mut args = vec![];
        for t in blocks::parse_spec(d.spec) {
            match t {
                Token::Label(l) => reads.push(l),
                Token::ArgLabel(_) => {
                    reads.push("‹variable›".into());
                    args.push("variable name".to_string());
                }
                Token::Param(_, p) => {
                    let w = match p {
                        Param::Text => "text".to_string(),
                        Param::Num => "number".into(),
                        Param::Bool => "boolean".into(),
                        Param::Menu(m) => menu_words(&m),
                    };
                    reads.push(format!("‹{w}›"));
                    args.push(w);
                }
            }
        }
        let mut line = format!("{} — {}", reads.join(" "), shape_name(d.shape));
        if !d.spec2.is_empty() {
            line.push_str(&format!(", then \"{}\"", d.spec2));
        }
        if !args.is_empty() {
            line.push_str(&format!("; args: [{}]", args.join(", ")));
        }
        out.entry(cat.to_string()).or_insert_with(|| json!({})).as_object_mut().unwrap().insert(d.op.into(), json!(line));
    }
    Ok(json!({
        "how": "Each entry is op → what the block reads, its shape, and its args in order. Statements go in stacks; reporters (boolean/number/text) only inside another block's argument of the same kind. \
getVar's one arg is a variable's name, and it reports that variable's type. A boolean argument left empty (null) means false. \
moreBlock calls a more block (set_more_block makes them): its args are the block's name, then a value for each input; paramStr / paramNum / paramBool read an input inside the more block.",
        "blocks": out,
    }))
}

fn param_type_name(t: VarType) -> &'static str {
    match t {
        VarType::Num => "number",
        VarType::Bool => "boolean",
        _ => "text",
    }
}

fn block_json(b: &Block) -> Value {
    let args: Vec<Value> = b
        .args
        .iter()
        .map(|a| match a {
            Arg::Text(t) => json!(t),
            Arg::Block(c) => block_json(c),
        })
        .collect();
    let mut o = json!({"op": b.op, "args": args});
    if !b.sub1.is_empty() || blocks::def(&b.op).is_some_and(|d| matches!(d.shape, Shape::C | Shape::E)) {
        o["do"] = Value::Array(b.sub1.iter().map(block_json).collect());
    }
    if blocks::def(&b.op).is_some_and(|d| d.shape == Shape::E) {
        o["else"] = Value::Array(b.sub2.iter().map(block_json).collect());
    }
    o
}

fn accepts(p: &Param, s: Shape) -> bool {
    matches!((p, s), (Param::Text, Shape::Str) | (Param::Num, Shape::Num) | (Param::Bool, Shape::Bool))
}

/// A block from its JSON, checked the way the editor checks a drop.
fn parse_block(v: &Value, s: &Screen, screens: &crate::blocks::Choices, at: &str) -> R<Block> {
    let op = v.get("op").and_then(|o| o.as_str()).ok_or_else(|| format!("{at}: each block needs an \"op\""))?;
    let d = blocks::def(op).ok_or_else(|| format!("{at}: there's no block \"{op}\" (list_blocks shows them)"))?;
    let here = format!("{at} ({op})");
    // a more block call's arguments are its name, then its inputs
    let spec = if op == "moreBlock" {
        let name = v.get("args").and_then(|a| a.get(0)).and_then(|n| n.as_str()).unwrap_or("");
        let e = s.events.iter().find(|e| e.target == "moreBlock" && e.name == name).ok_or_else(|| format!("{here}: its first argument is a more block's name, and this screen has no more block \"{name}\" (set_more_block makes one)"))?;
        blocks::more_spec(&e.params)
    } else {
        d.spec.to_string()
    };
    let params: Vec<Option<Param>> = blocks::parse_spec(&spec)
        .into_iter()
        .filter_map(|t| match t {
            Token::Label(_) => None,
            Token::ArgLabel(_) => Some(None),
            Token::Param(_, p) => Some(Some(p)),
        })
        .collect();
    let empty = vec![];
    let given = match v.get("args") {
        None | Some(Value::Null) => &empty,
        Some(Value::Array(a)) => a,
        Some(_) => return Err(format!("{here}: \"args\" is a list")),
    };
    if given.len() != params.len() {
        return Err(format!("{here}: takes {} argument{} but got {}", params.len(), if params.len() == 1 { "" } else { "s" }, given.len()));
    }
    let mut args = vec![];
    for (i, (p, g)) in params.iter().zip(given).enumerate() {
        let here = format!("{here} argument {}", i + 1);
        let lit = |g: &Value| -> Option<String> {
            match g {
                Value::String(t) => Some(t.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                Value::Null => Some(String::new()),
                _ => None,
            }
        };
        args.push(match p {
            None => {
                let n = lit(g).unwrap_or_default();
                match op {
                    "moreBlock" => {}
                    "paramStr" | "paramNum" | "paramBool" => {
                        if n.trim().is_empty() {
                            return Err(format!("{here}: names one of the more block's inputs"));
                        }
                    }
                    _ => {
                        if !s.vars.iter().any(|v| v.name == n) {
                            return Err(format!("{here}: no variable \"{n}\" on this screen (add_variable adds one)"));
                        }
                    }
                }
                Arg::Text(n)
            }
            Some(Param::Menu(kind)) => {
                let n = lit(g).ok_or_else(|| format!("{here}: a drop-down takes a value ({}), not a block", menu_words(kind)))?;
                let choices = blocks::menu_choices(kind, s, screens);
                if !choices.contains(&n) {
                    return Err(if choices.is_empty() {
                        format!("{here}: needs a {} and this screen has none yet", menu_words(kind))
                    } else {
                        format!("{here}: \"{n}\" isn't a {}; it can be {}", menu_words(kind), choices.join(", "))
                    });
                }
                Arg::Text(n)
            }
            Some(p) if g.is_object() => {
                let b = parse_block(g, s, screens, &here)?;
                let sh = blocks::shape_of(&b, s);
                if !accepts(p, sh) {
                    let want = match p {
                        Param::Text => "a text reporter",
                        Param::Num => "a number reporter",
                        _ => "a boolean reporter",
                    };
                    return Err(format!("{here}: wants {want}, but {} is a {}", b.op, shape_name(sh)));
                }
                Arg::Block(Box::new(b))
            }
            Some(Param::Bool) => match g {
                Value::Null | Value::Bool(false) => Arg::Text(String::new()),
                Value::Bool(true) => Arg::Block(Box::new(Block { id: new_id(), op: "true".into(), args: vec![], sub1: vec![], sub2: vec![] })),
                _ => return Err(format!("{here}: wants a boolean reporter block (or true / false)")),
            },
            Some(Param::Num) => {
                let n = lit(g).unwrap_or_default();
                if n.trim().parse::<f64>().is_err() {
                    return Err(format!("{here}: wants a number, not {g}"));
                }
                Arg::Text(n.trim().to_string())
            }
            Some(_) => Arg::Text(lit(g).ok_or_else(|| format!("{here}: wants text, not {g}"))?),
        });
    }
    let mut b = Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] };
    match d.shape {
        Shape::C | Shape::E => {
            b.sub1 = parse_stack(v.get("do"), s, screens, &format!("{here} do"))?;
            if d.shape == Shape::E {
                b.sub2 = parse_stack(v.get("else"), s, screens, &format!("{here} else"))?;
            } else if v.get("else").is_some() {
                return Err(format!("{here}: only ifElse has an \"else\""));
            }
        }
        _ if v.get("do").is_some() || v.get("else").is_some() => return Err(format!("{here}: only C-shaped blocks (repeat, if, ...) take \"do\"")),
        _ => {}
    }
    Ok(b)
}

pub(crate) fn parse_stack(v: Option<&Value>, s: &Screen, screens: &crate::blocks::Choices, at: &str) -> R<Vec<Block>> {
    let list = match v {
        None | Some(Value::Null) => return Ok(vec![]),
        Some(Value::Array(a)) => a,
        Some(_) => return Err(format!("{at}: a stack is a list of blocks")),
    };
    let mut out = vec![];
    for (i, item) in list.iter().enumerate() {
        let here = format!("{at}[{}]", i + 1);
        let b = parse_block(item, s, screens, &here)?;
        let sh = blocks::shape_of(&b, s);
        if sh.is_reporter() {
            return Err(format!("{here} ({}): a {} goes inside another block's argument, not in a stack", b.op, shape_name(sh)));
        }
        if sh == Shape::Cap && i + 1 < list.len() {
            return Err(format!("{here} ({}): nothing can follow it, so it must be last", b.op));
        }
        out.push(b);
    }
    Ok(out)
}

fn opens_as(sh: Sheet) -> &'static str {
    match sh {
        Sheet::No => "page",
        Sheet::Half => "half_sheet",
        Sheet::Full => "full_sheet",
    }
}

fn app_bar_json(b: &AppBar) -> Value {
    let c = |c: u32| if c == 0 { "default".to_string() } else { color_json(c) };
    json!({"shown": !b.hidden, "bar_color": c(b.color), "title_color": c(b.title_color), "back_button": !b.no_back,
        "actions": b.actions.iter().map(|a| json!({"id": a.id, "icon": a.icon, "label": a.label})).collect::<Vec<_>>()})
}

fn screen_json(p: &Project, s: &Screen) -> Value {
    json!({
        "project": {"id": p.id, "app_name": p.app_name, "package": p.package},
        "screen": s.name, "activity": s.activity_name(),
        "title": s.title, "app_bar": app_bar_json(&s.app_bar), "opens_as": opens_as(p.sheet_of(s)),
        "other_screens": p.screens.iter().filter(|o| o.name != s.name).map(|o| o.name.clone()).collect::<Vec<_>>(),
        "widgets": s.children("root").into_iter().map(|i| widget_json(s, &s.widgets[i])).collect::<Vec<_>>(),
        "variables": s.vars.iter().map(|v| json!({"name": v.name, "type": v.ty.label().to_lowercase()})).collect::<Vec<_>>(),
        "lists": s.lists.iter().map(|l| json!({"name": l.name, "type": l.ty.label().to_lowercase().replace(' ', "_")})).collect::<Vec<_>>(),
        "components": s.components.iter().map(|c| json!({"name": c.name, "type": c.kind.label()})).collect::<Vec<_>>(),
        "events": s.events.iter().filter(|e| e.target != "moreBlock").map(|e| json!({
            "target": if e.target.is_empty() { "screen" } else { e.target.as_str() },
            "event": e.name, "reads": e.title(),
            "blocks": e.blocks.iter().map(block_json).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "more_blocks": s.events.iter().filter(|e| e.target == "moreBlock").map(|e| json!({
            "name": e.name,
            "inputs": e.params.iter().map(|p| json!({"name": p.name, "type": param_type_name(p.ty)})).collect::<Vec<_>>(),
            "blocks": e.blocks.iter().map(block_json).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// Runs a tool that only needs the projects. None: the tool needs the app
/// (opening, pictures, devices). Changes mark the project as edited.
pub fn store_tool(store: &mut Store, tool: &str, a: &Value) -> Option<R<Value>> {
    Some(match tool {
        "list_projects" => Ok(json!({"projects": store.projects.iter().map(|p| json!({
            "id": p.id, "app_name": p.app_name, "package": p.package, "version": p.ver_name,
            "screens": p.screens.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>()})),
        "create_project" => create_project(store, a),
        "list_blocks" => list_blocks(a),
        _ => {
            let p = match find_project(store, a) {
                Ok(p) => p,
                Err(e) => return Some(Err(e)),
            };
            let before = project_snapshot(p);
            let r = project_tool(p, tool, a)?;
            if r.is_ok() && project_snapshot(p) != before {
                p.modified = now_secs();
            }
            r
        }
    })
}

fn create_project(store: &mut Store, a: &Value) -> R<Value> {
    let name = need_str(a, "app_name")?.trim().to_string();
    if name.is_empty() {
        return Err("\"app_name\" can't be empty".into());
    }
    let slug: String = name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let slug = if slug.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) { slug } else { format!("app{slug}") };
    let package = arg_str(a, "package").map(|s| s.trim().to_string()).unwrap_or(format!("com.my.{slug}"));
    let parts: Vec<&str> = package.split('.').collect();
    let ok = parts.len() >= 2 && parts.iter().all(|s| s.chars().next().is_some_and(|f| f.is_ascii_alphabetic()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    if !ok {
        return Err(format!("\"{package}\" isn't a valid app id; use something like com.example.notes"));
    }
    let id = store.next_project;
    store.next_project += 1;
    let p = Project::new(id, &name, &name, &package);
    store.projects.push(p);
    Ok(json!({"project_id": id, "screens": ["main"], "next": "Add widgets to screen \"main\" with add_widget (parent \"root\"); a column with width and height match_parent is a good root."}))
}

fn project_tool(p: &mut Project, tool: &str, a: &Value) -> Option<R<Value>> {
    if tool == "link_supabase" {
        return Some((|| {
            let url = need_str(a, "url")?.trim();
            if url.is_empty() {
                p.supabase = None;
                return Ok(json!({"linked": false}));
            }
            let link = SupabaseLink { url: url.trim_end_matches('/').to_string(), key: arg_str(a, "key").unwrap_or("").trim().to_string() };
            if let Some(e) = link.problem() {
                return Err(e.to_string());
            }
            p.supabase = Some(link);
            Ok(json!({"linked": true, "url": url.trim_end_matches('/')}))
        })());
    }
    if tool == "add_file" {
        return Some((|| {
            let path = std::path::PathBuf::from(need_str(a, "path")?.trim());
            if !path.is_file() {
                return Err(format!("There's no file at {}", path.display()));
            }
            let name = p.add_file(&path)?;
            let kind = p.file(&name).map(|f| f.kind.one()).unwrap_or("file");
            Ok(json!({"name": name, "kind": kind}))
        })());
    }
    if tool == "list_files" {
        return Some(Ok(json!({"files": p.files.iter().map(|f| json!({"name": f.name, "kind": f.kind.one(), "file": f.file_name(), "used_by": p.file_uses(&f.name)})).collect::<Vec<_>>()})));
    }
    if tool == "rename_file" {
        return Some((|| {
            let name = p.rename_file(need_str(a, "name")?, need_str(a, "new_name")?)?;
            Ok(json!({"name": name}))
        })());
    }
    if tool == "delete_file" {
        return Some((|| {
            let n = need_str(a, "name")?;
            if p.file(n).is_none() {
                return Err(format!("There is no asset called {n}"));
            }
            let uses = p.file_uses(n);
            p.remove_file(n);
            Ok(json!({"deleted": n, "left_without_it": uses}))
        })());
    }
    if tool == "add_screen" {
        return Some((|| {
            let n = need_str(a, "name")?.trim().to_string();
            let ok = n.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !ok {
                return Err("A screen name starts with a small letter and has small letters, digits and _".into());
            }
            if p.screens.iter().any(|s| s.name == n || s.activity_name() == activity_name(&n)) {
                return Err(format!("There is already a screen \"{n}\""));
            }
            let made = match arg_str(a, "template").map(str::trim).filter(|t| !t.is_empty()) {
                Some(id) => {
                    let t = crate::templates::find(id).ok_or_else(|| {
                        format!("No template \"{id}\"; there are: {}", crate::templates::TEMPLATES.iter().map(|t| t.id).collect::<Vec<_>>().join(", "))
                    })?;
                    crate::templates::screen(t, &n, &crate::blocks::screen_choices(p))?
                }
                None => Screen::new(&n),
            };
            let widgets = made.widgets.len();
            p.screens.push(made);
            Ok(json!({"screen": n, "activity": activity_name(&n), "widgets": widgets}))
        })());
    }
    let si = match screen_index(p, a) {
        Ok(i) => i,
        Err(e) => return Some(Err(e)),
    };
    match tool {
        "get_screen" => return Some(Ok(screen_json(p, &p.screens[si]))),
        "get_code" => {
            let s = &p.screens[si];
            return Some(Ok(json!(match arg_str(a, "language").unwrap_or("") {
                "kotlin" => crate::native::kotlin(p, s),
                "swift" => crate::native::swift_screen(p, s),
                "layout_xml" | "xml" => crate::codegen::xml(s),
                l => return Some(Err(format!("\"language\" is kotlin, swift or layout_xml, not \"{l}\""))),
            })));
        }
        _ => {}
    }
    let screens = blocks::screen_choices(p);
    let s = &mut p.screens[si];
    let r = match tool {
        "add_widget" => (|| {
            let parent = arg_str(a, "parent").unwrap_or("root").to_string();
            let index = a.get("index").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(usize::MAX);
            let spec = a.get("widget").ok_or("\"widget\" is needed, e.g. {\"type\": \"button\", \"text\": \"OK\"}")?;
            let backup = s.clone();
            let mut made = vec![];
            if let Err(e) = add_tree(s, &parent, index, spec, &mut made) {
                *s = backup;
                return Err(format!("{e} (nothing was added)"));
            }
            Ok(json!({"added": made}))
        })(),
        "update_widget" => (|| {
            let id = need_str(a, "id")?;
            let set = a.get("set").and_then(|v| v.as_object()).ok_or("\"set\" is needed: an object of properties")?;
            if set.contains_key("type") {
                return Err("A widget's type can't change; delete it and add a new one".into());
            }
            let id = apply_props(s, id, set)?;
            Ok(widget_json(s, s.widget(&id).unwrap()))
        })(),
        "move_widget" => (|| {
            let id = need_str(a, "id")?;
            let parent = need_str(a, "parent")?;
            if s.widget(id).is_none() {
                return Err(format!("No widget \"{id}\""));
            }
            if parent != "root" && !s.widget(parent).is_some_and(|w| w.kind.is_container()) {
                return Err(format!("\"{parent}\" isn't a row or column on this screen"));
            }
            let index = a.get("index").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(usize::MAX);
            crate::designer::put_widget(s, &crate::designer::WDrag::Move(id.into()), parent, index).ok_or_else(|| "A layout can't go inside itself".to_string())?;
            Ok(json!({"moved": id, "parent": parent}))
        })(),
        "delete_widget" => (|| {
            let id = need_str(a, "id")?;
            if s.widget(id).is_none() {
                return Err(format!("No widget \"{id}\""));
            }
            let n = s.widgets.len();
            s.remove_widget(id);
            Ok(json!({"deleted": n - s.widgets.len()}))
        })(),
        "update_screen" => (|| {
            let bar_color = |k: &str| -> R<Option<u32>> {
                let Some(v) = a.get(k) else { return Ok(None) };
                if v.as_str().is_some_and(|t| t.trim().is_empty() || t.trim().eq_ignore_ascii_case("default")) {
                    return Ok(Some(0));
                }
                Ok(Some(parse_color(v)? | 0xFF00_0000))
            };
            let flag = |k: &str| -> R<Option<bool>> {
                a.get(k).map(|v| v.as_bool().ok_or_else(|| format!("\"{k}\" is true or false, not {v}"))).transpose()
            };
            let (color, title_color, show, back) = (bar_color("bar_color")?, bar_color("title_color")?, flag("app_bar")?, flag("back_button")?);
            if let Some(t) = a.get("title") {
                s.title = t.as_str().ok_or_else(|| format!("\"title\" is text, not {t}"))?.to_string();
            }
            if let Some(v) = a.get("opens_as") {
                let sheet = match v.as_str().unwrap_or("") {
                    "page" => Sheet::No,
                    "half_sheet" => Sheet::Half,
                    "full_sheet" => Sheet::Full,
                    _ => return Err(format!("\"opens_as\" is page, half_sheet or full_sheet, not {v}")),
                };
                if si == 0 && sheet != Sheet::No {
                    return Err("The first screen is where the app starts, so it can't be a bottom sheet".into());
                }
                s.sheet = sheet;
            }
            if let Some(list) = a.get("actions") {
                let list = list.as_array().ok_or("\"actions\" is a list of {\"icon\", \"label\", \"id\"}")?;
                let old = std::mem::take(&mut s.app_bar.actions);
                let mut made: Vec<BarAction> = vec![];
                for (i, item) in list.iter().enumerate() {
                    let back = |s: &mut Screen| s.app_bar.actions = old.clone();
                    let icon = item.get("icon").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                    if !BAR_ICONS.iter().any(|(n, _, _)| *n == icon) {
                        back(s);
                        return Err(format!("actions[{i}]: \"{icon}\" isn't an icon; they are {}", BAR_ICONS.iter().map(|(n, _, _)| *n).collect::<Vec<_>>().join(", ")));
                    }
                    let label = item.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    s.app_bar.actions = made.clone();
                    let id = match item.get("id").and_then(|v| v.as_str()).map(str::trim).filter(|t| !t.is_empty()) {
                        Some(id) => {
                            if let Some(e) = crate::logic::name_problem(s, id) {
                                back(s);
                                return Err(format!("actions[{i}]: can't call it \"{id}\": {e}"));
                            }
                            id.to_string()
                        }
                        None => s.next_action_id(&icon),
                    };
                    made.push(BarAction { id, icon, label });
                }
                // events of buttons that are gone go too
                s.events.retain(|e| !old.iter().any(|o| o.id == e.target) || made.iter().any(|m| m.id == e.target));
                s.app_bar.actions = made;
            }
            let b = &mut s.app_bar;
            if let Some(c) = color { b.color = c }
            if let Some(c) = title_color { b.title_color = c }
            if let Some(v) = show { b.hidden = !v }
            if let Some(v) = back { b.no_back = !v }
            Ok(json!({"title": s.title, "app_bar": app_bar_json(&s.app_bar), "opens_as": opens_as(s.sheet)}))
        })(),
        "add_variable" => (|| {
            let n = need_str(a, "name")?.trim().to_string();
            let t = var_type(need_str(a, "type")?)?;
            if let Some(e) = crate::logic::name_problem(s, &n) {
                return Err(format!("Can't call it \"{n}\": {e}"));
            }
            match t {
                Ok(ty) => s.vars.push(Var { name: n.clone(), ty }),
                Err(ty) => s.lists.push(ListVar { name: n.clone(), ty }),
            }
            Ok(json!({"added": n}))
        })(),
        "add_component" => (|| {
            let k = comp_kind(need_str(a, "type")?)?;
            let n = match arg_str(a, "name").map(str::trim).filter(|n| !n.is_empty()) {
                Some(n) => n.to_string(),
                None => {
                    let base = k.default_name();
                    let mut n = base.to_string();
                    let mut i = 2;
                    while crate::logic::name_problem(s, &n).is_some() {
                        n = format!("{base}{i}");
                        i += 1;
                    }
                    n
                }
            };
            if let Some(e) = crate::logic::name_problem(s, &n) {
                return Err(format!("Can't call it \"{n}\": {e}"));
            }
            s.components.push(Component { name: n.clone(), kind: k });
            Ok(json!({"added": n, "type": k.label()}))
        })(),
        "set_more_block" => (|| {
            let name = need_str(a, "name")?.trim().to_string();
            let mut inputs: Vec<(String, VarType, Option<usize>)> = vec![];
            for i in a.get("inputs").and_then(|v| v.as_array()).into_iter().flatten() {
                let n = i.get("name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                let ty = match i.get("type").and_then(|v| v.as_str()).unwrap_or("text").to_lowercase().as_str() {
                    "text" | "string" | "str" => VarType::Str,
                    "number" | "num" | "double" => VarType::Num,
                    "boolean" | "bool" | "true_or_false" => VarType::Bool,
                    t => return Err(format!("An input's type is text, number or boolean, not \"{t}\"")),
                };
                inputs.push((n, ty, None));
            }
            if let Some(p) = crate::logic::params_problem(&inputs) {
                return Err(p);
            }
            let vars: Vec<Var> = inputs.iter().map(|(n, ty, _)| Var { name: n.clone(), ty: *ty }).collect();
            match s.events.iter().find(|e| e.target == "moreBlock" && e.name == name) {
                Some(e) => {
                    // an input keeps its place in uses when its name stays
                    let from: Vec<Option<usize>> = vars.iter().map(|v| e.params.iter().position(|p| p.name == v.name)).collect();
                    crate::logic::apply_more_edit(s, &name, &name, vars, &from);
                }
                None => {
                    if let Some(p) = crate::logic::name_problem(s, &name) {
                        return Err(format!("Can't call it \"{name}\": {p}"));
                    }
                    s.events.push(Event { target: "moreBlock".into(), name: name.clone(), blocks: vec![], loose: vec![], params: vars });
                }
            }
            let list = a.get("blocks").ok_or("\"blocks\" is needed (a list)")?;
            let blocks = parse_stack(Some(list), s, &screens, "blocks")?;
            let lines: Vec<String> = blocks.iter().map(blocks::describe).collect();
            let e = s.events.iter_mut().find(|e| e.target == "moreBlock" && e.name == name).unwrap();
            e.blocks = blocks;
            Ok(json!({"more_block": e.title(), "blocks": lines}))
        })(),
        "set_event_blocks" => (|| {
            let target = need_str(a, "target")?.trim();
            let event = need_str(a, "event")?.trim();
            let target = if target.is_empty() || target.eq_ignore_ascii_case("screen") || target.eq_ignore_ascii_case("activity") { "" } else { target };
            if target.is_empty() {
                if !matches!(event, "onCreate" | "onBackPressed") {
                    return Err(format!("The screen's events are onCreate and onBackPressed, not {event}"));
                }
            } else {
                if s.action(target).is_some() {
                    if event != "onClick" {
                        return Err(format!("An app bar button's event is onClick, not {event}"));
                    }
                } else if let Some(w) = s.widget(target) {
                    if !w.kind.events().contains(&event) {
                    let has = w.kind.events();
                    return Err(if has.is_empty() { format!("A {} has no events", kind_name(w.kind)) } else { format!("A {}'s events are {}", kind_name(w.kind), has.join(", ")) });
                    }
                } else {
                    return Err(format!("No widget or app bar button \"{target}\" on this screen"));
                }
            }
            let list = a.get("blocks").ok_or("\"blocks\" is needed (a list; [] empties the event)")?;
            let blocks = parse_stack(Some(list), s, &screens, "blocks")?;
            let lines: Vec<String> = blocks.iter().map(blocks::describe).collect();
            match s.events.iter_mut().find(|e| e.target == target && e.name == event) {
                Some(e) => e.blocks = blocks,
                None => s.events.push(Event { target: target.into(), name: event.into(), blocks, loose: vec![], params: vec![] }),
            }
            let e = s.events.iter().find(|e| e.target == target && e.name == event).unwrap();
            Ok(json!({"event": e.title(), "blocks": lines}))
        })(),
        _ => return None,
    };
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store { projects: vec![crate::app::sample_project(601)], next_project: 602, ..Default::default() }
    }

    fn call(st: &mut Store, tool: &str, a: Value) -> R<Value> {
        store_tool(st, tool, &a).expect("a store tool")
    }

    #[test]
    fn every_tool_has_a_schema() {
        let t = tools();
        assert_eq!(t.as_array().unwrap().len(), TOOLS.len());
        for x in t.as_array().unwrap() {
            assert_eq!(x["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn more_blocks_and_new_widget_looks() {
        let mut st = store();
        let id = 601;
        // a more block with inputs, then a use of it
        let r = call(&mut st, "set_more_block", json!({"project_id": id, "name": "price", "inputs": [{"name": "amount", "type": "number"}, {"name": "label", "type": "text"}],
            "blocks": [{"op": "setText", "args": ["textview1", {"op": "stringJoin", "args": [{"op": "paramStr", "args": ["label"]}, {"op": "toStringFixed", "args": [{"op": "paramNum", "args": ["amount"]}, "2"]}]}]}]})).unwrap();
        assert_eq!(r["more_block"], "define price (amount) (label)");
        call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "button1", "event": "onClick", "blocks": [{"op": "moreBlock", "args": ["price", "4.5", "Total: "]}]})).unwrap();
        let e = call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "button1", "event": "onClick", "blocks": [{"op": "moreBlock", "args": ["price", "4.5"]}]})).unwrap_err();
        assert!(e.contains("takes 3 arguments"), "{e}");
        let e = call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "button1", "event": "onClick", "blocks": [{"op": "moreBlock", "args": ["nope"]}]})).unwrap_err();
        assert!(e.contains("no more block \"nope\""), "{e}");
        let scr = call(&mut st, "get_screen", json!({"project_id": id})).unwrap();
        assert_eq!(scr["more_blocks"][0]["inputs"][1]["type"], "text");
        let kt = call(&mut st, "get_code", json!({"project_id": id, "language": "kotlin"})).unwrap();
        assert!(kt.as_str().unwrap().contains("_price(4.5, \"Total: \")"));
        // changing its inputs keeps the uses' values by name
        call(&mut st, "set_more_block", json!({"project_id": id, "name": "price", "inputs": [{"name": "label", "type": "text"}], "blocks": []})).unwrap();
        let ev = st.projects[0].screens[0].events.iter().find(|e| e.target == "button1" && e.name == "onClick").unwrap();
        assert_eq!(ev.blocks[0].args, vec![Arg::Text("price".into()), Arg::Text("Total: ".into())]);
        // corners, shadow, border; a sound player
        call(&mut st, "update_widget", json!({"project_id": id, "id": "button1", "set": {"corner_radius": 12, "shadow": 4, "border": 2, "border_color": "#FF0000"}})).unwrap();
        let w = st.projects[0].screens[0].widget("button1").unwrap().clone();
        assert_eq!((w.radius, w.elevation, w.border, w.border_color), (12, 4, 2, 0xFFFF_0000));
        let kt = call(&mut st, "get_code", json!({"project_id": id, "language": "kotlin"})).unwrap();
        let kt = kt.as_str().unwrap();
        assert!(kt.contains("AppUtil.setRadius(button1, 12.0)") && kt.contains("AppUtil.setElevation(button1, 4.0)") && kt.contains("AppUtil.setBorder(button1, 2.0, 0xFFFF0000.toInt())"), "{kt}");
        call(&mut st, "add_component", json!({"project_id": id, "type": "sound"})).unwrap();
        assert!(st.projects[0].screens[0].components.iter().any(|c| c.name == "player" && c.kind == CompKind::Sound));
    }

    #[test]
    fn assets_add_rename_delete() {
        let mut st = store();
        st.projects[0].id = 990_003;
        let id = 990_003;
        let _ = std::fs::remove_dir_all(st.projects[0].files_dir());
        let src = std::env::temp_dir().join("blockwise-asset-test");
        let _ = std::fs::create_dir_all(&src);
        std::fs::write(src.join("Word List.txt"), "\u{FEFF}apple,pear").unwrap();
        std::fs::write(src.join("bad.ttf"), "not a font").unwrap();
        std::fs::write(src.join("notes.doc"), "x").unwrap();
        let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/lucide.ttf");
        let r = call(&mut st, "add_file", json!({"project_id": id, "path": src.join("Word List.txt").display().to_string()})).unwrap();
        assert_eq!((r["name"].as_str().unwrap(), r["kind"].as_str().unwrap()), ("word_list", "data file"));
        let r = call(&mut st, "add_file", json!({"project_id": id, "path": font.display().to_string()})).unwrap();
        assert_eq!(r["name"], "lucide");
        let e = call(&mut st, "add_file", json!({"project_id": id, "path": src.join("bad.ttf").display().to_string()})).unwrap_err();
        assert!(e.contains("isn't a font"), "{e}");
        let e = call(&mut st, "add_file", json!({"project_id": id, "path": src.join("notes.doc").display().to_string()})).unwrap_err();
        assert!(e.contains("can't be added"), "{e}");
        // a text using the font, and blocks using both
        call(&mut st, "update_widget", json!({"project_id": id, "id": "textview1", "set": {"font": "lucide"}})).unwrap();
        call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "button1", "event": "onClick", "blocks": [
            {"op": "setFont", "args": ["textview2", "lucide"]},
            {"op": "setText", "args": ["textview2", {"op": "assetText", "args": ["word_list"]}]}]})).unwrap();
        let l = call(&mut st, "list_files", json!({"project_id": id})).unwrap();
        assert_eq!(l["files"][1]["used_by"].as_array().unwrap().len(), 2, "{l}");
        let kt = call(&mut st, "get_code", json!({"project_id": id, "language": "kotlin"})).unwrap();
        let kt = kt.as_str().unwrap();
        assert!(kt.contains("AppUtil.setFont(textview1, \"lucide\")") && kt.contains("AppUtil.setFont(textview2, \"lucide\")") && kt.contains("AppUtil.assetText(_activity, \"word_list\")"), "{kt}");
        let sw = call(&mut st, "get_code", json!({"project_id": id, "language": "swift"})).unwrap();
        let sw = sw.as_str().unwrap();
        assert!(sw.contains("font: \"lucide\"") && sw.contains("textview2.font = \"lucide\"") && sw.contains("Helpers.assetText(\"word_list\")"), "{sw}");
        // the preview reads the data file as the phones do (no BOM)
        let mut rt = crate::runtime::Runtime::new(st.projects[0].clone(), 0.0);
        rt.fire("button1", "onClick", Default::default());
        assert_eq!(rt.widgets["textview2"].text, "apple,pear");
        assert_eq!(rt.widgets["textview2"].font, "lucide");
        // renaming carries every use along; the name is made fit for code
        let r = call(&mut st, "rename_file", json!({"project_id": id, "name": "lucide", "new_name": "Icons Font"})).unwrap();
        assert_eq!(r["name"], "icons_font");
        let p = &st.projects[0];
        assert_eq!(p.screens[0].widget("textview1").unwrap().font, "icons_font");
        assert_eq!(p.file_uses("icons_font").len(), 2);
        assert!(p.file_path(p.file("icons_font").unwrap()).exists());
        let e = call(&mut st, "rename_file", json!({"project_id": id, "name": "icons_font", "new_name": "word_list"})).unwrap_err();
        assert!(e.contains("already"), "{e}");
        // deleting leaves the users without it, and closing cleans the folder
        let r = call(&mut st, "delete_file", json!({"project_id": id, "name": "icons_font"})).unwrap();
        assert_eq!(r["left_without_it"].as_array().unwrap().len(), 2);
        let p = &st.projects[0];
        assert_eq!(p.screens[0].widget("textview1").unwrap().font, "");
        assert!(p.file_uses("word_list").len() == 1 && p.files.len() == 1);
        p.clean_files();
        let left: Vec<String> = std::fs::read_dir(p.files_dir()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(left, vec!["word_list.txt".to_string()]);
        let _ = std::fs::remove_dir_all(p.files_dir());
        let _ = std::fs::remove_dir_all(&src);
    }

    #[test]
    fn video_widget_and_video_assets() {
        let mut st = store();
        st.projects[0].id = 990_006;
        let id = 990_006;
        let _ = std::fs::remove_dir_all(st.projects[0].files_dir());
        let src = std::env::temp_dir().join("blockwise-video-test");
        let _ = std::fs::create_dir_all(&src);
        // the smallest MP4 the length reader needs: ftyp, then moov › mvhd (v0, 1000 a second, 6000)
        let mut mp4 = vec![0, 0, 0, 16, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm', 0, 0, 0, 0];
        let mut mvhd = vec![0u8; 8 + 100];
        mvhd[..8].copy_from_slice(&[0, 0, 0, 108, b'm', b'v', b'h', b'd']);
        mvhd[20..24].copy_from_slice(&1000u32.to_be_bytes());
        mvhd[24..28].copy_from_slice(&6000u32.to_be_bytes());
        mp4.extend_from_slice(&[0, 0, 0, 116, b'm', b'o', b'o', b'v']);
        mp4.extend_from_slice(&mvhd);
        std::fs::write(src.join("Intro Clip.mp4"), &mp4).unwrap();
        assert_eq!(crate::model::video_duration_ms(&src.join("Intro Clip.mp4")), Some(6000.0));
        let r = call(&mut st, "add_file", json!({"project_id": id, "path": src.join("Intro Clip.mp4").display().to_string()})).unwrap();
        assert_eq!((r["name"].as_str().unwrap(), r["kind"].as_str().unwrap()), ("intro_clip", "video"));
        call(&mut st, "add_widget", json!({"project_id": id, "parent": "linear1", "widget": {"type": "video", "id": "movie", "video": "intro_clip", "autoplay": true, "loop": false, "controls": false}})).unwrap();
        call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "button1", "event": "onClick", "blocks": [
            {"op": "videoSeek", "args": ["movie", "2500"]},
            {"op": "setText", "args": ["textview2", {"op": "toString", "args": [{"op": "videoDuration", "args": ["movie"]}]}]}]})).unwrap();
        call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "movie", "event": "onCompletion", "blocks": [{"op": "videoPlay", "args": ["movie"]}]})).unwrap();
        let w = st.projects[0].screens[0].widget("movie").unwrap().clone();
        assert!(w.autoplay && !w.controls && w.video == "intro_clip");
        assert_eq!(st.projects[0].file_uses("intro_clip").len(), 1);
        let kt = call(&mut st, "get_code", json!({"project_id": id, "language": "kotlin"})).unwrap();
        let kt = kt.as_str().unwrap();
        for want in ["movie_video = VideoControl(movie)", "movie_video.setControls(false)", "movie_video.load(\"intro_clip\")", "movie_video.play()", "movie_video.seek(2500", "movie_video.onEnd = {"] {
            assert!(kt.contains(want), "{want}\n{kt}");
        }
        let xml = call(&mut st, "get_code", json!({"project_id": id, "language": "layout_xml"})).unwrap();
        assert!(xml.as_str().unwrap().contains("<VideoView"));
        let sw = call(&mut st, "get_code", json!({"project_id": id, "language": "swift"})).unwrap();
        let sw = sw.as_str().unwrap();
        for want in ["let movie_video = VideoControl()", "movie_video.onEnd = { [weak self] in self?.movie_onCompletion() }", "movie_video.load(\"intro_clip\")", "VideoBox(control: m.movie_video, controls: false)"] {
            assert!(sw.contains(want), "{want}\n{sw}");
        }
        // the preview: starts playing by itself, knows the length, goes where it's told
        let mut rt = crate::runtime::Runtime::new(st.projects[0].clone(), 0.0);
        assert!(rt.widgets["movie"].playing);
        rt.fire("button1", "onClick", Default::default());
        assert_eq!(rt.widgets["textview2"].text, "6000");
        assert_eq!(rt.widgets["movie"].video_pos, 2500.0);
        // renaming and deleting reach the widget
        call(&mut st, "rename_file", json!({"project_id": id, "name": "intro_clip", "new_name": "intro"})).unwrap();
        assert_eq!(st.projects[0].screens[0].widget("movie").unwrap().video, "intro");
        call(&mut st, "delete_file", json!({"project_id": id, "name": "intro"})).unwrap();
        assert_eq!(st.projects[0].screens[0].widget("movie").unwrap().video, "");
        let _ = std::fs::remove_dir_all(st.projects[0].files_dir());
        let _ = std::fs::remove_dir_all(&src);
    }

    #[test]
    fn templates_and_web_requests() {
        let mut st = store();
        let r = call(&mut st, "create_project", json!({"app_name": "Shop"})).unwrap();
        let id = r["project_id"].as_u64().unwrap();
        let r = call(&mut st, "add_screen", json!({"project_id": id, "name": "login", "template": "login"})).unwrap();
        assert!(r["widgets"].as_u64().unwrap() > 3);
        let e = call(&mut st, "add_screen", json!({"project_id": id, "name": "other", "template": "nope"})).unwrap_err();
        assert!(e.contains("login") && e.contains("settings"), "{e}");
        let e = call(&mut st, "link_supabase", json!({"project_id": id, "url": "https://ab.supabase.co", "key": "sb_secret_x"})).unwrap_err();
        assert!(e.contains("secret"), "{e}");
        call(&mut st, "link_supabase", json!({"project_id": id, "url": "https://ab.supabase.co/", "key": "sb_publishable_x"})).unwrap();
        call(&mut st, "add_screen", json!({"project_id": id, "name": "signin", "template": "supabase_login"})).unwrap();
        let p = st.projects.iter().find(|p| p.id as u64 == id).unwrap();
        assert_eq!(p.supabase.as_ref().unwrap().base(), "https://ab.supabase.co");
        let kt = crate::native::kotlin_util(p);
        assert!(kt.contains("const val URL = \"https://ab.supabase.co\"") && kt.contains("const val KEY = \"sb_publishable_x\""));
        assert!(crate::native::swift_helpers(p).contains("static let url = \"https://ab.supabase.co\""));
        let r = call(&mut st, "add_component", json!({"project_id": id, "type": "web"})).unwrap();
        assert_eq!(r["added"], "web");
        call(&mut st, "add_variable", json!({"project_id": id, "name": "user", "type": "map"})).unwrap();
        call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "screen", "event": "onCreate", "blocks": [
            {"op": "webSetHeader", "args": ["web", "apikey", "abc"]},
            {"op": "webRequest", "args": ["web", "GET", "https://example.com/u", ""],
             "do": [{"op": "mapFromJson", "args": ["user", {"op": "webResponse"}]}],
             "else": [{"op": "doToast", "args": [{"op": "toString", "args": [{"op": "webStatus"}]}]}]},
        ]})).unwrap();
        let code = call(&mut st, "get_code", json!({"project_id": id, "language": "kotlin"})).unwrap().to_string();
        assert!(code.contains("AppUtil.web(_activity, web, \\\"GET\\\""), "{code}");
        assert!(code.contains("AppUtil.jsonToMap(_response)"), "{code}");
        let code = call(&mut st, "get_code", json!({"project_id": id, "language": "swift"})).unwrap().to_string();
        assert!(code.contains("Helpers.web(web, \\\"GET\\\""), "{code}");
        assert!(code.contains("Helpers.jsonToMap(_response)"), "{code}");
    }

    #[test]
    fn build_a_screen_and_program_it() {
        let mut st = store();
        let r = call(&mut st, "create_project", json!({"app_name": "Notes 2"})).unwrap();
        let id = r["project_id"].as_u64().unwrap();
        let r = call(&mut st, "add_widget", json!({"project_id": id, "widget": {
            "type": "column", "width": "match_parent", "height": "match_parent", "gravity": "center",
            "children": [
                {"type": "text", "id": "title", "text": "Hello", "text_size": 24, "text_color": "#5B5BD6", "bold": true},
                {"type": "row", "children": [{"type": "button", "id": "go", "text": "Go", "weight": 1}]},
            ],
        }})).unwrap();
        assert_eq!(r["added"], json!(["linear1", "title", "linear2", "go"]));
        let p = st.projects.iter().find(|p| p.id as u64 == id).unwrap();
        assert_eq!(p.package, "com.my.notes2");
        let s = &p.screens[0];
        assert_eq!(s.widget("title").unwrap().parent, "linear1");
        assert_eq!(s.widget("title").unwrap().text_color, 0xFF5B_5BD6);
        assert_eq!(s.widget("go").unwrap().weight, 1.0);

        call(&mut st, "add_variable", json!({"project_id": id, "name": "taps", "type": "number"})).unwrap();
        let r = call(&mut st, "set_event_blocks", json!({"project_id": id, "target": "go", "event": "onClick", "blocks": [
            {"op": "increaseInt", "args": ["taps"]},
            {"op": "setText", "args": ["title", {"op": "toString", "args": [{"op": "getVar", "args": ["taps"]}]}]},
            {"op": "if", "args": [{"op": ">", "args": [{"op": "getVar", "args": ["taps"]}, 9]}], "do": [{"op": "doToast", "args": ["Ten!"]}]},
        ]}));
        let r = r.unwrap();
        assert_eq!(r["blocks"].as_array().unwrap().len(), 3);
        // read back in the same form
        let back = call(&mut st, "get_screen", json!({"project_id": id})).unwrap();
        let ev = back["events"].as_array().unwrap().iter().find(|e| e["target"] == "go").unwrap();
        assert_eq!(ev["blocks"][2]["do"][0]["op"], "doToast");
        assert_eq!(back["widgets"][0]["children"][1]["children"][0]["id"], "go");
    }

    #[test]
    fn wrong_blocks_say_what_is_wrong() {
        let mut st = store();
        let e = |st: &mut Store, b: Value| call(st, "set_event_blocks", json!({"project_id": 601, "target": "button1", "event": "onClick", "blocks": b})).unwrap_err();
        assert!(e(&mut st, json!([{"op": "nope"}])).contains("there's no block \"nope\""));
        assert!(e(&mut st, json!([{"op": "setText", "args": ["textview1"]}])).contains("takes 2 arguments"));
        assert!(e(&mut st, json!([{"op": "setText", "args": ["nowhere", "x"]}])).contains("it can be"));
        // a number reporter in a text slot
        assert!(e(&mut st, json!([{"op": "setText", "args": ["textview1", {"op": "getVar", "args": ["count"]}]}])).contains("wants a text reporter"));
        assert!(e(&mut st, json!([{"op": "getText", "args": ["textview1"]}])).contains("not in a stack"));
        // nothing changed
        let s = &st.projects[0].screens[0];
        assert!(s.events.iter().find(|e| e.target == "button1").unwrap().blocks.len() > 1);
    }

    #[test]
    fn screen_title_and_app_bar() {
        let mut st = store();
        let r = call(&mut st, "update_screen", json!({"project_id": 601, "title": "Notes", "bar_color": "#FFFFFF", "back_button": false})).unwrap();
        assert_eq!(r["app_bar"], json!({"shown": true, "bar_color": "#FFFFFF", "title_color": "default", "back_button": false, "actions": []}));
        let b = &st.projects[0].screens[0].app_bar;
        assert_eq!((b.color, b.no_back), (0xFFFF_FFFF, true));
        // a white bar gets a dark title
        assert_eq!(b.title(false), 0xFF11_1111);
        let g = call(&mut st, "get_screen", json!({"project_id": 601})).unwrap();
        assert_eq!((g["title"].as_str(), g["app_bar"]["bar_color"].as_str()), (Some("Notes"), Some("#FFFFFF")));
        call(&mut st, "update_screen", json!({"project_id": 601, "bar_color": "default", "app_bar": false})).unwrap();
        let b = &st.projects[0].screens[0].app_bar;
        assert_eq!((b.color, b.hidden), (0, true));
        assert!(call(&mut st, "update_screen", json!({"project_id": 601, "title_color": "red"})).is_err());
    }

    #[test]
    fn app_bar_buttons_and_their_events() {
        let mut st = store();
        let r = call(&mut st, "update_screen", json!({"project_id": 601, "actions": [{"icon": "search", "label": "Search"}, {"icon": "add", "id": "newNote"}]})).unwrap();
        assert_eq!(r["app_bar"]["actions"][0]["id"], "searchButton");
        assert_eq!(r["app_bar"]["actions"][1]["id"], "newNote");
        call(&mut st, "set_event_blocks", json!({"project_id": 601, "target": "newNote", "event": "onClick", "blocks": [{"op": "doToast", "args": ["New"]}]})).unwrap();
        let e = call(&mut st, "set_event_blocks", json!({"project_id": 601, "target": "newNote", "event": "onLongClick", "blocks": []})).unwrap_err();
        assert!(e.contains("onClick"), "{e}");
        // bad icons and taken names change nothing
        assert!(call(&mut st, "update_screen", json!({"project_id": 601, "actions": [{"icon": "rocket"}]})).unwrap_err().contains("isn't an icon"));
        assert!(call(&mut st, "update_screen", json!({"project_id": 601, "actions": [{"icon": "add", "id": "button1"}]})).unwrap_err().contains("already used"));
        assert_eq!(st.projects[0].screens[0].app_bar.actions.len(), 2);
        // keeping a button by id keeps its event; dropping one drops it
        call(&mut st, "update_screen", json!({"project_id": 601, "actions": [{"icon": "add", "id": "newNote"}]})).unwrap();
        assert!(st.projects[0].screens[0].events.iter().any(|e| e.target == "newNote"));
        call(&mut st, "update_screen", json!({"project_id": 601, "actions": []})).unwrap();
        assert!(!st.projects[0].screens[0].events.iter().any(|e| e.target == "newNote"));
    }

    #[test]
    fn screens_open_as_sheets() {
        let mut st = store();
        call(&mut st, "add_screen", json!({"project_id": 601, "name": "picker"})).unwrap();
        let r = call(&mut st, "update_screen", json!({"project_id": 601, "screen": "picker", "opens_as": "half_sheet"})).unwrap();
        assert_eq!(r["opens_as"], "half_sheet");
        assert_eq!(call(&mut st, "get_screen", json!({"project_id": 601, "screen": "picker"})).unwrap()["opens_as"], "half_sheet");
        assert!(call(&mut st, "update_screen", json!({"project_id": 601, "opens_as": "full_sheet"})).unwrap_err().contains("first screen"));
        assert!(call(&mut st, "update_screen", json!({"project_id": 601, "screen": "picker", "opens_as": "popup"})).is_err());
    }

    #[test]
    fn a_failed_tree_adds_nothing() {
        let mut st = store();
        let n = st.projects[0].screens[0].widgets.len();
        let e = call(&mut st, "add_widget", json!({"project_id": 601, "widget": {"type": "column", "children": [{"type": "button", "colour": "red"}]}})).unwrap_err();
        assert!(e.contains("Unknown property \"colour\""), "{e}");
        assert_eq!(st.projects[0].screens[0].widgets.len(), n);
    }

    #[test]
    fn rename_and_colours() {
        let mut st = store();
        let r = call(&mut st, "update_widget", json!({"project_id": 601, "id": "button1", "set": {"id": "addButton", "background": "transparent", "width": "120dp"}})).unwrap();
        assert_eq!(r["id"], "addButton");
        assert_eq!(r["width"], 120);
        let s = &st.projects[0].screens[0];
        // the click event follows the new name
        assert!(s.events.iter().any(|e| e.target == "addButton"));
        assert_eq!(parse_color(&json!("#abc")).unwrap(), 0xFFAA_BBCC);
        assert_eq!(color_json(0x80FF_0000), "#80FF0000");
    }

    #[test]
    fn root_is_not_a_widget_name() {
        let mut st = store();
        let e = call(&mut st, "add_widget", json!({"project_id": 601, "widget": {"type": "column", "id": "root"}})).unwrap_err();
        assert!(e.contains("the screen itself"), "{e}");
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn http_round_trip() {
        let ctx = egui::Context::default();
        let port = 47_000 + (std::process::id() % 2000) as u16;
        let server = Server::start(port, ctx).unwrap();
        // answer tool calls the way the app does
        let worker = std::thread::spawn(move || {
            let mut st = store();
            for _ in 0..200 {
                for c in server.poll() {
                    let r = store_tool(&mut st, &c.tool, &c.args).unwrap();
                    let _ = c.reply.send(match r {
                        Ok(v) => Reply::Json(v),
                        Err(e) => Reply::Error(e),
                    });
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let init = post(port, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#).unwrap().unwrap();
        let init: Value = serde_json::from_str(&init).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "blockwise");
        assert_eq!(post(port, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap(), None);
        let list: Value = serde_json::from_str(&post(port, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap().unwrap()).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() > 10);
        let call: Value = serde_json::from_str(&post(port, r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_projects","arguments":{}}}"#).unwrap().unwrap()).unwrap();
        assert!(call["result"]["content"][0]["text"].as_str().unwrap().contains("Tap Counter"));
        let bad: Value = serde_json::from_str(&post(port, r#"{"jsonrpc":"2.0","id":4,"method":"nope"}"#).unwrap().unwrap()).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
        assert!(!origin_ok(Some("https://evil.example")));
        assert!(origin_ok(Some("http://localhost:6274")));
        worker.join().unwrap();
    }
}
