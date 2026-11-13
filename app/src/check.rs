//! Does the preview run blocks the way the phones do? One block program
//! that stores every result in a variable runs in the preview and in debug
//! builds on the Android emulator and the iPhone simulator; debug builds log
//! each value set, so the three lists of values can be compared in order.
//!
//! Four checks, each against both phones (BLOCKWISE_DEVICE=android or ios
//! for one): `cargo test check_ -- --ignored --nocapture`
//! - check_preview_against_phones: numbers, text, lists, maps, JSON, loops
//! - check_crashes_against_phones: mistakes that crash, one small app each
//! - check_timers_against_phones: repeating, cancelled and restarted timers
//! - check_supabase_against_phones: against a pretend Supabase on this computer

use crate::blocks::describe;
use crate::device::{Device, DeviceRun, LineKind, Phase};
use crate::model::*;
use crate::runtime::{Runtime, Val};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn r(op: &str, args: Value) -> Value {
    json!({"op": op, "args": args})
}
fn n(e: Value) -> Value {
    r("setVarInt", json!(["n", e]))
}
fn s(e: Value) -> Value {
    r("setVarString", json!(["s", e]))
}
fn b(e: Value) -> Value {
    r("setVarBoolean", json!(["b", e]))
}

/// The program: plain values only (no randomness, clock or screen size),
/// with the awkward cases on purpose. It ends by setting `done`.
pub fn program() -> Vec<Value> {
    let mut p = vec![];
    // ---- numbers
    for (x, y) in [("7", "2"), ("0.1", "0.2"), ("-7", "2"), ("1e10", "3")] {
        for op in ["+", "-", "*", "/", "%"] {
            p.push(n(r(op, json!([x, y]))));
        }
    }
    for (x, y) in [("7", "0"), ("-7", "0"), ("0", "0"), ("7.5", "2")] {
        p.push(n(r("/", json!([x, y]))));
        p.push(n(r("%", json!([x, y]))));
    }
    for (op, x) in [("mathSqrt", "2"), ("mathSqrt", "-1"), ("mathAbs", "-3.5"), ("mathRound", "2.5"), ("mathRound", "-2.5"), ("mathRound", "3.49"), ("mathCeil", "-1.5"), ("mathFloor", "-1.5"), ("mathSin", "1"), ("mathCos", "0"), ("mathTan", "1"), ("mathLog", "10"), ("mathLog10", "1000"), ("mathToRadian", "180"), ("mathToDegree", "3.14159")] {
        p.push(n(r(op, json!([x]))));
    }
    p.push(n(r("mathPi", json!([]))));
    p.push(n(r("mathE", json!([]))));
    for (op, x, y) in [("mathPow", "2", "10"), ("mathPow", "2", "0.5"), ("mathMin", "3", "-1"), ("mathMax", "3", "-1")] {
        p.push(n(r(op, json!([x, y]))));
    }
    // ---- text
    p.push(s(r("stringSub", json!(["👍ab", "2", "4"]))));
    p.push(n(r("stringIndex", json!(["b", "👍ab"]))));
    for t in ["héllo", "👍", "", "a b"] {
        p.push(n(r("stringLength", json!([t]))));
    }
    p.push(n(r("stringIndex", json!(["o", "Hello World"]))));
    p.push(n(r("stringIndex", json!(["z", "Hello"]))));
    p.push(n(r("stringLastIndex", json!(["o", "Hello World"]))));
    for t in ["3.5", " 42 ", "1e3", "-0"] {
        p.push(n(r("toNumber", json!([t]))));
    }
    p.push(s(r("stringJoin", json!(["a", "b"]))));
    p.push(s(r("stringJoin", json!(["n=", r("toString", json!([r("+", json!(["1", "2"]))]))]))));
    p.push(s(r("stringJoin", json!(["half ", r("toStringWithDecimal", json!([r("/", json!(["1", "2"]))]))]))));
    p.push(s(r("stringSub", json!(["Hello World", "0", "5"]))));
    p.push(s(r("stringSub", json!(["Hello World", "6", "11"]))));
    p.push(s(r("stringReplace", json!(["a-b-c", "-", "+"]))));
    p.push(s(r("stringReplace", json!(["a.b.c", ".", ""]))));
    p.push(s(r("trim", json!(["  hi  "]))));
    p.push(s(r("toUpperCase", json!(["élan"]))));
    p.push(s(r("toLowerCase", json!(["ÀB"]))));
    for x in ["3.7", "-3.7", "1e20", "0"] {
        p.push(s(r("toString", json!([x]))));
    }
    for x in [json!("3"), r("+", json!(["0.1", "0.2"])), json!("1e20"), json!("0.000001"), json!("1234567.5"), json!("12345678"), json!("-0.001"), r("/", json!(["1", "0"])), r("mathSqrt", json!(["-1"]))] {
        p.push(s(r("toStringWithDecimal", json!([x]))));
    }
    // ---- true / false
    p.push(b(r("<", json!(["1", "2"]))));
    p.push(b(r("=", json!([r("+", json!(["0.1", "0.2"])), "0.3"]))));
    p.push(b(r("=", json!(["2", "2.0"]))));
    p.push(b(r(">", json!(["-1", "-2"]))));
    p.push(b(r("&&", json!([r("true", json!([])), r("false", json!([]))]))));
    p.push(b(r("||", json!([r("true", json!([])), r("false", json!([]))]))));
    p.push(b(r("not", json!([r("false", json!([]))]))));
    p.push(b(r("stringEquals", json!(["a", "A"]))));
    p.push(b(r("stringContains", json!(["Hello", "ell"]))));
    p.push(b(r("stringContains", json!(["Hello", ""]))));
    // ---- lists
    p.push(r("addListInt", json!(["3", "ln"])));
    p.push(r("addListInt", json!(["1", "ln"])));
    p.push(r("insertListInt", json!(["9", "0", "ln"])));
    p.push(n(r("getAtListInt", json!(["0", "ln"]))));
    p.push(n(r("indexListInt", json!(["1", "ln"]))));
    p.push(n(r("indexListInt", json!(["5", "ln"]))));
    p.push(b(r("containListInt", json!(["ln", "3"]))));
    p.push(n(r("lengthList", json!(["ln"]))));
    p.push(r("deleteList", json!(["1", "ln"])));
    p.push(n(r("getAtListInt", json!(["1", "ln"]))));
    p.push(r("addListStr", json!(["x", "ls"])));
    p.push(r("addListStr", json!(["y", "ls"])));
    p.push(s(r("getAtListStr", json!(["1", "ls"]))));
    p.push(n(r("indexListStr", json!(["y", "ls"]))));
    p.push(b(r("containListStr", json!(["ls", "z"]))));
    // ---- maps and JSON
    p.push(r("mapPut", json!(["m", "a", "1"])));
    p.push(r("mapPut", json!(["m", "b", "x y"])));
    p.push(n(r("mapSize", json!(["m"]))));
    p.push(b(r("mapContainKey", json!(["m", "a"]))));
    p.push(s(r("mapGet", json!(["m", "b"]))));
    p.push(s(r("mapToJson", json!(["m"]))));
    p.push(r("mapRemoveKey", json!(["m", "a"])));
    p.push(s(r("mapToJson", json!(["m"]))));
    p.push(r("mapPut", json!(["m", "u", "a/b \"q\" é"])));
    p.push(s(r("mapToJson", json!(["m"]))));
    p.push(r("mapFromJson", json!(["m", r#"{"k":[1,2],"t":true,"x":null,"f":1.5,"i":2,"o":{"z":1}}"#])));
    for k in ["k", "t", "x", "f", "i", "o"] {
        p.push(s(r("mapGet", json!(["m", k]))));
    }
    p.push(r("listMapFromJson", json!(["lm", r#"[{"a":1},{"a":"two"},"loose"]"#])));
    p.push(n(r("lengthList", json!(["lm"]))));
    p.push(s(r("getAtListMap", json!(["1", "a", "lm"]))));
    p.push(s(r("getAtListMap", json!(["2", "value", "lm"]))));
    p.push(s(r("listMapToJson", json!(["lm"]))));
    // ---- decimals, starts and ends, numbers in text
    for (x, d) in [("2.675", "2"), ("0.125", "2"), ("-2.5", "0"), ("1e20", "2"), ("-0.001", "2"), ("9.995", "2"), ("1.5e-6", "7"), ("3", "0"), ("1234.5678", "1")] {
        p.push(s(r("toStringFixed", json!([x, d]))));
    }
    p.push(s(r("toStringFixed", json!([r("/", json!(["1", "3"])), "4"]))));
    for (a, x) in [("Hello", "He"), ("Hello", ""), ("👍ab", "👍"), ("é", "e"), ("ab", "abc")] {
        p.push(b(r("stringStartsWith", json!([a, x]))));
    }
    for (a, x) in [("Hello", "lo"), ("ab👍", "👍"), ("x", ""), ("café", "é"), ("a", "ba")] {
        p.push(b(r("stringEndsWith", json!([a, x]))));
    }
    p.push(b(r("stringStartsWith", json!([r("stringJoin", json!(["ab", "cd"])), "abc"]))));
    for t in ["3.5", " 42 ", "abc", "", "1e3", "-0", "NaN"] {
        p.push(b(r("isNumber", json!([t]))));
    }
    // ---- splitting, joining, sorting and adding up lists
    p.push(r("splitToList", json!(["pear,Apple,,banana,Éclair,apple", ",", "ls"])));
    p.push(n(r("lengthList", json!(["ls"]))));
    p.push(s(r("joinList", json!(["ls", "|"]))));
    p.push(r("sortListStr", json!(["ls"])));
    p.push(s(r("joinList", json!(["ls", "|"]))));
    p.push(r("reverseList", json!(["ls"])));
    p.push(s(r("joinList", json!(["ls", ", "]))));
    p.push(r("splitToList", json!(["a b", "", "ls"])));
    p.push(s(r("joinList", json!(["ls", "+"]))));
    p.push(r("clearList", json!(["ln"])));
    for x in ["3", "-1", "2.5", "0.1", "0.2"] {
        p.push(r("addListInt", json!([x, "ln"])));
    }
    p.push(n(r("sumList", json!(["ln"]))));
    p.push(r("sortListInt", json!(["ln"])));
    p.push(n(r("getAtListInt", json!(["0", "ln"]))));
    p.push(n(r("getAtListInt", json!(["4", "ln"]))));
    p.push(r("reverseList", json!(["ln"])));
    p.push(n(r("getAtListInt", json!(["0", "ln"]))));
    // ---- a more block with inputs (defined in project())
    p.push(r("moreBlock", json!(["pair", "1.5", "ab", true])));
    p.push(r("moreBlock", json!(["pair", r("+", json!(["1", "2"])), r("stringJoin", json!(["x", "y"])), false])));
    // ---- control
    p.push(n(json!("0")));
    p.push(json!({"op": "repeat", "args": ["3"], "do": [r("increaseInt", json!(["n"]))]}));
    p.push(json!({"op": "if", "args": [r(">", json!([r("getVar", json!(["n"])), "2"]))], "do": [s(json!("more"))]}));
    p.push(r("setVarBoolean", json!(["done", r("true", json!([]))])));
    p
}

pub fn project() -> Project {
    let mut p = Project::new(1, "Check", "Check", "com.blockwise.check");
    let vars = json!([
        {"name": "n", "type": "number"}, {"name": "s", "type": "string"}, {"name": "b", "type": "boolean"}, {"name": "done", "type": "boolean"},
        {"name": "m", "type": "map"}, {"name": "ln", "type": "list_number"}, {"name": "ls", "type": "list_string"}, {"name": "lm", "type": "list_map"},
    ]);
    let spec = json!({"variables": vars, "events": [{"target": "screen", "event": "onCreate", "blocks": []}]});
    p.screens[0] = crate::templates::build(&spec, "main", &Default::default()).expect("the check program is made of valid blocks");
    // pair x w f: n = x * 2, s = w + w, b = not f
    let pair = json!([
        n(r("*", json!([r("paramNum", json!(["x"])), "2"]))),
        s(r("stringJoin", json!([r("paramStr", json!(["w"])), r("paramStr", json!(["w"]))]))),
        b(r("not", json!([r("paramBool", json!(["f"]))]))),
    ]);
    let s0 = &mut p.screens[0];
    s0.events.push(Event { target: "moreBlock".into(), name: "pair".into(), blocks: vec![], loose: vec![], params: vec![Var { name: "x".into(), ty: VarType::Num }, Var { name: "w".into(), ty: VarType::Str }, Var { name: "f".into(), ty: VarType::Bool }] });
    let none = Default::default();
    let blocks = crate::mcp::parse_stack(Some(&pair), s0, &none, "pair").expect("valid blocks");
    s0.events.last_mut().unwrap().blocks = blocks;
    let blocks = crate::mcp::parse_stack(Some(&Value::Array(program())), s0, &none, "program").expect("the check program is made of valid blocks");
    s0.events[0].blocks = blocks;
    p
}

/// What each value set says, in order, as "name = value".
fn steps(p: &Project) -> Vec<String> {
    fn walk(s: &Screen, bs: &[Block], out: &mut Vec<String>) {
        for b in bs {
            if matches!(b.op.as_str(), "setVarBoolean" | "setVarInt" | "setVarString" | "increaseInt" | "decreaseInt") {
                out.push(describe(b));
            }
            // a more block's values are set inside it
            if b.op == "moreBlock" {
                if let Some(e) = s.events.iter().find(|e| e.target == "moreBlock" && Some(&Arg::Text(e.name.clone())) == b.args.first()) {
                    walk(s, &e.blocks, out);
                }
            }
            // the program's loops have a plain count
            let times = if b.op == "repeat" { describe(b).trim_start_matches("repeat ").trim_matches('"').parse::<usize>().unwrap_or(1) } else { 1 };
            for _ in 0..times {
                walk(s, &b.sub1, out);
            }
            walk(s, &b.sub2, out);
        }
    }
    let mut out = vec![];
    walk(&p.screens[0], &p.screens[0].events[0].blocks, &mut out);
    out
}

/// Are two logged values the same? Numbers by value (each phone prints
/// them its own way: 1.0E10, 1e+10, Infinity, inf), the rest as text.
fn same(kind: Option<VarType>, a: &str, b: &str) -> bool {
    if kind == Some(VarType::Num) {
        match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            (Ok(x), Ok(y)) => x == y || (x.is_nan() && y.is_nan()) || ((x - y).abs() <= 1e-12 * x.abs().max(y.abs())),
            _ => a == b,
        }
    } else {
        a == b
    }
}

pub fn preview_values(p: &Project) -> Vec<(String, String)> {
    let rt = Runtime::new(p.clone(), 0.0);
    if let Some(c) = &rt.crash {
        println!("the preview crashed: {}", c.text);
    }
    rt.values.iter().map(|(k, v): &(String, Val)| (k.clone(), v.text())).collect()
}

fn device_values(d: Device, p: &Project) -> Result<Vec<(String, String)>, String> {
    let run = DeviceRun::start(d, p.clone(), egui::Context::default());
    let t0 = Instant::now();
    let out = loop {
        std::thread::sleep(Duration::from_millis(300));
        let st = run.state.lock().unwrap();
        let values: Vec<(String, String)> = st.lines.iter().filter(|l| l.0 == LineKind::Value).filter_map(|l| l.1.split_once(" =").map(|(k, v)| (k.to_string(), v.strip_prefix(' ').unwrap_or(v).to_string()))).collect();
        let problem = st.lines.iter().find(|l| l.0 == LineKind::Problem).map(|l| l.1.clone());
        if st.phase == Phase::Failed {
            break Err(format!("{}: {}", st.step, problem.unwrap_or_default()));
        }
        if values.iter().any(|(k, v)| k == "done" && v == "true") || problem.is_some() && t0.elapsed() > Duration::from_secs(3) && st.phase == Phase::Running {
            if let Some(pr) = problem {
                println!("{d:?} app problem: {pr}");
            }
            std::thread::sleep(Duration::from_millis(500));
            break Ok(values);
        }
        if t0.elapsed() > Duration::from_secs(400) {
            break Err(format!("timed out at: {}", st.step));
        }
    };
    run.stop();
    out
}

/// The differences, one line each; empty when they agree.
pub fn compare(p: &Project, preview: &[(String, String)], phone: &[(String, String)]) -> Vec<String> {
    let kind = |name: &str| p.screens[0].vars.iter().find(|v| v.name == name).map(|v| v.ty);
    let what = steps(p);
    let mut diffs = vec![];
    for i in 0..preview.len().max(phone.len()) {
        let label = what.get(i).cloned().unwrap_or_else(|| "?".into());
        match (preview.get(i), phone.get(i)) {
            (Some(a), Some(b)) if a.0 == b.0 && same(kind(&a.0), &a.1, &b.1) => {}
            (Some(a), Some(b)) => diffs.push(format!("#{i} {label}\n      preview: {} = {:?}\n      phone:   {} = {:?}", a.0, a.1, b.0, b.1)),
            (Some(a), None) => diffs.push(format!("#{i} {label}\n      preview: {} = {:?}\n      phone:   (stopped before this)", a.0, a.1)),
            (None, Some(b)) => diffs.push(format!("#{i} {label}\n      preview: (stopped before this)\n      phone:   {} = {:?}", b.0, b.1)),
            (None, None) => {}
        }
    }
    diffs
}

/// Mistakes that crash the Android app. Each runs as its own little app:
/// n = 1, the mistake (its result, if any, goes into s or n), n = 2, done.
pub fn crash_cases() -> Vec<(&'static str, Vec<Value>)> {
    vec![
        ("insert past the end of a list", vec![r("insertListInt", json!(["9", "5", "ln"]))]),
        ("get past the end of a number list", vec![n(r("getAtListInt", json!(["3", "ln"])))]),
        ("get past the end of a text list", vec![s(r("getAtListStr", json!(["3", "ls"])))]),
        ("delete past the end of a list", vec![r("deleteList", json!(["3", "ln"]))]),
        ("map get a missing key", vec![s(r("mapGet", json!(["m", "nope"])))]),
        ("list map get past the end", vec![s(r("getAtListMap", json!(["3", "a", "lm"])))]),
        ("list map get a missing key", vec![r("listMapFromJson", json!(["lm", r#"[{"a":1}]"#])), s(r("getAtListMap", json!(["0", "nope", "lm"])))]),
        ("substring outside the text", vec![s(r("stringSub", json!(["Hello", "2", "99"])))]),
        ("substring backwards", vec![s(r("stringSub", json!(["Hello", "3", "1"])))]),
        ("number from text that isn't one", vec![n(r("toNumber", json!(["abc"])))]),
        ("number from empty text", vec![n(r("toNumber", json!([""])))]),
    ]
}

fn project_with(name: &str, blocks: Vec<Value>) -> Project {
    let mut p = Project::new(1, name, name, "com.blockwise.check");
    let vars = json!([
        {"name": "n", "type": "number"}, {"name": "s", "type": "string"}, {"name": "b", "type": "boolean"}, {"name": "done", "type": "boolean"},
        {"name": "m", "type": "map"}, {"name": "ln", "type": "list_number"}, {"name": "ls", "type": "list_string"}, {"name": "lm", "type": "list_map"},
    ]);
    let spec = json!({"variables": vars, "events": [{"target": "screen", "event": "onCreate", "blocks": blocks}]});
    p.screens[0] = crate::templates::build(&spec, "main", &Default::default()).expect("valid blocks");
    p
}

/// What happened, in a few words: "crashed (…)" or "went on, s = …".
fn outcome(values: &[(String, String)], problem: Option<String>) -> String {
    let after: Vec<String> = values.iter().skip(1).filter(|(k, _)| k != "done").map(|(k, v)| format!("{k} = {v:?}")).collect();
    match problem {
        Some(p) => format!("crashed ({})", p.chars().take(70).collect::<String>()),
        None => format!("went on: {}", after.join(", ")),
    }
}

fn crash_program(case: &[Value]) -> Vec<Value> {
    let mut p = vec![n(json!("1"))];
    p.extend(case.iter().cloned());
    p.push(n(json!("2")));
    p.push(r("setVarBoolean", json!(["done", r("true", json!([]))])));
    p
}

#[test]
#[ignore]
fn check_crashes_against_phones() {
    let devices = match std::env::var("BLOCKWISE_DEVICE").as_deref() {
        Ok("android") => vec![Device::Android],
        Ok("iphone") | Ok("ios") => vec![Device::Iphone],
        _ => vec![Device::Android, Device::Iphone],
    };
    let mut bad = vec![];
    for (what, case) in crash_cases() {
        let p = project_with("Crash", crash_program(&case));
        let rt = Runtime::new(p.clone(), 0.0);
        let pv: Vec<(String, String)> = rt.values.iter().map(|(k, v)| (k.clone(), v.text())).collect();
        let want = (rt.crash.is_some(), pv.clone());
        println!("\n=== {what}\n  preview: {}", outcome(&pv, rt.crash.as_ref().map(|c| c.text.lines().next().unwrap_or("").to_string())));
        for d in &devices {
            let (vals, problem) = device_run(*d, &p);
            println!("  {:8} {}", format!("{}:", if *d == Device::Android { "Android" } else { "iOS" }), outcome(&vals, problem.clone()));
            // the same: both crash, or both go on with the same values
            let same_values = compare(&p, &want.1, &vals).is_empty();
            if problem.is_some() != want.0 || (!want.0 && !same_values) {
                bad.push(format!("{what} on {d:?}"));
            }
        }
    }
    assert!(bad.is_empty(), "disagree: {bad:?}");
}

/// Values and the first problem a debug build reports.
fn device_run(d: Device, p: &Project) -> (Vec<(String, String)>, Option<String>) {
    let run = DeviceRun::start(d, p.clone(), egui::Context::default());
    let t0 = Instant::now();
    let mut seen_problem: Option<Instant> = None;
    let out = loop {
        std::thread::sleep(Duration::from_millis(300));
        let st = run.state.lock().unwrap();
        let values: Vec<(String, String)> = st.lines.iter().filter(|l| l.0 == LineKind::Value).filter_map(|l| l.1.split_once(" =").map(|(k, v)| (k.to_string(), v.strip_prefix(' ').unwrap_or(v).to_string()))).collect();
        let problem = st.lines.iter().find(|l| l.0 == LineKind::Problem).map(|l| l.1.clone());
        if st.phase == Phase::Failed {
            break (values, Some(format!("build failed: {}", st.step)));
        }
        if problem.is_some() && seen_problem.is_none() {
            seen_problem = Some(Instant::now());
        }
        let done = values.iter().any(|(k, v)| k == "done" && v == "true");
        if done || seen_problem.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) || t0.elapsed() > Duration::from_secs(400) {
            break (values, problem);
        }
    };
    run.stop();
    out
}

/// Timers: a repeating one that stops itself, one started twice (the
/// second start replaces the first), and one that ends the check.
pub fn timer_project() -> Project {
    let mut p = project_with("Timers", vec![]);
    let spec = json!({
        "variables": [{"name": "n", "type": "number"}, {"name": "s", "type": "string"}, {"name": "done", "type": "boolean"}],
        "components": [{"type": "timer", "name": "t"}, {"type": "timer", "name": "t2"}, {"type": "timer", "name": "t3"}],
        "events": [{"target": "screen", "event": "onCreate", "blocks": [
            n(json!("0")),
            {"op": "timerEvery", "args": ["t", "0", "300"], "do": [
                r("increaseInt", json!(["n"])),
                {"op": "if", "args": [r("=", json!([r("getVar", json!(["n"])), "3"]))], "do": [r("timerCancel", json!(["t"]))]},
            ]},
            {"op": "timerAfter", "args": ["t2", "600"], "do": [s(json!("first"))]},
            {"op": "timerAfter", "args": ["t2", "1300"], "do": [s(json!("second"))]},
            {"op": "timerAfter", "args": ["t3", "2000"], "do": [r("setVarBoolean", json!(["done", r("true", json!([]))]))]},
        ]}],
    });
    p.screens[0] = crate::templates::build(&spec, "main", &Default::default()).expect("valid blocks");
    p
}

/// The preview's values with its clock moved on 4 seconds.
fn preview_values_timed(p: &Project) -> Vec<(String, String)> {
    let mut rt = Runtime::new(p.clone(), 0.0);
    for i in 1..=200 {
        rt.tick(i as f64 * 0.02);
    }
    rt.values.iter().map(|(k, v)| (k.clone(), v.text())).collect()
}

#[test]
#[ignore]
fn check_timers_against_phones() {
    let p = timer_project();
    let preview = preview_values_timed(&p);
    println!("preview: {preview:?}");
    let devices = match std::env::var("BLOCKWISE_DEVICE").as_deref() {
        Ok("android") => vec![Device::Android],
        Ok("iphone") | Ok("ios") => vec![Device::Iphone],
        _ => vec![Device::Android, Device::Iphone],
    };
    let mut all = 0;
    for d in devices {
        let (phone, problem) = device_run(d, &p);
        let diffs = compare(&p, &preview, &phone);
        println!("\n===== {}: {} values, {} differ{}", if d == Device::Android { "Android" } else { "iOS" }, phone.len(), diffs.len(), problem.map(|p| format!(" ({p})")).unwrap_or_default());
        println!("  phone: {phone:?}");
        all += diffs.len();
    }
    assert_eq!(all, 0, "the preview and the phones disagree");
}

/// Supabase against the pretend Supabase on this computer: a wrong
/// password, signing in, an expired sign-in renewed, reading and adding rows,
/// signing out. `url` is how the phone reaches this computer.
pub fn supabase_project(url: &str) -> Project {
    let mut p = project_with("Supa", vec![]);
    let err = |what: &str| json!([s(json!(what)), s(r("supaError", json!([])))]);
    let signed_in = || b(r("supaSignedIn", json!([])));
    let spec = json!({
        "variables": [{"name": "n", "type": "number"}, {"name": "s", "type": "string"}, {"name": "b", "type": "boolean"}, {"name": "done", "type": "boolean"},
                      {"name": "note", "type": "map"}, {"name": "rows", "type": "list_map"}],
        "events": [{"target": "screen", "event": "onCreate", "blocks": [
            r("supaSignOut", json!([])),
            signed_in(),
            {"op": "supaSignIn", "args": ["ada@x.io", "wrong"], "do": [s(json!("unexpected"))], "else": [
                s(r("supaError", json!([]))),
                n(r("webStatus", json!([]))),
                {"op": "supaSignIn", "args": ["ada@x.io ", "secret1"], "else": err("sign in failed"), "do": [
                    s(r("supaUserEmail", json!([]))),
                    s(r("supaUserId", json!([]))),
                    signed_in(),
                    {"op": "supaSelectWhere", "args": ["notes", "owner", r("supaUserId", json!([]))], "else": err("select failed"), "do": [
                        r("listMapFromJson", json!(["rows", r("webResponse", json!([]))])),
                        n(r("lengthList", json!(["rows"]))),
                        s(r("getAtListMap", json!(["0", "text", "rows"]))),
                        r("mapPut", json!(["note", "text", "a b&c"])),
                        {"op": "supaInsert", "args": ["note", "notes"], "else": err("insert failed"), "do": [
                            n(r("webStatus", json!([]))),
                            r("listMapFromJson", json!(["rows", r("webResponse", json!([]))])),
                            s(r("getAtListMap", json!(["0", "text", "rows"]))),
                            r("supaSignOut", json!([])),
                            signed_in(),
                            r("setVarBoolean", json!(["done", r("true", json!([]))])),
                        ]},
                    ]},
                ]},
            ]},
        ]}],
    });
    p.screens[0] = crate::templates::build(&spec, "main", &Default::default()).expect("valid blocks");
    p.supabase = Some(SupabaseLink { url: url.into(), key: "anon".into() });
    p
}

fn preview_values_web(p: &Project) -> Vec<(String, String)> {
    let mut rt = Runtime::new(p.clone(), 0.0);
    for i in 0..500 {
        rt.tick(i as f64 * 0.01);
        if !rt.has_timers() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    rt.values.iter().map(|(k, v)| (k.clone(), v.text())).collect()
}

#[test]
#[ignore]
fn check_supabase_against_phones() {
    let (url, log) = crate::runtime::tests::fake_supabase();
    let port = url.rsplit(':').next().unwrap().to_string();
    let preview = preview_values_web(&supabase_project(&url));
    println!("preview: {preview:?}");
    let devices = match std::env::var("BLOCKWISE_DEVICE").as_deref() {
        Ok("android") => vec![Device::Android],
        Ok("iphone") | Ok("ios") => vec![Device::Iphone],
        _ => vec![Device::Android, Device::Iphone],
    };
    let mut all = 0;
    for d in devices {
        // the Android emulator sees this computer as 10.0.2.2
        let host = if d == Device::Android { "10.0.2.2" } else { "127.0.0.1" };
        let p = supabase_project(&format!("http://{host}:{port}"));
        let (phone, problem) = device_run(d, &p);
        let diffs = compare(&p, &preview, &phone);
        println!("\n===== {}: {} values, {} differ{}", if d == Device::Android { "Android" } else { "iOS" }, phone.len(), diffs.len(), problem.map(|p| format!(" ({p})")).unwrap_or_default());
        for l in &diffs {
            println!("  {l}");
        }
        all += diffs.len();
    }
    println!("\nrequests the pretend Supabase saw:\n{}", log.lock().unwrap().join("\n"));
    assert_eq!(all, 0, "the preview and the phones disagree");
}

#[test]
fn the_supabase_check_runs_in_the_preview() {
    let (url, _) = crate::runtime::tests::fake_supabase();
    let v = preview_values_web(&supabase_project(&url));
    let got: Vec<String> = v.iter().map(|(k, v)| format!("{k}={v}")).collect();
    assert_eq!(got, ["b=false", "s=Invalid login credentials", "n=400.0", "s=ada@x.io", "s=u1", "b=true", "n=1.0", "s=hi there", "n=201.0", "s=a b&c", "b=false", "done=true"]);
}

#[test]
fn the_timer_check_runs_in_the_preview() {
    let v = preview_values_timed(&timer_project());
    let names: Vec<String> = v.iter().map(|(k, v)| format!("{k}={v}")).collect();
    assert_eq!(names, ["n=0.0", "n=1.0", "n=2.0", "n=3.0", "s=second", "done=true"]);
}

#[test]
fn the_check_program_runs_in_the_preview() {
    let p = project();
    let v = preview_values(&p);
    assert_eq!(v.len(), steps(&p).len(), "every step sets one value");
    assert_eq!(v.last().unwrap(), &("done".to_string(), "true".to_string()));
}

#[test]
#[ignore]
fn check_preview_against_phones() {
    let p = project();
    let preview = preview_values(&p);
    let devices = match std::env::var("BLOCKWISE_DEVICE").as_deref() {
        Ok("android") => vec![Device::Android],
        Ok("iphone") => vec![Device::Iphone],
        _ => vec![Device::Android, Device::Iphone],
    };
    let mut all = 0;
    for d in devices {
        let phone = match device_values(d, &p) {
            Ok(v) => v,
            Err(e) => panic!("{d:?}: {e}"),
        };
        let diffs = compare(&p, &preview, &phone);
        println!("\n===== {}: {} values, {} differ", if d == Device::Android { "Android" } else { "iOS" }, phone.len(), diffs.len());
        for l in &diffs {
            println!("  {l}");
        }
        all += diffs.len();
    }
    assert_eq!(all, 0, "the preview and the phones disagree");
}

#[test]
#[ignore]
fn engine_speed() {
    // 200,000 loop turns, 5 blocks each: a heavy event for a phone app
    let blocks = vec![
        n(json!("0")),
        json!({"op": "repeat", "args": ["200000"], "do": [
            r("increaseInt", json!(["n"])),
            r("addListInt", json!([r("*", json!([r("getVar", json!(["n"])), "2"])), "ln"])),
            {"op": "if", "args": [r(">", json!([r("lengthList", json!(["ln"])), "100"]))], "do": [r("deleteList", json!(["0", "ln"]))]},
        ]}),
        s(r("toString", json!([r("getVar", json!(["n"]))]))),
    ];
    let p = project_with("Speed", blocks);
    let t = Instant::now();
    let rt = Runtime::new(p, 0.0);
    println!("took {:?}, steps stopped: {:?}, s = {:?}", t.elapsed(), rt.log.iter().find(|l| l.contains("Stopped")), rt.vars.get("s").map(|v| v.text()));
}
