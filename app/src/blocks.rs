//! Block catalogue: categories, colours, shapes and labels (the Kotlin and Swift each block becomes are in native.rs).

use crate::model::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// " " plain statement
    Stmt,
    /// "b" boolean reporter
    Bool,
    /// "d" number reporter
    Num,
    /// "s" string reporter
    Str,
    /// "c" one inner stack
    C,
    /// "e" if / else
    E,
    /// "f" nothing attaches below
    Cap,
}

impl Shape {
    pub fn is_reporter(self) -> bool {
        matches!(self, Shape::Bool | Shape::Num | Shape::Str)
    }
}

pub struct Category {
    pub name: &'static str,
    pub color: u32,
}

/// Index, name and colour of each category.
pub const CATEGORIES: [Category; 10] = [
    Category { name: "Variable", color: 0xEE7D16 },
    Category { name: "List", color: 0xCC5B22 },
    Category { name: "Control", color: 0xE1A92A },
    Category { name: "Operator", color: 0x5CB722 },
    Category { name: "Math", color: 0x23B9A9 },
    Category { name: "Saved data", color: 0xA1887F },
    Category { name: "View", color: 0x4A6CD4 },
    Category { name: "Component", color: 0x2CA5E2 },
    Category { name: "More Block", color: 0x8A55D7 },
    Category { name: "Supabase", color: 0x249E6B },
];

/// Event (hat) colour.
pub const EVENT_COLOR: u32 = 0x6A7B8C;
/// Palette section header colour 0xFF555555.
pub const HEADER_COLOR: u32 = 0x555555;

pub struct BlockDef {
    pub op: &'static str,
    pub cat: usize,
    pub shape: Shape,
    /// Words and parameters: %s text, %d number, %b boolean, %m.<kind> drop-down.
    pub spec: &'static str,
    /// Second label row for if/else ("else").
    pub spec2: &'static str,
}

macro_rules! b {
    ($op:expr, $cat:expr, $shape:ident, $spec:expr) => {
        BlockDef { op: $op, cat: $cat, shape: Shape::$shape, spec: $spec, spec2: "" }
    };
    ($op:expr, $cat:expr, $shape:ident, $spec:expr, $spec2:expr) => {
        BlockDef { op: $op, cat: $cat, shape: Shape::$shape, spec: $spec, spec2: $spec2 }
    };
}

pub static DEFS: &[BlockDef] = &[
    // ---- Variable (0)
    b!("setVarBoolean", 0, Stmt, "set %m.varBool to %b"),
    b!("setVarInt", 0, Stmt, "set %m.varNum to %d"),
    b!("setVarString", 0, Stmt, "set %m.varStr to %s"),
    b!("increaseInt", 0, Stmt, "increase %m.varNum by 1"),
    b!("decreaseInt", 0, Stmt, "decrease %m.varNum by 1"),
    b!("mapCreateNew", 0, Stmt, "%m.varMap start as empty map"),
    b!("mapPut", 0, Stmt, "%m.varMap put key %s value %s"),
    b!("mapGet", 0, Str, "%m.varMap get key %s"),
    b!("mapContainKey", 0, Bool, "%m.varMap contain key %s"),
    b!("mapRemoveKey", 0, Stmt, "%m.varMap remove key %s"),
    b!("mapSize", 0, Num, "%m.varMap size"),
    b!("mapClear", 0, Stmt, "%m.varMap clear"),
    b!("mapIsEmpty", 0, Bool, "%m.varMap is empty"),
    // dynamic reporters
    b!("getVar", 0, Num, "@0"),
    b!("eventPosition", 0, Num, "item position"),
    b!("eventChecked", 0, Bool, "is checked"),
    b!("eventText", 0, Str, "typed text"),
    b!("eventProgress", 0, Num, "slider value"),
    // ---- List (1)
    b!("addListInt", 1, Stmt, "add %d to %m.listNum"),
    b!("insertListInt", 1, Stmt, "insert %d at %d to %m.listNum"),
    b!("getAtListInt", 1, Num, "get at %d of %m.listNum"),
    b!("indexListInt", 1, Num, "index %d in %m.listNum"),
    b!("containListInt", 1, Bool, "%m.listNum contains %d"),
    b!("addListStr", 1, Stmt, "add %s to %m.listStr"),
    b!("insertListStr", 1, Stmt, "insert %s at %d to %m.listStr"),
    b!("getAtListStr", 1, Str, "get at %d of %m.listStr"),
    b!("indexListStr", 1, Num, "index %s in %m.listStr"),
    b!("containListStr", 1, Bool, "%m.listStr contains %s"),
    b!("addMapToList", 1, Stmt, "add %m.varMap to %m.listMap"),
    b!("getMapInList", 1, Stmt, "get at %d of %m.listMap to %m.varMap"),
    b!("getAtListMap", 1, Str, "get value at %d key %s of %m.listMap"),
    b!("lengthList", 1, Num, "length of %m.list"),
    b!("deleteList", 1, Stmt, "delete at %d of %m.list"),
    b!("clearList", 1, Stmt, "clear %m.list"),
    b!("reverseList", 1, Stmt, "reverse %m.list"),
    b!("sortListInt", 1, Stmt, "sort %m.listNum smallest first"),
    b!("sortListStr", 1, Stmt, "sort %m.listStr A to Z"),
    b!("sumList", 1, Num, "sum of %m.listNum"),
    b!("splitToList", 1, Stmt, "split %s at %s into %m.listStr"),
    b!("joinList", 1, Str, "join %m.listStr with %s between"),
    // ---- Control (2)
    b!("repeat", 2, C, "repeat %d"),
    b!("forever", 2, C, "forever"),
    b!("whileLoop", 2, C, "while %b"),
    b!("if", 2, C, "if %b then"),
    b!("ifElse", 2, E, "if %b then", "else"),
    b!("break", 2, Cap, "stop"),
    // ---- Operator (3)
    b!("true", 3, Bool, "true"),
    b!("false", 3, Bool, "false"),
    b!("<", 3, Bool, "%d < %d"),
    b!("=", 3, Bool, "%d = %d"),
    b!(">", 3, Bool, "%d > %d"),
    b!("&&", 3, Bool, "%b and %b"),
    b!("||", 3, Bool, "%b or %b"),
    b!("not", 3, Bool, "not %b"),
    b!("+", 3, Num, "%d + %d"),
    b!("-", 3, Num, "%d - %d"),
    b!("*", 3, Num, "%d * %d"),
    b!("/", 3, Num, "%d / %d"),
    b!("%", 3, Num, "%d % %d"),
    b!("random", 3, Num, "pick random %d to %d"),
    b!("stringLength", 3, Num, "length of %s"),
    b!("stringJoin", 3, Str, "join %s and %s"),
    b!("stringIndex", 3, Num, "index %s of %s"),
    b!("stringLastIndex", 3, Num, "last index %s of %s"),
    b!("stringSub", 3, Str, "%s substring %d to %d"),
    b!("stringEquals", 3, Bool, "%s equals %s"),
    b!("stringContains", 3, Bool, "%s contains %s"),
    b!("stringReplace", 3, Str, "%s replace all %s with %s"),
    b!("trim", 3, Str, "trim %s"),
    b!("toUpperCase", 3, Str, "upper case %s"),
    b!("toLowerCase", 3, Str, "lower case %s"),
    b!("toNumber", 3, Num, "number from %s"),
    b!("toString", 3, Str, "text of %d without decimals"),
    b!("toStringWithDecimal", 3, Str, "text of %d with decimals"),
    b!("toStringFixed", 3, Str, "text of %d with %d decimals"),
    b!("stringStartsWith", 3, Bool, "%s starts with %s"),
    b!("stringEndsWith", 3, Bool, "%s ends with %s"),
    b!("isNumber", 3, Bool, "%s is a number"),
    // ---- Math (4)
    b!("mathGetDip", 4, Num, "%d dp in pixels"),
    b!("mathGetDisplayWidth", 4, Num, "screen width in pixels"),
    b!("mathGetDisplayHeight", 4, Num, "screen height in pixels"),
    b!("mathPi", 4, Num, "PI(π)"),
    b!("mathE", 4, Num, "E(e)"),
    b!("mathPow", 4, Num, "%d to the %d power"),
    b!("mathMin", 4, Num, "minimum of %d and %d"),
    b!("mathMax", 4, Num, "maximum of %d and %d"),
    b!("mathSqrt", 4, Num, "square root of %d"),
    b!("mathAbs", 4, Num, "absolute value of %d"),
    b!("mathRound", 4, Num, "round %d"),
    b!("mathCeil", 4, Num, "ceil %d"),
    b!("mathFloor", 4, Num, "floor %d"),
    b!("mathSin", 4, Num, "sin %d"),
    b!("mathCos", 4, Num, "cos %d"),
    b!("mathTan", 4, Num, "tan %d"),
    b!("mathLog", 4, Num, "ln %d"),
    b!("mathLog10", 4, Num, "log %d"),
    b!("mathToRadian", 4, Num, "degrees %d to radians"),
    b!("mathToDegree", 4, Num, "radians %d to degrees"),
    // ---- File (5)
    b!("fileGetData", 5, Str, "%m.file saved value for %s"),
    b!("fileSetData", 5, Stmt, "%m.file save key %s value %s"),
    b!("fileRemoveData", 5, Stmt, "%m.file forget key %s"),
    b!("fileContainsData", 5, Bool, "%m.file contains key %s"),
    // ---- View (6)
    b!("setEnable", 6, Stmt, "%m.view set enabled %b"),
    b!("getEnable", 6, Bool, "%m.view is enabled"),
    b!("setVisible", 6, Stmt, "%m.view set visibility %m.visible"),
    b!("setBgColor", 6, Stmt, "%m.view set background %m.color"),
    b!("setAlpha", 6, Stmt, "%m.view set opacity %d"),
    b!("setRadius", 6, Stmt, "%m.view set corner radius %d"),
    b!("setElevation", 6, Stmt, "%m.view set shadow %d"),
    b!("setBorder", 6, Stmt, "%m.view set border %d colour %m.color"),
    b!("setText", 6, Stmt, "%m.textview set text %s"),
    b!("getText", 6, Str, "%m.textview text"),
    b!("setTextColor", 6, Stmt, "%m.textview set text colour %m.color"),
    b!("setTextSize", 6, Stmt, "%m.textview set text size %d"),
    b!("setHint", 6, Stmt, "%m.edittext set hint text to %s"),
    b!("setChecked", 6, Stmt, "%m.checkbox set checked %b"),
    b!("getChecked", 6, Bool, "%m.checkbox is checked"),
    b!("seekBarSetProgress", 6, Stmt, "%m.seekbar set value %d"),
    b!("seekBarGetProgress", 6, Num, "%m.seekbar value"),
    b!("seekBarSetMax", 6, Stmt, "%m.seekbar set maximum %d"),
    b!("seekBarGetMax", 6, Num, "%m.seekbar maximum"),
    b!("listSetData", 6, Stmt, "%m.listspn show items %m.listStr"),
    b!("webViewLoadUrl", 6, Stmt, "%m.webview open address %s"),
    b!("setImage", 6, Stmt, "%m.imageview set picture %m.image"),
    // a picture, video or sound from a file path (camera, phone) or a web address
    b!("setImageFrom", 6, Stmt, "%m.imageview set picture from %s"),
    b!("setFont", 6, Stmt, "%m.textview set font %m.font"),
    // videos: a video widget plays one of the project's videos
    b!("setVideo", 6, Stmt, "%m.videoview set video %m.video"),
    b!("setVideoFrom", 6, Stmt, "%m.videoview set video from %s"),
    b!("videoPlay", 6, Stmt, "%m.videoview play"),
    b!("videoPause", 6, Stmt, "%m.videoview pause"),
    b!("videoSeek", 6, Stmt, "%m.videoview go to %d ms"),
    b!("videoLoop", 6, Stmt, "%m.videoview play again and again %b"),
    b!("videoIsPlaying", 6, Bool, "%m.videoview is playing"),
    b!("videoPosition", 6, Num, "%m.videoview position in ms"),
    b!("videoDuration", 6, Num, "%m.videoview length in ms"),
    b!("assetText", 3, Str, "text of data file %m.asset"),
    b!("requestFocus", 6, Stmt, "%m.view request focus"),
    b!("performClick", 6, Stmt, "%m.view tap"),
    // ---- Component (7)
    b!("doToast", 7, Stmt, "show message %s"),
    b!("setTitle", 7, Stmt, "set screen title %s"),
    b!("copyToClipboard", 7, Stmt, "copy %s to clipboard"),
    b!("hideKeyboard", 7, Stmt, "hide the keyboard"),
    b!("openLink", 7, Stmt, "open link %s"),
    b!("shareText", 7, Stmt, "share text %s"),
    b!("finishActivity", 7, Stmt, "close this screen"),
    b!("intentSetScreen", 7, Stmt, "%m.intent go to %m.screen"),
    b!("intentPutExtra", 7, Stmt, "%m.intent pass key %s value %s"),
    b!("intentGetString", 7, Str, "value passed for key %s"),
    b!("startActivity", 7, Stmt, "open screen with %m.intent"),
    // one block for the common case: no Intent component needed
    b!("openScreen", 7, Stmt, "open screen %m.screen"),
    b!("timerAfter", 7, C, "%m.timer after %d ms"),
    b!("timerEvery", 7, C, "%m.timer after %d ms for every %d ms"),
    b!("timerCancel", 7, Stmt, "%m.timer cancel"),
    b!("dialogSetTitle", 7, Stmt, "%m.dialog set title %s"),
    b!("dialogSetMessage", 7, Stmt, "%m.dialog set message %s"),
    b!("dialogOkButton", 7, C, "%m.dialog when OK button %s pressed"),
    b!("dialogCancelButton", 7, C, "%m.dialog when cancel button %s pressed"),
    b!("dialogShow", 7, Stmt, "%m.dialog show"),
    b!("vibratorAction", 7, Stmt, "%m.vibrator vibrate for %d ms"),
    b!("calendarGetNow", 7, Stmt, "%m.calendar set to now"),
    b!("calendarFormat", 7, Str, "%m.calendar as text %s"),
    b!("calendarGetTime", 7, Num, "%m.calendar in milliseconds"),
    // web requests: the first inner stack runs with the answer, the second
    // when the request fails (no connection, or a code outside 200-299)
    b!("webRequest", 7, E, "%m.web send %m.method to %s with body %s", "if it fails"),
    b!("webSetHeader", 7, Stmt, "%m.web set header %s to %s"),
    b!("webClearHeaders", 7, Stmt, "%m.web clear headers"),
    // sounds: one player plays one sound at a time
    b!("soundPlay", 7, Stmt, "%m.player play %m.sound"),
    b!("soundPlayFrom", 7, Stmt, "%m.player play from %s"),
    b!("soundPause", 7, Stmt, "%m.player pause"),
    b!("soundResume", 7, Stmt, "%m.player resume"),
    b!("soundStop", 7, Stmt, "%m.player stop"),
    b!("soundLoop", 7, Stmt, "%m.player play again and again %b"),
    b!("soundVolume", 7, Stmt, "%m.player set volume %d percent"),
    b!("soundSeek", 7, Stmt, "%m.player go to %d ms"),
    b!("soundIsPlaying", 7, Bool, "%m.player is playing"),
    b!("soundPosition", 7, Num, "%m.player position in ms"),
    b!("soundDuration", 7, Num, "%m.player length in ms"),
    // camera and the phone's own files: the first inner stack runs with the
    // file (its path in "file path"), the second when the person cancels
    b!("cameraPhoto", 7, E, "%m.camera take a photo", "if cancelled"),
    b!("cameraVideo", 7, E, "%m.camera record a video", "if cancelled"),
    b!("pickMedia", 7, E, "pick a %m.mediaKind from the phone", "if cancelled"),
    b!("mediaPath", 7, Str, "file path"),
    b!("webResponse", 7, Str, "response"),
    b!("webStatus", 7, Num, "response code"),
    // ---- Supabase (9): sign-in and tables of the linked project
    b!("supaSignUp", 9, E, "Supabase sign up with email %s password %s", "if it fails"),
    b!("supaSignIn", 9, E, "Supabase sign in with email %s password %s", "if it fails"),
    b!("supaSignOut", 9, Stmt, "Supabase sign out"),
    b!("supaSignedIn", 9, Bool, "signed in"),
    b!("supaUserEmail", 9, Str, "user's email"),
    b!("supaUserId", 9, Str, "user's id"),
    b!("supaSelect", 9, E, "Supabase get rows of table %s", "if it fails"),
    b!("supaSelectWhere", 9, E, "Supabase get rows of table %s where %s is %s", "if it fails"),
    b!("supaInsert", 9, E, "Supabase add %m.varMap to table %s", "if it fails"),
    b!("supaUpdate", 9, E, "Supabase change rows of table %s where %s is %s to %m.varMap", "if it fails"),
    b!("supaDelete", 9, E, "Supabase delete rows of table %s where %s is %s", "if it fails"),
    b!("supaError", 9, Str, "error message"),
    // JSON
    b!("mapFromJson", 0, Stmt, "%m.varMap read JSON %s"),
    b!("mapToJson", 0, Str, "%m.varMap as JSON"),
    b!("listMapFromJson", 1, Stmt, "%m.listMap read JSON %s"),
    b!("listMapToJson", 1, Str, "%m.listMap as JSON"),
    // ---- More Block (8)
    b!("moreBlock", 8, Stmt, "@0"),
    // a more block's inputs, inside its definition
    b!("paramStr", 8, Str, "@0"),
    b!("paramNum", 8, Num, "@0"),
    b!("paramBool", 8, Bool, "@0"),
];

/// The reporter that reads a more block input of this type.
pub fn param_op(ty: VarType) -> &'static str {
    match ty {
        VarType::Num => "paramNum",
        VarType::Bool => "paramBool",
        _ => "paramStr",
    }
}

/// A more block call's spec: its name, then each input's name and slot
/// ("greet name %s times %d").
pub fn more_spec(params: &[Var]) -> String {
    let mut s = String::from("@0");
    for p in params {
        let slot = match p.ty {
            VarType::Num => "%d",
            VarType::Bool => "%b",
            _ => "%s",
        };
        s.push_str(&format!(" {} {slot}", p.name));
    }
    s
}

/// The spec a block is drawn and written from: a more block call's comes
/// from its definition on the screen.
pub fn spec_for(b: &Block, s: &Screen) -> String {
    if b.op == "moreBlock" {
        if let Some(Arg::Text(n)) = b.args.first() {
            if let Some(e) = s.events.iter().find(|e| e.target == "moreBlock" && &e.name == n) {
                return more_spec(&e.params);
            }
        }
    }
    def(&b.op).map(|d| d.spec.to_string()).unwrap_or_default()
}

pub fn def(op: &str) -> Option<&'static BlockDef> {
    DEFS.iter().find(|d| d.op == op)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Text,
    Num,
    Bool,
    Menu(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Label(String),
    /// Label taken from args[n] (variable reporters, more block calls).
    ArgLabel(usize),
    Param(usize, Param),
}

pub fn parse_spec(spec: &str) -> Vec<Token> {
    let mut out = vec![];
    let mut idx = 0;
    let mut label = String::new();
    for word in spec.split(' ') {
        let p = match word {
            "%s" => Some(Param::Text),
            "%d" => Some(Param::Num),
            "%b" => Some(Param::Bool),
            w if w.starts_with("%m.") => Some(Param::Menu(w[3..].to_string())),
            w if w.starts_with('@') && w.len() > 1 => {
                if !label.is_empty() {
                    out.push(Token::Label(std::mem::take(&mut label)));
                }
                out.push(Token::ArgLabel(w[1..].parse().unwrap_or(0)));
                idx += 1;
                continue;
            }
            _ => None,
        };
        match p {
            Some(p) => {
                if !label.is_empty() {
                    out.push(Token::Label(std::mem::take(&mut label)));
                }
                out.push(Token::Param(idx, p));
                idx += 1;
            }
            None => {
                if !label.is_empty() {
                    label.push(' ');
                }
                label.push_str(word);
            }
        }
    }
    if !label.is_empty() {
        out.push(Token::Label(label));
    }
    out
}

#[allow(dead_code)]
pub fn param_count(spec: &str) -> usize {
    parse_spec(spec).iter().filter(|t| !matches!(t, Token::Label(_))).count()
}

/// Shape of a block, resolving variable reporters by the variable's type.
pub fn shape_of(b: &Block, screen: &Screen) -> Shape {
    if b.op == "getVar" {
        if let Some(Arg::Text(n)) = b.args.first() {
            if let Some(v) = screen.vars.iter().find(|v| &v.name == n) {
                return match v.ty {
                    VarType::Bool => Shape::Bool,
                    VarType::Num => Shape::Num,
                    _ => Shape::Str,
                };
            }
        }
        return Shape::Str;
    }
    def(&b.op).map(|d| d.shape).unwrap_or(Shape::Stmt)
}

pub fn color_of(b: &Block) -> u32 {
    def(&b.op).map(|d| CATEGORIES[d.cat].color).unwrap_or(0x888888)
}

/// A fresh block from the palette with default argument values.
pub fn make_block(op: &str, screen: &Screen, screens: &Choices) -> Block {
    let d = def(op);
    let mut args = vec![];
    if let Some(d) = d {
        for t in parse_spec(d.spec) {
            match t {
                Token::Param(_, Param::Text) => args.push(Arg::Text(String::new())),
                Token::Param(_, Param::Num) => args.push(Arg::Text(if op == "repeat" { "10".into() } else { "0".into() })),
                Token::Param(_, Param::Bool) => args.push(Arg::Text(String::new())),
                Token::Param(_, Param::Menu(m)) => {
                    let c = menu_choices(&m, screen, screens);
                    // a screen picker starts on another screen than this one
                    let here = screen.activity_name();
                    let pick = if m == "screen" { c.iter().find(|n| **n != here).or(c.first()) } else { c.first() };
                    args.push(Arg::Text(pick.cloned().unwrap_or_default()));
                }
                Token::ArgLabel(_) => args.push(Arg::Text(String::new())),
                Token::Label(_) => {}
            }
        }
    }
    if op == "timerEvery" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text("0".into());
        }
        if let Some(a) = args.get_mut(2) {
            *a = Arg::Text("1000".into());
        }
    }
    if op == "timerAfter" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text("1000".into());
        }
    }
    if op == "dialogOkButton" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text("OK".into());
        }
    }
    if op == "dialogCancelButton" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text("Cancel".into());
        }
    }
    if op == "webRequest" {
        if let Some(a) = args.get_mut(2) {
            *a = Arg::Text("https://".into());
        }
    }
    if op == "toStringFixed" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text("2".into());
        }
    }
    if op == "splitToList" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text(",".into());
        }
    }
    if op == "joinList" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text(", ".into());
        }
    }
    if op == "openLink" {
        if let Some(a) = args.get_mut(0) {
            *a = Arg::Text("https://".into());
        }
    }
    if op == "soundVolume" {
        if let Some(a) = args.get_mut(1) {
            *a = Arg::Text("100".into());
        }
    }
    if op == "random" {
        args = vec![Arg::Text("1".into()), Arg::Text("10".into())];
    }
    Block { id: new_id(), op: op.into(), args, sub1: vec![], sub2: vec![] }
}

pub fn named_block(op: &str, name: &str) -> Block {
    Block { id: new_id(), op: op.into(), args: vec![Arg::Text(name.into())], sub1: vec![], sub2: vec![] }
}

pub const COLORS: [(&str, u32); 12] = [
    ("Color.TRANSPARENT", 0x0000_0000),
    ("0xFFFFFFFF", 0xFFFF_FFFF),
    ("0xFF000000", 0xFF00_0000),
    ("0xFF008DCD", 0xFF00_8DCD),
    ("0xFFF44336", 0xFFF4_4336),
    ("0xFFE91E63", 0xFFE9_1E63),
    ("0xFF9C27B0", 0xFF9C_27B0),
    ("0xFF3F51B5", 0xFF3F_51B5),
    ("0xFF4CAF50", 0xFF4C_AF50),
    ("0xFFFFEB3B", 0xFFFF_EB3B),
    ("0xFFFF9800", 0xFFFF_9800),
    ("0xFF9E9E9E", 0xFF9E_9E9E),
];

pub fn color_value(name: &str) -> u32 {
    COLORS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c).unwrap_or(0xFF00_0000)
}

/// Choices for a %m.<kind> drop-down in the current screen.
pub fn menu_choices(kind: &str, s: &Screen, screens: &Choices) -> Vec<String> {
    let widgets = |f: &dyn Fn(WKind) -> bool| -> Vec<String> { s.widgets.iter().filter(|w| f(w.kind)).map(|w| w.id.clone()).collect() };
    let vars = |t: VarType| -> Vec<String> { s.vars.iter().filter(|v| v.ty == t).map(|v| v.name.clone()).collect() };
    let lists = |t: Option<ListType>| -> Vec<String> { s.lists.iter().filter(|l| t.map_or(true, |t| l.ty == t)).map(|l| l.name.clone()).collect() };
    let comps = |k: CompKind| -> Vec<String> { s.components.iter().filter(|c| c.kind == k).map(|c| c.name.clone()).collect() };
    match kind {
        "view" => widgets(&|_| true),
        "textview" => widgets(&|k| k.has_text()),
        "edittext" => widgets(&|k| k == WKind::EditText),
        "checkbox" => widgets(&|k| k.is_compound()),
        "seekbar" => widgets(&|k| k == WKind::SeekBar || k == WKind::ProgressBar),
        "listspn" => widgets(&|k| k == WKind::ListView || k == WKind::Spinner),
        "webview" => widgets(&|k| k == WKind::WebView),
        "imageview" => widgets(&|k| k == WKind::ImageView),
        "videoview" => widgets(&|k| k == WKind::VideoView),
        "video" => screens.videos.clone(),
        "image" => screens.images.clone(),
        "sound" => screens.sounds.clone(),
        "font" => screens.fonts.clone(),
        "asset" => screens.assets.clone(),
        "player" => comps(CompKind::Sound),
        "varBool" => vars(VarType::Bool),
        "varNum" => vars(VarType::Num),
        "varStr" => vars(VarType::Str),
        "varMap" => vars(VarType::Map),
        "listNum" => lists(Some(ListType::Num)),
        "listStr" => lists(Some(ListType::Str)),
        "listMap" => lists(Some(ListType::Map)),
        "list" => lists(None),
        "timer" => comps(CompKind::Timer),
        "dialog" => comps(CompKind::Dialog),
        "intent" => comps(CompKind::Intent),
        "file" => comps(CompKind::SharedPref),
        "vibrator" => comps(CompKind::Vibrator),
        "calendar" => comps(CompKind::Calendar),
        "web" => comps(CompKind::Web),
        "camera" => comps(CompKind::Camera),
        "mediaKind" => ["picture", "video", "sound"].iter().map(|m| m.to_string()).collect(),
        "method" => ["GET", "POST", "PUT", "PATCH", "DELETE"].iter().map(|m| m.to_string()).collect(),
        "screen" => screens.screens.clone(),
        "visible" => vec!["VISIBLE".into(), "INVISIBLE".into(), "GONE".into()],
        "color" => COLORS.iter().map(|(n, _)| n.to_string()).collect(),
        _ => vec![],
    }
}

/// What a drop-down value is called on screen. Saved values keep their
/// first form (so old projects still open); only the words change.
pub fn menu_label(kind: &str, value: &str) -> String {
    match (kind, value) {
        ("visible", "VISIBLE") => "shown".into(),
        ("visible", "INVISIBLE") => "hidden".into(),
        ("visible", "GONE") => "removed".into(),
        ("color", "Color.TRANSPARENT") => "clear".into(),
        ("color", v) => match v {
            "0xFFFFFFFF" => "white".into(),
            "0xFF000000" => "black".into(),
            "0xFF008DCD" => "blue".into(),
            "0xFFF44336" => "red".into(),
            "0xFFE91E63" => "pink".into(),
            "0xFF9C27B0" => "purple".into(),
            "0xFF3F51B5" => "indigo".into(),
            "0xFF4CAF50" => "green".into(),
            "0xFFFFEB3B" => "yellow".into(),
            "0xFFFF9800" => "orange".into(),
            "0xFF9E9E9E" => "grey".into(),
            v => v.into(),
        },
        ("screen", v) => screen_label(v),
        (_, v) => v.into(),
    }
}

/// "SecondActivity" → "second screen" (the class name stays inside the code).
pub fn screen_label(class_name: &str) -> String {
    let base = class_name.strip_suffix("Activity").unwrap_or(class_name);
    let mut out = String::new();
    for (i, c) in base.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c.to_ascii_lowercase());
    }
    if out.is_empty() {
        class_name.to_string()
    } else {
        format!("{out} screen")
    }
}

/// What drop-downs can pick from the whole project: its screens (read as a
/// list of their class names) and its pictures and sounds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choices {
    pub screens: Vec<String>,
    pub images: Vec<String>,
    pub sounds: Vec<String>,
    pub fonts: Vec<String>,
    /// Data files.
    pub assets: Vec<String>,
    pub videos: Vec<String>,
}

impl Choices {
    #[allow(dead_code)]
    pub fn screens(screens: Vec<String>) -> Self {
        Choices { screens, ..Default::default() }
    }
}

impl std::ops::Deref for Choices {
    type Target = [String];
    fn deref(&self) -> &[String] {
        &self.screens
    }
}

/// Screens are resolved at the project level.
pub fn screen_choices(p: &Project) -> Choices {
    Choices {
        screens: p.screens.iter().map(|s| s.activity_name()).collect(),
        images: p.file_names(FileKind::Image),
        sounds: p.file_names(FileKind::Sound),
        fonts: p.file_names(FileKind::Font),
        assets: p.file_names(FileKind::Data),
        videos: p.file_names(FileKind::Video),
    }
}

/// Which palette entries a category shows (ops; "#Title" is a section header).
pub fn palette(cat: usize) -> Vec<&'static str> {
    match cat {
        0 => vec!["setVarBoolean", "setVarInt", "setVarString", "increaseInt", "decreaseInt", "#Map", "mapCreateNew", "mapPut", "mapGet", "mapContainKey", "mapRemoveKey", "mapSize", "mapClear", "mapIsEmpty", "mapFromJson", "mapToJson", "#Event values", "eventPosition", "eventChecked", "eventText", "eventProgress"],
        1 => vec!["addListInt", "insertListInt", "getAtListInt", "indexListInt", "containListInt", "addListStr", "insertListStr", "getAtListStr", "indexListStr", "containListStr", "#List Map", "addMapToList", "getMapInList", "getAtListMap", "listMapFromJson", "listMapToJson", "sortListInt", "sumList", "sortListStr", "splitToList", "joinList", "#Any list", "lengthList", "deleteList", "clearList", "reverseList"],
        2 => vec!["repeat", "forever", "whileLoop", "if", "ifElse", "break"],
        3 => DEFS.iter().filter(|d| d.cat == 3).map(|d| d.op).collect(),
        4 => DEFS.iter().filter(|d| d.cat == 4).map(|d| d.op).collect(),
        5 => vec!["#Saved data", "fileGetData", "fileSetData", "fileRemoveData", "fileContainsData"],
        6 => vec!["setEnable", "getEnable", "setVisible", "setBgColor", "setAlpha", "setRadius", "setElevation", "setBorder", "requestFocus", "performClick", "#Text and values", "setText", "getText", "setTextColor", "setTextSize", "setFont", "setHint", "setChecked", "getChecked", "seekBarSetProgress", "seekBarGetProgress", "seekBarSetMax", "seekBarGetMax", "#List", "listSetData", "#Web page", "webViewLoadUrl", "#Picture", "setImage", "setImageFrom", "#Video", "setVideo", "setVideoFrom", "videoPlay", "videoPause", "videoSeek", "videoLoop", "videoIsPlaying", "videoPosition", "videoDuration"],
        7 => vec!["#Screen", "doToast", "setTitle", "copyToClipboard", "hideKeyboard", "finishActivity", "#Links and sharing", "openLink", "shareText", "#Navigation", "openScreen", "intentSetScreen", "intentPutExtra", "intentGetString", "startActivity", "#Timer", "timerAfter", "timerEvery", "timerCancel", "#Pop-up", "dialogSetTitle", "dialogSetMessage", "dialogOkButton", "dialogCancelButton", "dialogShow", "#Sound", "soundPlay", "soundPlayFrom", "soundPause", "soundResume", "soundStop", "soundLoop", "soundVolume", "soundSeek", "soundIsPlaying", "soundPosition", "soundDuration", "#Vibration", "vibratorAction", "#Date & time", "calendarGetNow", "calendarFormat", "calendarGetTime", "#Camera and phone files", "cameraPhoto", "cameraVideo", "pickMedia", "mediaPath", "#Web request", "webRequest", "webSetHeader", "webClearHeaders", "webResponse", "webStatus"],
        9 => vec!["#Account", "supaSignUp", "supaSignIn", "supaSignOut", "supaSignedIn", "supaUserEmail", "supaUserId", "#Tables", "supaSelect", "supaSelectWhere", "supaInsert", "supaUpdate", "supaDelete", "#The answer (inside a Supabase block)", "webResponse", "webStatus", "supaError"],
        _ => vec![],
    }
}

/// A block as one line of plain text, with the values typed into it
/// (plugged-in blocks shown in brackets). Used by the debugger.
pub fn describe(b: &Block) -> String {
    let Some(d) = def(&b.op) else { return b.op.clone() };
    if b.op == "moreBlock" {
        // the call's inputs, without knowing their names
        let mut out = vec![];
        for (i, a) in b.args.iter().enumerate() {
            out.push(match a {
                Arg::Text(t) if i == 0 => t.clone(),
                Arg::Text(t) => format!("{t:?}"),
                Arg::Block(r) => format!("({})", describe(r)),
            });
        }
        return out.join(" ");
    }
    let mut out = vec![];
    for t in parse_spec(d.spec) {
        match t {
            Token::Label(l) => out.push(l),
            Token::ArgLabel(i) => match b.args.get(i) {
                Some(Arg::Text(t)) => out.push(t.clone()),
                _ => {}
            },
            Token::Param(i, p) => out.push(match b.args.get(i) {
                Some(Arg::Block(r)) => format!("({})", describe(r)),
                Some(Arg::Text(t)) => match p {
                    Param::Text => format!("{t:?}"),
                    Param::Bool => "false".into(),
                    _ => t.clone(),
                },
                None => String::new(),
            }),
        }
    }
    out.join(" ")
}
